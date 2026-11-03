// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult};

use inca_gpui::NodeId;

use super::Natives;
use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;

/// The retained tree itself: allocating, attaching, and describing nodes, plus `destroyNode`,
/// `releaseListeners`, `holdNode` and `releaseNode`. Freeing a subtree is a tree operation first,
/// even though it also releases the listener/focus state that named its nodes.
pub(super) fn install_tree(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("rootNodeId", root_node_id)?;
    n.set("createNode", create_node)?;
    n.set("appendChild", append_child)?;
    n.set("insertBefore", insert_before)?;
    n.set("removeChild", remove_child)?;
    n.set("setAttribute", super::attributes::set_attribute)?;
    n.set("getAttribute", super::attributes::get_attribute)?;
    n.set("removeAttribute", super::attributes::remove_attribute)?;
    n.set("attributeNames", super::attributes::attribute_names)?;
    n.set("setStyle", super::attributes::set_style)?;
    n.set("removeStyle", super::attributes::remove_style)?;
    n.set("destroyNode", destroy_node)?;
    n.set("releaseListeners", release_listeners)?;
    n.set("holdNode", hold_node)?;
    n.set("releaseNode", release_node)
}

fn root_node_id(host: Rc<RefCell<Host>>) -> impl Fn() -> NodeId {
    move || host.borrow().root()
}

fn create_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, String) -> JsResult<NodeId> {
    move |ctx, tag_name| {
        let mut host = host.borrow_mut();
        host.tree
            .create_node(tag_name)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn append_child<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id)| {
        insert_child(&ctx, &mut host.borrow_mut(), parent_id, child_id, None)
    }
}

fn insert_before<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId, Option<JsNodeId>) -> JsResult<()> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id), anchor_id| {
        let anchor_id = anchor_id.map(|JsNodeId(id)| id);
        insert_child(&ctx, &mut host.borrow_mut(), parent_id, child_id, anchor_id)
    }
}

/// Inserts `child_id` under `parent_id` before `anchor_id`, or last without one, and queues
/// its focus re-check. A moved attached node drops the focus it held.
fn insert_child(
    ctx: &Ctx<'_>,
    host: &mut Host,
    parent_id: NodeId,
    child_id: NodeId,
    anchor_id: Option<NodeId>,
) -> JsResult<()> {
    refuse_root(ctx, host, child_id, "moved")?;
    let attached = is_attached(host, child_id);
    let before = host.tree.preceding(child_id);
    host.tree
        .insert_before(parent_id, child_id, anchor_id)
        .map_err(|err| throw_tree_error(ctx, err))?;
    if attached {
        drop_focus(host, child_id);
        move_tab_start(host, child_id, before);
    }
    mark_focus_dirty(host, child_id);
    Ok(())
}

fn remove_child<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id)| {
        let mut host = host.borrow_mut();
        let child_of_parent = host
            .tree
            .get(child_id)
            .is_some_and(|node| node.parent() == Some(parent_id));
        let before = host.tree.preceding(child_id);
        host.tree
            .remove_child(parent_id, child_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        if child_of_parent {
            drop_focus(&mut host, child_id);
            move_tab_start(&mut host, child_id, before);
            mark_focus_dirty(&mut host, child_id);
        }
        Ok(())
    }
}

fn destroy_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Vec<u32>> {
    move |ctx, JsNodeId(node_id)| {
        let mut host = host.borrow_mut();
        refuse_root(&ctx, &host, node_id, "destroyed")?;
        let before = host.tree.preceding(node_id);
        move_tab_start(&mut host, node_id, before);
        let freed = host.tree.destroy_node(node_id);
        Ok(host.free_ids(&freed))
    }
}

/// Drops the listeners of `node_id` and of every node below it and returns their callback ids.
/// The nodes stay in the tree. An unknown id returns no ids.
fn release_listeners(host: Rc<RefCell<Host>>) -> impl Fn(JsNodeId) -> Vec<u32> {
    move |JsNodeId(node_id)| {
        let mut host = host.borrow_mut();
        let mut callbacks = Vec::new();
        let mut pending = vec![node_id];
        while let Some(current) = pending.pop() {
            if let Some(node) = host.tree.get(current) {
                pending.extend(node.children().iter().copied());
                callbacks.extend(host.listeners.release(current));
            }
        }
        callbacks
    }
}

