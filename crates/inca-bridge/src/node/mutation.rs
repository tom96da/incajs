// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mutation natives, `removeNode` included, and the host steps that follow a node leaving
//! its place.

use std::rc::Rc;

use rquickjs::{Array, Ctx, Result as JsResult, Value};

use inca_document::settle;
use inca_jsenv::callbacks;
use inca_node::{InsertArg, NodeId, Placement, VirtualTree};

use super::clone_node::clone_node;
use super::insert_nodes::insert_planned;
use crate::bindings::convert::{JsNodeId, throw_tree_error};
use crate::bindings::{Host, Natives};

/// The mutation members: `appendChild`, `insertBefore`, `removeChild`, `removeNode`,
/// `replaceChild`, `replaceWithText` and `cloneNode`. Each moved or removed node drops the focus it
/// held, moves the Tab start, queues a focus re-check and resets the scroll offsets of its subtree.
pub(crate) fn install_mutation(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("appendChild", append_child)?;
    n.set("insertBefore", insert_before)?;
    n.set("removeChild", remove_child)?;
    n.set("removeNode", remove_node)?;
    n.set("replaceChild", replace_child)?;
    n.set("replaceWithText", replace_with_text)?;
    n.set("cloneNode", clone_node)
}

/// The removed subtrees of a change: for each of `candidates` that has no parent after the change,
/// the ids of its subtree in pre-order with the node first. A candidate that has a parent, or is
/// unknown, is skipped, so a node the change inserted is never listed.
pub(crate) fn removed_subtrees<'js>(
    ctx: &Ctx<'js>,
    tree: &VirtualTree,
    candidates: &[NodeId],
) -> JsResult<Array<'js>> {
    let array = Array::new(ctx.clone())?;
    let tops = candidates
        .iter()
        .filter(|&&id| tree.get(id).is_some_and(|node| node.parent().is_none()));
    for (index, &id) in tops.enumerate() {
        array.set(index, tree.subtree(id))?;
    }
    Ok(array)
}

fn append_child<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<usize> {
    move |ctx, JsNodeId(parent_id), JsNodeId(node_id)| {
        insert_child(&ctx, &host, parent_id, node_id, None)
    }
}

fn insert_before<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId, Option<JsNodeId>) -> JsResult<usize> {
    move |ctx, JsNodeId(parent_id), JsNodeId(node_id), reference_id| {
        let reference_id = reference_id.map(|JsNodeId(id)| id);
        insert_child(&ctx, &host, parent_id, node_id, reference_id)
    }
}

/// Puts `node_id` in the place of `child_id` under `parent_id` and returns the removed subtrees.
fn replace_child<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId, JsNodeId) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(parent_id), JsNodeId(node_id), JsNodeId(child_id)| {
        let mut document = host.document.borrow_mut();
        let (_, left) = document
            .tree
            .replace_child(parent_id, node_id, child_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        settle(&mut document, &*host.window.borrow(), &left);
        removed_subtrees(&ctx, &document.tree, &[child_id])
    }
}

/// Takes `child_id` out of `parent_id` and returns the removed subtrees.
fn remove_child<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id)| {
        let mut document = host.document.borrow_mut();
        let left = document
            .tree
            .remove_child(parent_id, child_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        settle(&mut document, &*host.window.borrow(), &left);
        removed_subtrees(&ctx, &document.tree, &[child_id])
    }
}

/// Replaces the children of `node_id` with one text node holding `text`, or with none for an empty
/// `text`. Returns `[textId | null, removedSubtrees]`.
fn replace_with_text<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(node_id), text| {
        let nodes = if text.is_empty() {
            Vec::new()
        } else {
            vec![InsertArg::Text(text)]
        };
        let inserted = insert_planned(&ctx, &host, node_id, Placement::ReplaceChildren, &nodes)?;
        let array = Array::new(ctx.clone())?;
        let (text_id, removed) = inserted.map_or((None, Vec::new()), |made| {
            (made.texts.first().copied(), made.removed)
        });
        match text_id {
            Some(id) => array.set(0, id)?,
            None => array.set(0, Value::new_null(ctx.clone()))?,
        }
        array.set(
            1,
            removed_subtrees(&ctx, &host.document.borrow().tree, &removed)?,
        )?;
        Ok(array)
    }
}

