// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use inca_document::free_ids;
use inca_jsenv::callbacks;
use inca_node::NodeId;

use crate::bindings::convert::{
    Flag, JsNodeId, existing_node, or_null, saturated, throw_tree_error, unsigned,
};
use crate::bindings::{Host, Natives};

/// The retained tree itself: allocating nodes, plus the collection reads `childCount`, `childAt`,
/// `namedChild` and `childNames` (a freed node has no children), `templateContent`, `destroyNode`,
/// `holdNode` and `releaseNode`. Freeing a subtree is a tree operation first, even though it also
/// releases the listener/focus state that named its nodes.
pub(crate) fn install_tree(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("rootNodeId", root_node_id)?;
    n.set("documentNodeId", document_node_id)?;
    n.set("createNode", create_node)?;
    n.set("childCount", child_count)?;
    n.set("childAt", child_at)?;
    n.set("namedChild", named_child)?;
    n.set("childNames", child_names)?;
    n.set("templateContent", template_content)?;
    n.set("destroyNode", destroy_node)?;
    n.set("holdNode", hold_node)?;
    n.set("releaseNode", release_node)
}

fn root_node_id(host: Rc<Host>) -> impl Fn() -> NodeId {
    move || host.document.borrow().root()
}

fn document_node_id(host: Rc<Host>) -> impl Fn() -> NodeId {
    move || host.document.borrow().document()
}

fn create_node<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, String) -> JsResult<NodeId> {
    move |ctx, local_name| {
        let mut host = host.document.borrow_mut();
        host.tree
            .create_node(local_name)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

/// The number of children of the node `id`, only the elements with `elementOnly`.
fn child_count(host: Rc<Host>) -> impl Fn(JsNodeId, Flag) -> u32 {
    move |JsNodeId(node_id), element_only| {
        saturated(
            host.document
                .borrow()
                .tree
                .child_count(node_id, element_only.0.flatten() == Some(true)),
        )
    }
}

/// The id of the child of the node `id` at `index`, counting only the elements with
/// `elementOnly`; `null` past the last child.
fn child_at<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>, Flag) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), index, element_only| {
        let found = host.document.borrow().tree.child_at(
            node_id,
            unsigned(index) as usize,
            element_only.0.flatten() == Some(true),
        );
        or_null(&ctx, found)
    }
}

/// The id of the first element child of the node `id` that `HTMLCollection.namedItem` finds for
/// `name`, or `null`.
fn named_child<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), name| {
        or_null(
            &ctx,
            host.document.borrow().tree.named_child(node_id, &name),
        )
    }
}

/// The supported property names of the element children of the node `id`.
fn child_names(host: Rc<Host>) -> impl Fn(JsNodeId) -> Vec<String> {
    move |JsNodeId(node_id)| host.document.borrow().tree.child_names(node_id)
}

/// The id of the content fragment of the template `node_id`, `null` for a node that is no template.
fn template_content<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.document.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        or_null(&ctx, host.tree.template_content(node_id))
    }
}

fn destroy_node<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Vec<u32>> {
    move |ctx, JsNodeId(node_id)| {
        let mut document = host.document.borrow_mut();
        if document
            .tree
            .ancestors(document.root())
            .any(|id| id == node_id)
        {
            return Err(Exception::throw_type(
                &ctx,
                "the root node belongs to the host. neither it nor a node above it can be destroyed",
            ));
        }
        if node_id == document.document() {
            return Err(Exception::throw_type(
                &ctx,
                "the document belongs to the host. it cannot be destroyed",
            ));
        }
        let before = document.tree.preceding(node_id);
        document.move_tab_start(node_id, before);
        let freed = document.tree.destroy_node(node_id);
        let callback_ids = free_ids(&mut document, &mut *host.window.borrow_mut(), &freed);
        crate::abort::settle_freed(&ctx, &mut document)?;
        callbacks::release(&ctx, &callback_ids)?;
        Ok(callback_ids)
    }
}