fn hold_node<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(node_id)| {
        host.borrow_mut()
            .tree
            .hold(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn release_node(host: Rc<RefCell<Host>>) -> impl Fn(JsNodeId) {
    move |JsNodeId(node_id)| host.borrow_mut().tree.release(node_id)
}

/// Refuses `id` when it is the root node, which belongs to the host. `verb`
/// completes the message, as in "cannot be {verb}".
///
/// # Errors
///
/// Throws a `TypeError` when `id` is the root.
fn refuse_root(ctx: &Ctx<'_>, host: &Host, id: NodeId, verb: &str) -> JsResult<()> {
    if id == host.root() {
        return Err(Exception::throw_type(
            ctx,
            &format!("the root node belongs to the host and cannot be {verb}"),
        ));
    }
    Ok(())
}

fn is_attached(host: &Host, node_id: NodeId) -> bool {
    host.tree
        .get(node_id)
        .is_some_and(|node| node.parent().is_some())
}

/// Called when `removeChild` detaches a node or an insert moves an attached
/// one. A node that was never attached holds no focus to drop.
fn drop_focus(host: &mut Host, node_id: NodeId) {
    host.focus.drop_focus_in(&host.tree, node_id);
}

/// Moves where Tab starts off `node_id`'s subtree to `before`, the node right before its place.
fn move_tab_start(host: &mut Host, node_id: NodeId, before: Option<NodeId>) {
    host.focus.start_leaves(&host.tree, node_id, before);
}

/// Queues a focus re-check of the node and its subtree.
pub(super) fn mark_focus_dirty(host: &mut Host, node_id: NodeId) {
    host.focus.mark_subtree_dirty(&host.tree, node_id);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{LISTEN_JS, callback_ids, engine_with_bindings, error_name};
    use crate::{EventTargetId, Host};
    use inca_gpui::NodeId;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn remove_style_on_an_unknown_node_throws() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                let caught = false;
                try {
                    n.removeStyle(999, 'gap');
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }

    /// Evaluates `script` against bindings whose style warnings are
    /// collected, and returns them with the host.
    fn warnings_from(script: &str) -> (Rc<RefCell<Vec<String>>>, Rc<RefCell<Host>>) {
        let (engine, host) = engine_with_bindings();
        let warnings = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&warnings);
        host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));
        engine.eval::<()>(script).unwrap();
        (warnings, host)
    }

    #[test]
    fn set_style_warns_once_per_unknown_key() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'flexDirections', 'column');
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 1 (div): ignoring unknown style key `flexDirections`"]
        );
    }

    #[test]
    fn a_bad_value_warns_once_for_a_node_and_key() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'background', -1);
            __inca_native__.setStyle(n, 'background', -1);
            __inca_native__.setStyle(n, 'background', 0xff0000);
            __inca_native__.setStyle(n, 'background', -1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 1 (div): ignoring invalid color for style key `background`"]
        );
    }

    #[test]
    fn remove_style_does_not_make_a_key_warn_again() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.removeStyle(n, 'bogus');
            __inca_native__.setStyle(n, 'bogus', 1);
            ",
        );

        assert_eq!(warnings.borrow().len(), 1, "{:?}", warnings.borrow());
    }

    #[test]
    fn each_node_warns_for_the_same_bad_key() {
        let (warnings, _) = warnings_from(
            r"
            const a = __inca_native__.createNode('div');
            const b = __inca_native__.createNode('div');
            __inca_native__.setStyle(a, 'bogus', 1);
            __inca_native__.setStyle(b, 'bogus', 1);
            __inca_native__.setStyle(a, 'bogus', 1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            [
                "node 1 (div): ignoring unknown style key `bogus`",
                "node 2 (div): ignoring unknown style key `bogus`",
            ]
        );
    }

    #[test]
    fn each_spelling_of_a_bad_key_warns() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus-key', 1);
            __inca_native__.setStyle(n, 'bogusKey', 1);
            __inca_native__.setStyle(n, 'bogus-key', 1);
            __inca_native__.setStyle(n, 'bogusKey', 1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            [
                "node 1 (div): ignoring unknown style key `bogus-key`",
                "node 1 (div): ignoring unknown style key `bogusKey`",
            ]
        );
    }

    #[test]
    fn each_bad_key_of_a_node_warns_once() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.setStyle(n, 'width', '50%');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.setStyle(n, 'width', '50%');
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            [
                "node 1 (div): ignoring unknown style key `bogus`",
                "node 1 (div): ignoring unsupported value for style key `width`",
            ]
        );
    }

    #[test]
    fn a_destroyed_node_forgets_its_style_warnings() {
        let (_, host) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            __inca_native__.destroyNode(n);
            ",
        );

        assert!(host.borrow().style_warned.is_empty());
    }

    #[test]
    fn a_failed_set_style_emits_no_warning() {
        let (warnings, host) = warnings_from(
            r"
            try {
                __inca_native__.setStyle(999, 'bogus', 1);
            } catch (e) {}
            const n = __inca_native__.createNode('div');
            try {
                __inca_native__.setStyle(n, 'bogus', { nested: true });
            } catch (e) {}
            ",
        );

        assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
        assert!(host.borrow().style_warned.is_empty());
    }

    #[test]
    fn set_style_names_the_tag_of_the_node() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('#text');
            __inca_native__.setStyle(n, 'bogus', 1);
            ",
        );

        assert_eq!(
            *warnings.borrow(),
            ["node 1 (#text): ignoring unknown style key `bogus`"]
        );
    }

    #[test]
    fn set_style_is_quiet_for_known_keys_and_box_keys() {
        let (warnings, _) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'display', 'flex');
            for (const k of ['padding', 'padding_x', 'margin_left', 'gap']) {
                __inca_native__.setStyle(n, k, 4);
            }
            __inca_native__.setStyle(n, 'text_color', '#fff');
            ",
        );

        assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
    }

    #[test]
    fn a_stored_bad_style_does_not_warn_again_when_the_tree_is_read() {
        let (warnings, host) = warnings_from(
            r"
            const n = __inca_native__.createNode('div');
            __inca_native__.setStyle(n, 'bogus', 1);
            ",
        );
        assert_eq!(warnings.borrow().len(), 1);

        let host = host.borrow();
        for _ in 0..3 {
            inca_gpui::build_spec(&host.tree, 1).unwrap();
        }
        assert_eq!(warnings.borrow().len(), 1);
    }

    #[test]
    fn set_style_unknown_node_id_throws_catchable_exception() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.setStyle(999, 'display', 'flex');
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(
            caught,
            "an unknown node id must raise a catchable exception"
        );
    }

    #[test]
    fn destroy_node_hands_back_the_callback_ids_of_its_subtree() {
        let (engine, _host) = engine_with_bindings();

        let released: Vec<u32> = engine
            .eval(
                r"
                const root = __inca_native__.rootNodeId();
                const branch = __inca_native__.createNode('div');
                const leaf = __inca_native__.createNode('#text');
                __inca_native__.appendChild(root, branch);
                __inca_native__.appendChild(branch, leaf);
                __inca_native__.addEventListener(branch, 'click');
                __inca_native__.addEventListener(leaf, 'click');

                __inca_native__.destroyNode(branch);
                ",
            )
            .unwrap();
        let mut released = released;
        released.sort_unstable();

        assert_eq!(
            released,
            vec![0, 1],
            "a descendant's callback is unreachable from JS once its root is gone"
        );
    }

    #[test]
    fn destroy_node_frees_its_subtree() {
        let (engine, host) = engine_with_bindings();

        let _: Vec<u32> = engine
            .eval(
                r"
                const root = __inca_native__.rootNodeId();
                const branch = __inca_native__.createNode('div');
                const leaf = __inca_native__.createNode('#text');
                __inca_native__.appendChild(root, branch);
                __inca_native__.appendChild(branch, leaf);
                __inca_native__.addEventListener(branch, 'click');
                __inca_native__.addEventListener(leaf, 'click');

                __inca_native__.destroyNode(branch);
                ",
            )
            .unwrap();

        let host = host.borrow();
        assert!(host.tree.get(host.root()).unwrap().children().is_empty());
        assert!(callback_ids(&host.listeners, crate::EventTargetId::Node(1), "click").is_empty());
    }

    #[test]
    fn destroy_node_frees_the_scroll_handles_of_the_subtree_only() {
        let (engine, host) = engine_with_bindings();
        let ids: Vec<NodeId> = engine
            .eval(
                r"
                const root = __inca_native__.rootNodeId();
                const branch = __inca_native__.createNode('div');
                const leaf = __inca_native__.createNode('div');
                const kept = __inca_native__.createNode('div');
                __inca_native__.appendChild(root, branch);
                __inca_native__.appendChild(branch, leaf);
                __inca_native__.appendChild(root, kept);
                [branch, leaf, kept]
                ",
            )
            .unwrap();
        let (branch, leaf, kept) = (ids[0], ids[1], ids[2]);
        for id in [branch, leaf, kept] {
            host.borrow().scroll_handle(id);
        }

        engine
            .eval::<()>(&format!("__inca_native__.destroyNode({branch})"))
            .unwrap();

        let host = host.borrow();
        let handles = host.scroll_handles.borrow();
        assert!(!handles.contains_key(&branch));
        assert!(
            !handles.contains_key(&leaf),
            "a descendant's handle goes with its ancestor"
        );
        assert!(
            handles.contains_key(&kept),
            "a surviving node keeps its handle"
        );
    }

    #[test]
    fn destroy_node_is_idempotent() {
        let (engine, _host) = engine_with_bindings();

        let second_pass: Vec<u32> = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                __inca_native__.destroyNode(node);
                __inca_native__.destroyNode(node);
                ",
            )
            .unwrap();

        assert!(second_pass.is_empty());
    }

    #[test]
    fn destroying_the_root_throws_a_catchable_exception() {
        let (engine, host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.destroyNode(__inca_native__.rootNodeId());
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught, "the root belongs to the host, not to the app");
        let host = host.borrow();
        assert!(host.tree.get(host.root()).is_some());
    }

    #[test]
    fn moving_the_root_throws_a_catchable_exception() {
        let (engine, host) = engine_with_bindings();

        let caught: u32 = engine
            .eval(
                r"
                const root = __inca_native__.rootNodeId();
                const n = __inca_native__.createNode('div');
                let caught = 0;
                try {
                    __inca_native__.appendChild(n, root);
                } catch (e) {
                    if (e instanceof TypeError) caught += 1;
                }
                try {
                    __inca_native__.insertBefore(n, root, null);
                } catch (e) {
                    if (e instanceof TypeError) caught += 1;
                }
                __inca_native__.destroyNode(n);
                caught;
                ",
            )
            .unwrap();

        assert_eq!(caught, 2, "the root belongs to the host, not to the app");
        let host = host.borrow();
        let root = host.tree.get(host.root()).unwrap();
        assert_eq!(root.parent(), None);
    }

    #[test]
    fn a_cycle_throws_hierarchy_request_error() {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                globalThis.a = __inca_native__.createNode('div');
                globalThis.c = __inca_native__.createNode('div');
                __inca_native__.appendChild(a, c);
                ",
            )
            .unwrap();

        assert_eq!(
            error_name(&engine, "__inca_native__.appendChild(c, a)"),
            "HierarchyRequestError"
        );
    }

    /// Builds `root > branch > leaf` with a click listener on each of the two lower nodes.
    const BRANCH_WITH_LISTENERS: &str = r"
        const root = __inca_native__.rootNodeId();
        const branch = __inca_native__.createNode('div');
        const leaf = __inca_native__.createNode('div');
        __inca_native__.appendChild(root, branch);
        __inca_native__.appendChild(branch, leaf);
        __inca_native__.addEventListener(branch, 'click');
        __inca_native__.addEventListener(leaf, 'click');
        ";

    #[test]
    fn release_listeners_returns_the_callbacks_of_the_node_and_a_descendant() {
        let (engine, _host) = engine_with_bindings();

        let mut released: Vec<u32> = engine
            .eval(&format!(
                r"
                {BRANCH_WITH_LISTENERS}
                __inca_native__.releaseListeners(branch);
                "
            ))
            .unwrap();
        released.sort_unstable();

        assert_eq!(released, [0, 1]);
    }

    /// Releases the listeners of `branch` and returns `[branch, leaf]`.
    fn release_branch(engine: &inca_jsenv::Engine) -> Vec<NodeId> {
        engine
            .eval(&format!(
                r"
                {BRANCH_WITH_LISTENERS}
                __inca_native__.setAttribute(leaf, 'id', 'x');
                __inca_native__.releaseListeners(branch);
                [branch, leaf];
                "
            ))
            .unwrap()
    }

    #[test]
    fn release_listeners_drops_the_registrations() {
        let (engine, host) = engine_with_bindings();
        let ids = release_branch(&engine);

        let host = host.borrow();
        assert!(callback_ids(&host.listeners, EventTargetId::Node(ids[0]), "click").is_empty());
        assert!(callback_ids(&host.listeners, EventTargetId::Node(ids[1]), "click").is_empty());
    }

    #[test]
    fn release_listeners_keeps_the_nodes_and_their_attributes() {
        let (engine, host) = engine_with_bindings();
        let ids = release_branch(&engine);

        let host = host.borrow();
        let leaf_node = host.tree.get(ids[1]).unwrap();
        assert_eq!(leaf_node.parent(), Some(ids[0]));
        assert!(leaf_node.attributes().contains_key("id"));
    }

    #[test]
    fn release_listeners_of_an_unknown_id_returns_nothing() {
        let (engine, _host) = engine_with_bindings();

        let released: Vec<u32> = engine
            .eval("__inca_native__.releaseListeners(999);")
            .unwrap();

        assert!(released.is_empty());
    }

    #[test]
    fn a_dispatched_event_reaches_no_listener_after_release_listeners() {
        let (engine, _host) = engine_with_bindings();
        engine.eval::<()>(LISTEN_JS).unwrap();

        let ran: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                let ran = false;
                __inca_test_listen__(node, 'ping', () => {
                    ran = true;
                });
                __inca_native__.releaseListeners(node);
                __inca_native__.dispatchEvent(node, new Event('ping'));
                ran;
                ",
            )
            .unwrap();

        assert!(!ran);
    }
}