/// Takes `node_id` out of its parent when it has one, holds it, drops the listeners of its
/// subtree and returns `[callbackIds, nodeIds]`: the callback ids of those listeners and the ids
/// of the subtree in pre-order. An unknown id returns two empty lists.
fn remove_node<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(node_id)| {
        let mut document = host.document.borrow_mut();
        let Some(parent_id) = document
            .tree
            .get(node_id)
            .map(inca_node::VirtualNode::parent)
        else {
            return pair(&ctx, &[], &[]);
        };
        if let Some(parent_id) = parent_id {
            let left = document
                .tree
                .remove_child(parent_id, node_id)
                .map_err(|err| throw_tree_error(&ctx, err))?;
            settle(&mut document, &*host.window.borrow(), &left);
        }
        document
            .tree
            .hold(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let node_ids = document.tree.subtree(node_id);
        let callback_ids = document.listeners.release_nodes(&node_ids);
        crate::abort::settle_freed(&ctx, &mut document)?;
        callbacks::release(&ctx, &callback_ids)?;
        pair(&ctx, &callback_ids, &node_ids)
    }
}

/// The array `[first, second]` of two id lists.
fn pair<'js>(ctx: &Ctx<'js>, first: &[u32], second: &[u32]) -> JsResult<Array<'js>> {
    let array = Array::new(ctx.clone())?;
    array.set(0, first.to_vec())?;
    array.set(1, second.to_vec())?;
    Ok(array)
}