fn hold_node<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(node_id)| {
        host.document
            .borrow_mut()
            .tree
            .hold(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn release_node(host: Rc<Host>) -> impl Fn(JsNodeId) {
    move |JsNodeId(node_id)| host.document.borrow_mut().tree.release(node_id)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use rstest::rstest;

    use crate::test_support::{callback_ids, engine_with_bindings, engine_with_view, error_name};
    use crate::tests::{callback_stored, clocked_with_signal_option};
    use inca_node::NodeId;
    use serde_json::json;

    /// Builds `parent` with the children `a` (div, id `x`), a text node and `b` (span, name `y`).
    const COLLECTION: &str = r"
        const n = __inca_native__;
        const parent = n.createNode('div');
        const a = n.createNode('div');
        const text = n.createNode('#text');
        const b = n.createNode('span');
        n.setAttribute(a, 'id', 'x');
        n.setAttribute(b, 'name', 'y');
        n.appendChild(parent, a);
        n.appendChild(parent, text);
        n.appendChild(parent, b);
        ";

    #[rstest]
    #[case::count_of_all("n.childCount(parent)", "3")]
    #[case::count_of_elements("n.childCount(parent, true)", "2")]
    #[case::at_of_all("n.childAt(parent, 1) === text", "true")]
    #[case::at_of_elements("n.childAt(parent, 1, true) === b", "true")]
    #[case::at_past_the_end("n.childAt(parent, 2, true)", "null")]
    #[case::named_by_id("n.namedChild(parent, 'x') === a", "true")]
    #[case::named_by_name("n.namedChild(parent, 'y') === b", "true")]
    #[case::named_no_match("n.namedChild(parent, 'z')", "null")]
    #[case::names("n.childNames(parent).join()", "x,y")]
    fn a_collection_native_reads_the_children(#[case] call: &str, #[case] want: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(&format!("{COLLECTION} String({call});"))
            .unwrap();

        assert_eq!(got, want);
    }

    #[rstest]
    #[case::count("childCount(999)", "0")]
    #[case::count_of_elements("childCount(999, true)", "0")]
    #[case::at("childAt(999, 0)", "null")]
    #[case::at_of_elements("childAt(999, 0, true)", "null")]
    #[case::named("namedChild(999, 'x')", "null")]
    #[case::names("childNames(999).length", "0")]
    fn a_collection_native_reads_a_freed_node_as_empty(#[case] call: &str, #[case] want: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(&format!("String(__inca_native__.{call});"))
            .unwrap();

        assert_eq!(got, want);
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
                __inca_native__.addEventListener(branch, 'click', () => {});
                __inca_native__.addEventListener(leaf, 'click', () => {});

                __inca_native__.destroyNode(branch);
                ",
            )
            .unwrap();
        let mut released = released;
        released.sort_unstable();

        assert_eq!(
            released,
            vec![1, 2],
            "a descendant's callback is unreachable from JS once its root is gone"
        );
    }

    /// Builds `root > branch > leaf` with a click listener on each of the two lower nodes.
    const BRANCH_WITH_LISTENERS: &str = r"
        const root = __inca_native__.rootNodeId();
        const branch = __inca_native__.createNode('div');
        const leaf = __inca_native__.createNode('div');
        __inca_native__.appendChild(root, branch);
        __inca_native__.appendChild(branch, leaf);
        __inca_native__.addEventListener(branch, 'click', () => {});
        __inca_native__.addEventListener(leaf, 'click', () => {});
        ";

    #[test]
    fn destroy_node_releases_the_table_entries_of_its_subtree() {
        let (engine, _host) = engine_with_bindings();

        engine
            .eval::<()>(&format!(
                r"
                {BRANCH_WITH_LISTENERS}
                __inca_native__.destroyNode(branch);
                "
            ))
            .unwrap();

        assert!(!callback_stored(&engine, 1));
        assert!(!callback_stored(&engine, 2));
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
                __inca_native__.addEventListener(branch, 'click', () => {});
                __inca_native__.addEventListener(leaf, 'click', () => {});

                __inca_native__.destroyNode(branch);
                ",
            )
            .unwrap();

        let host = host.document.borrow();
        assert!(host.tree.get(host.root()).unwrap().children().is_empty());
        assert!(
            callback_ids(
                &host.listeners,
                inca_document::EventTargetId::Node(1),
                "click"
            )
            .is_empty()
        );
    }

    #[test]
    fn destroy_node_frees_the_scroll_handles_of_the_subtree_only() {
        let (engine, _host, view) = engine_with_view();
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
            view.borrow().offsets.borrow_mut().insert(id, (1.0, 1.0));
        }

        engine
            .eval::<()>(&format!("__inca_native__.destroyNode({branch})"))
            .unwrap();

        let view = view.borrow();
        let handles = view.offsets.borrow();
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
    fn destroying_a_node_above_the_root_throws_and_keeps_the_root() {
        let (engine, host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const loose = __inca_native__.createNode('div');
                __inca_native__.appendChild(loose, __inca_native__.rootNodeId());
                let caught = false;
                try {
                    __inca_native__.destroyNode(loose);
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
        let host = host.document.borrow();
        assert!(host.tree.get(host.root()).is_some());
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
        let host = host.document.borrow();
        assert!(host.tree.get(host.root()).is_some());
    }

    #[test]
    fn destroying_the_document_throws_a_type_error() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                __inca_native__.removeChild(
                    __inca_native__.documentNodeId(),
                    __inca_native__.rootNodeId()
                );
                ",
            )
            .unwrap();

        let name = error_name(
            &engine,
            "__inca_native__.destroyNode(__inca_native__.documentNodeId())",
        );

        assert_eq!(name, "TypeError");
        let host = host.document.borrow();
        assert!(host.tree.get(host.document()).is_some());
    }

    #[test]
    fn the_document_node_is_the_parent_of_the_root_node() {
        let (engine, host) = engine_with_bindings();

        let ids: Vec<u32> = engine
            .eval("[__inca_native__.documentNodeId(), __inca_native__.rootNodeId()]")
            .unwrap();

        let host = host.document.borrow();
        assert_eq!(ids, [host.document(), host.root()]);
        assert_eq!(host.tree.get(ids[1]).unwrap().parent(), Some(ids[0]));
    }

    #[test]
    fn template_content_is_the_same_fragment_on_each_read() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const template = n.createNode('template');
                const content = n.templateContent(template);
                content !== template && content === n.templateContent(template);
                ",
            )
            .unwrap();

        assert!(same);
    }

    #[test]
    fn template_content_of_a_div_is_null() {
        let (engine, _host) = engine_with_bindings();

        let is_null: bool = engine
            .eval("__inca_native__.templateContent(__inca_native__.createNode('div')) === null;")
            .unwrap();

        assert!(is_null);
    }

    #[test]
    fn template_content_of_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.templateContent(999);
                } catch (e) {
                    caught = e instanceof TypeError;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught);
    }

    #[test]
    fn root_node_id_returns_the_host_root() {
        let (engine, host) = engine_with_bindings();

        let root_id: NodeId = engine.eval("__inca_native__.rootNodeId();").unwrap();

        assert_eq!(root_id, host.document.borrow().root());
        assert_eq!(
            host.document
                .borrow()
                .tree
                .get(root_id)
                .unwrap()
                .local_name(),
            "div"
        );
    }

    #[test]
    fn hold_node_throws_type_error_for_an_unknown_id() {
        let (engine, _host) = engine_with_bindings();

        let type_error: bool = engine
            .eval(
                r"
                let type_error = false;
                try {
                    __inca_native__.holdNode(999);
                } catch (e) {
                    type_error = e instanceof TypeError;
                }
                type_error;
                ",
            )
            .unwrap();

        assert!(type_error);
    }

    #[test]
    fn release_node_ignores_an_unknown_id() {
        let (engine, _host) = engine_with_bindings();

        let threw: bool = engine
            .eval(
                r"
                let threw = false;
                try {
                    __inca_native__.releaseNode(999);
                } catch (e) {
                    threw = true;
                }
                threw;
                ",
            )
            .unwrap();

        assert!(!threw);
    }

    #[test]
    fn a_held_node_survives_a_sweep_until_it_is_released() {
        let (engine, host) = engine_with_bindings();
        let node: NodeId = engine
            .eval("const n = __inca_native__.createNode('div'); __inca_native__.holdNode(n); n;")
            .unwrap();

        assert!(host.sweep_orphans().is_none());
        assert!(host.document.borrow().tree.get(node).is_some());

        engine
            .eval::<()>(&format!("__inca_native__.releaseNode({node})"))
            .unwrap();
        let (nodes, _) = host.sweep_orphans().unwrap();
        assert_eq!(nodes, [node]);
    }

    #[test]
    fn the_free_of_a_node_unlinks_its_listeners() {
        let f = clocked_with_signal_option();
        f.engine
            .eval::<()>("add(node, () => {}, s); n.destroyNode(node);")
            .unwrap();

        assert_eq!(f.host.document.borrow().listeners.links.len(), 0);
    }

    #[rstest]
    #[case::remove_node("n.removeNode(node);")]
    #[case::destroy_node("n.destroyNode(node);")]
    fn freeing_a_node_unpins_the_dependent_its_listener_names(#[case] free: &str) {
        let f = clocked_with_signal_option();
        f.engine
            .eval::<()>(&format!(
                r"
                (() => {{
                    const dependent = AbortSignal.any([s]);
                    add(node, () => {{}}, dependent);
                    globalThis.w = new WeakRef(dependent);
                }})();
                {free}
                "
            ))
            .unwrap();

        for _ in 0..2 {
            f.engine.with(|ctx| ctx.run_gc());
            drop(f.engine.run_jobs());
        }

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }
}