/// Inserts `node_id` under `parent_id` before `reference_id`, or last without one, and returns
/// the position of the first inserted node.
fn insert_child(
    ctx: &Ctx<'_>,
    host: &Host,
    parent_id: NodeId,
    node_id: NodeId,
    reference_id: Option<NodeId>,
) -> JsResult<usize> {
    let mut document = host.document.borrow_mut();
    let (position, left) = document
        .tree
        .insert_before(parent_id, node_id, reference_id)
        .map_err(|err| throw_tree_error(ctx, err))?;
    settle(&mut document, &*host.window.borrow(), &left);
    Ok(position)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use rstest::rstest;

    use crate::test_support::{TestView, callback_ids, error_name};
    use crate::tests::{callback_stored, engine_with_family, engine_with_family_view};
    use inca_document::EventTargetId;

    #[rstest]
    #[case::append_under_text("n.appendChild(t, loose)", "HierarchyRequestError")]
    #[case::cycle("n.appendChild(a, p)", "HierarchyRequestError")]
    #[case::root("n.appendChild(loose2, n.documentNodeId())", "HierarchyRequestError")]
    #[case::stray_reference("n.insertBefore(p, loose, x)", "NotFoundError")]
    #[case::remove_a_non_child("n.removeChild(p, x)", "NotFoundError")]
    #[case::append_unknown_parent("n.appendChild(999, a)", "TypeError")]
    #[case::remove_unknown_parent("n.removeChild(999, a)", "TypeError")]
    fn a_native_throws_the_error_of_its_failed_check(#[case] call: &str, #[case] name: &str) {
        let (engine, _host) = engine_with_family();

        assert_eq!(error_name(&engine, call), name);
    }

    #[test]
    fn remove_child_ignores_an_unknown_child() {
        let (engine, host) = engine_with_family();
        host.document.borrow_mut().take_tab_dirty();

        assert_eq!(error_name(&engine, "n.removeChild(p, 999)"), "none");

        assert!(host.document.borrow_mut().take_tab_dirty().is_empty());
    }

    #[rstest]
    #[case::append("n.appendChild(p, loose)", 2)]
    #[case::insert_in_the_middle("n.insertBefore(p, loose, b)", 1)]
    #[case::insert_with_null("n.insertBefore(p, loose, null)", 2)]
    #[case::insert_with_undefined("n.insertBefore(p, loose, undefined)", 2)]
    #[case::fragment_with_children("n.insertBefore(p, g, b)", 1)]
    #[case::empty_fragment("n.appendChild(p, f)", 2)]
    fn a_native_returns_the_position_of_the_inserted_node(#[case] call: &str, #[case] want: u32) {
        let (engine, _host) = engine_with_family();

        let position: u32 = engine.eval(call).unwrap();

        assert_eq!(position, want);
    }

    #[test]
    fn a_fragment_insert_queues_a_focus_check_for_each_child() {
        let (engine, host) = engine_with_family();
        engine
            .eval::<()>(
                r"
                n.setAttribute(g1, 'tabindex', 0);
                n.setAttribute(g2, 'tabindex', 0);
                ",
            )
            .unwrap();
        host.document.borrow_mut().take_tab_dirty();
        engine.eval::<()>("n.appendChild(p, g);").unwrap();
        let ids: Vec<u32> = engine.eval("[g1, g2]").unwrap();

        let queued = host.document.borrow_mut().take_tab_dirty();

        assert!(ids.iter().all(|id| queued.contains(id)));
    }

    #[test]
    fn replace_child_queues_a_focus_check_for_the_replaced_child() {
        let (engine, host) = engine_with_family();
        engine
            .eval::<()>(
                r"
                n.setAttribute(a, 'tabindex', 0);
                n.replaceChild(p, loose, a);
                ",
            )
            .unwrap();
        let a: u32 = engine.eval("a").unwrap();

        assert!(host.document.borrow_mut().take_tab_dirty().contains(&a));
    }

    #[test]
    fn a_refused_remove_child_queues_no_focus_check() {
        let (engine, host) = engine_with_family();
        engine
            .eval::<()>(
                r"
                n.setAttribute(x, 'tabindex', 0);
                ",
            )
            .unwrap();
        host.document.borrow_mut().take_tab_dirty();

        assert_eq!(error_name(&engine, "n.removeChild(p, x)"), "NotFoundError");

        assert!(host.document.borrow_mut().take_tab_dirty().is_empty());
        let x: u32 = engine.eval("x").unwrap();
        let host = host.document.borrow();
        assert!(host.tree.get(x).unwrap().parent().is_some());
    }

    #[test]
    fn a_listener_of_a_removed_node_fires_after_re_appending() {
        let (engine, _host) = engine_with_family();

        let fired: u32 = engine
            .eval(
                r"
                globalThis.count = 0;
                __inca_test_listen__(a, 'ping', () => {
                    count += 1;
                });
                n.removeChild(p, a);
                n.appendChild(p, a);
                n.dispatchEvent(a, new Event('ping'));
                count;
                ",
            )
            .unwrap();

        assert_eq!(fired, 1);
    }

    fn zero() -> (f32, f32) {
        (0., 0.)
    }

    /// The offset the view holds for the node `id`.
    fn offset_of(view: &Rc<RefCell<TestView>>, id: u32) -> (f32, f32) {
        view.borrow()
            .offsets
            .borrow()
            .get(&id)
            .copied()
            .unwrap_or_default()
    }

    /// Scrolls `div` and `inner` (a child of `div`) to a non-zero offset.
    fn scrolled(view: &Rc<RefCell<TestView>>, ids: &[u32]) {
        for &id in ids {
            view.borrow().offsets.borrow_mut().insert(id, (3., 40.));
        }
    }

    #[rstest]
    #[case::removed("n.removeChild(p, a)")]
    #[case::moved("n.appendChild(q, a)")]
    #[case::replaced("n.replaceChild(p, loose, a)")]
    #[case::taken_out("n.removeNode(a)")]
    #[case::inserted_elsewhere("n.insertNodes(q, 'append', [a])")]
    #[case::replaced_children("n.insertNodes(p, 'replaceChildren', [])")]
    #[case::replaced_with_nothing("n.insertNodes(a, 'replaceWith', [])")]
    fn a_scrolled_node_and_its_scrolled_descendant_read_zero(#[case] call: &str) {
        let (engine, _host, view) = engine_with_family_view();
        engine
            .eval::<()>(
                r"
                globalThis.inner = n.createNode('div');
                n.appendChild(a, inner);
                ",
            )
            .unwrap();
        let ids: Vec<u32> = engine.eval("[a, inner]").unwrap();
        scrolled(&view, &ids);

        engine.eval::<()>(call).unwrap();

        assert_eq!(offset_of(&view, ids[0]), zero());
        assert_eq!(offset_of(&view, ids[1]), zero());
    }

    #[test]
    fn a_scrolled_node_that_stays_keeps_its_offset() {
        let (engine, _host, view) = engine_with_family_view();
        let ids: Vec<u32> = engine.eval("[a, b]").unwrap();
        scrolled(&view, &ids);

        engine.eval::<()>("n.removeChild(p, a)").unwrap();

        assert_ne!(offset_of(&view, ids[1]), zero());
    }

    #[test]
    fn a_fragment_move_resets_the_scroll_offset_of_its_children() {
        let (engine, _host, view) = engine_with_family_view();
        let ids: Vec<u32> = engine.eval("[g1]").unwrap();
        scrolled(&view, &ids);

        engine.eval::<()>("n.appendChild(p, g)").unwrap();

        assert_eq!(offset_of(&view, ids[0]), zero());
    }

    /// `top > (kid > deep, sibling)`, each node a global, with a `click` listener on `top`, `kid`
    /// and `deep`. `listeners` holds the three callback ids.
    const REMOVABLE: &str = r"
        globalThis.top = n.createNode('div');
        globalThis.kid = n.createNode('div');
        globalThis.deep = n.createNode('div');
        globalThis.sibling = n.createNode('div');
        n.appendChild(top, kid);
        n.appendChild(kid, deep);
        n.appendChild(top, sibling);
        globalThis.listeners = [
            n.addEventListener(top, 'click', () => {}),
            n.addEventListener(kid, 'click', () => {}),
            n.addEventListener(deep, 'click', () => {}),
        ];
        ";

    #[test]
    fn remove_node_leaves_no_parent() {
        let (engine, _host) = engine_with_family();

        let parent: Option<u32> = engine
            .eval(&format!(
                r"
                {REMOVABLE}
                n.appendChild(p, top);
                n.removeNode(top);
                n.linkedNode(top, 'parent');
                "
            ))
            .unwrap();

        assert_eq!(parent, None);
    }

    #[test]
    fn remove_node_holds_the_node_against_the_sweep() {
        let (engine, host) = engine_with_family();
        let top: u32 = engine
            .eval(&format!(
                r"
                {REMOVABLE}
                n.appendChild(p, top);
                n.removeNode(top);
                top;
                "
            ))
            .unwrap();

        host.document.borrow_mut().tree.sweep();

        assert!(host.document.borrow().tree.get(top).is_some());
    }

    #[test]
    fn remove_node_drops_the_listeners_of_the_subtree() {
        let (engine, host) = engine_with_family();
        let ids: Vec<u32> = engine
            .eval(&format!(
                r"
                {REMOVABLE}
                n.appendChild(p, top);
                n.removeNode(top);
                [top, kid, deep];
                "
            ))
            .unwrap();

        let host = host.document.borrow();
        for id in ids {
            assert!(callback_ids(&host.listeners, EventTargetId::Node(id), "click").is_empty());
        }
    }

    #[test]
    fn remove_node_releases_the_table_entries_of_the_subtree() {
        let (engine, _host) = engine_with_family();

        let ids: Vec<u32> = engine
            .eval(&format!(
                r"
                {REMOVABLE}
                n.appendChild(p, top);
                n.removeNode(top);
                listeners;
                "
            ))
            .unwrap();

        assert!(ids.iter().all(|&id| !callback_stored(&engine, id)));
    }

    #[test]
    fn remove_node_returns_the_callback_ids_of_the_subtree() {
        let (engine, _host) = engine_with_family();

        let mut lists: Vec<Vec<u32>> = engine
            .eval(&format!(
                r"
                {REMOVABLE}
                n.appendChild(p, top);
                [n.removeNode(top)[0], listeners];
                "
            ))
            .unwrap();

        let mut expected = lists.pop().unwrap();
        let mut returned = lists.pop().unwrap();
        expected.sort_unstable();
        returned.sort_unstable();
        assert_eq!(returned, expected);
    }

    #[test]
    fn remove_node_returns_the_ids_of_the_subtree_in_pre_order() {
        let (engine, _host) = engine_with_family();

        let lists: Vec<Vec<u32>> = engine
            .eval(&format!(
                r"
                {REMOVABLE}
                n.appendChild(p, top);
                [n.removeNode(top)[1], [top, kid, deep, sibling]];
                "
            ))
            .unwrap();

        assert_eq!(lists[0], lists[1]);
    }

    #[test]
    fn remove_node_includes_the_content_of_a_template() {
        let (engine, _host) = engine_with_family();

        let got: Vec<Vec<u32>> = engine
            .eval(
                r"
                const template = n.createNode('template');
                const content = n.templateContent(template);
                const inner = n.createNode('div');
                n.appendChild(content, inner);
                const id = n.addEventListener(inner, 'click', () => {});
                const [callbackIds, nodeIds] = n.removeNode(template);
                [callbackIds, [id], nodeIds, [template, content, inner]];
                ",
            )
            .unwrap();

        assert_eq!(got[0], got[1]);
        assert_eq!(got[2], got[3]);
    }

    #[test]
    fn remove_node_of_an_unknown_id_returns_two_empty_lists() {
        let (engine, _host) = engine_with_family();

        let got: String = engine.eval("JSON.stringify(n.removeNode(999))").unwrap();

        assert_eq!(got, "[[],[]]");
    }

    #[test]
    fn remove_node_drops_the_focus_requests_of_the_subtree() {
        let (engine, host) = engine_with_family();
        let kid: u32 = engine
            .eval(
                r"
                n.appendChild(n.rootNodeId(), p);
                n.setAttribute(a, 'tabindex', 0);
                n.focusNode(a);
                a;
                ",
            )
            .unwrap();
        assert_eq!(
            host.document
                .borrow()
                .focus()
                .active_node(&host.document.borrow().tree, &host.document.borrow().style),
            Some(kid)
        );

        engine
            .eval::<()>(
                r"
                n.removeNode(p);
                n.appendChild(n.rootNodeId(), p);
                ",
            )
            .unwrap();

        assert_eq!(
            host.document
                .borrow()
                .focus()
                .active_node(&host.document.borrow().tree, &host.document.borrow().style),
            None
        );
    }

    /// `a` gets the child `k` and `listed` evaluates `call` and names each returned id.
    const NAMED: &str = r"
        globalThis.k = n.createNode('i');
        n.appendChild(a, k);
        globalThis.listed = (call) => {
            const names = { [a]: 'a', [b]: 'b', [k]: 'k', [x]: 'x', [loose]: 'loose' };
            return JSON.stringify(call().map((ids) => ids.map((id) => names[id] ?? 'other')));
        };
        ";

    #[rstest]
    #[case::remove_child("() => n.removeChild(p, a)", r#"[["a","k"]]"#)]
    #[case::replace_child("() => n.replaceChild(p, loose, a)", r#"[["a","k"]]"#)]
    #[case::unknown_child_removed("() => n.removeChild(p, 999)", "[]")]
    fn a_removing_native_returns_the_pre_order_subtree_of_the_removed_node(
        #[case] call: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine_with_family();
        engine.eval::<()>(NAMED).unwrap();

        let got: String = engine.eval(&format!("listed({call})")).unwrap();

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::replace_child_with_itself("() => n.replaceChild(p, a, a)", "[]")]
    #[case::node_of_another_parent("() => n.replaceChild(p, x, b)", r#"[["b"]]"#)]
    fn a_node_the_call_inserts_is_not_listed_as_removed(
        #[case] call: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine_with_family();
        engine.eval::<()>(NAMED).unwrap();

        let got: String = engine.eval(&format!("listed({call})")).unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn replace_child_does_not_list_the_descendant_it_moves_into_the_place() {
        let (engine, _host) = engine_with_family();
        engine.eval::<()>(NAMED).unwrap();

        let got: String = engine
            .eval("listed(() => n.replaceChild(p, k, a))")
            .unwrap();

        assert_eq!(got, r#"[["a"]]"#);
    }

    #[test]
    fn a_removed_template_lists_its_content_before_its_children() {
        let (engine, _host) = engine_with_family();

        let got: String = engine
            .eval(
                r"
                const template = n.createNode('template');
                const content = n.templateContent(template);
                const inner = n.createNode('div');
                n.appendChild(content, inner);
                n.appendChild(p, template);
                const removed = n.removeChild(p, template)[0];
                JSON.stringify(removed.map((id) => [template, content, inner].indexOf(id)));
                ",
            )
            .unwrap();

        assert_eq!(got, "[0,1,2]");
    }

    #[rstest]
    #[case::text("'hi'", r#"["hi",[["a","k"],["b"]]]"#)]
    #[case::empty("''", r#"[null,[["a","k"],["b"]]]"#)]
    fn replace_with_text_returns_the_text_id_and_the_removed_subtrees(
        #[case] text: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine_with_family();
        engine.eval::<()>(NAMED).unwrap();

        let got: String = engine
            .eval(&format!(
                r"
                const [textId, removed] = n.replaceWithText(p, {text});
                const data = textId === null ? null : n.textContent(textId);
                JSON.stringify([data, JSON.parse(listed(() => removed))]);
                "
            ))
            .unwrap();

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::text("'hi'", "1")]
    #[case::empty("''", "0")]
    fn replace_with_text_leaves_the_text_node_as_the_only_child(
        #[case] text: &str,
        #[case] count: &str,
    ) {
        let (engine, _host) = engine_with_family();

        let got: String = engine
            .eval(&format!(
                "n.replaceWithText(p, {text}); String(n.childNodeIds(p).length)"
            ))
            .unwrap();

        assert_eq!(got, count);
    }

    #[test]
    fn replace_with_text_throws_for_a_lone_surrogate_and_keeps_the_children() {
        let (engine, _host) = engine_with_family();

        assert_eq!(
            error_name(&engine, r"n.replaceWithText(p, '\ud800')"),
            "TypeError"
        );

        let count: u32 = engine.eval("n.childNodeIds(p).length").unwrap();
        assert_eq!(count, 2);
    }
}
