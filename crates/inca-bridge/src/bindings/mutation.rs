// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mutation natives, `removeNode` included, and the host steps that follow a node leaving
//! its place.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Array, Ctx, Exception, Result as JsResult};

use inca_jsenv::callbacks;
use inca_node::NodeId;

use super::Natives;
use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;

/// The mutation members: `appendChild`, `insertBefore`, `removeChild`, `removeNode`,
/// `replaceChild` and `cloneNode`. Each moved or removed node drops the focus it held, moves the
/// Tab start, queues a focus re-check and resets the scroll offsets of its subtree.
pub(super) fn install_mutation(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("appendChild", append_child)?;
    n.set("insertBefore", insert_before)?;
    n.set("removeChild", remove_child)?;
    n.set("removeNode", remove_node)?;
    n.set("replaceChild", replace_child)?;
    n.set("cloneNode", clone_node)
}

fn append_child<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<usize> {
    move |ctx, JsNodeId(parent_id), JsNodeId(node_id)| {
        insert_child(&ctx, &mut host.borrow_mut(), parent_id, node_id, None)
    }
}

fn insert_before<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId, Option<JsNodeId>) -> JsResult<usize> {
    move |ctx, JsNodeId(parent_id), JsNodeId(node_id), reference_id| {
        let reference_id = reference_id.map(|JsNodeId(id)| id);
        insert_child(
            &ctx,
            &mut host.borrow_mut(),
            parent_id,
            node_id,
            reference_id,
        )
    }
}

fn replace_child<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId, JsNodeId) -> JsResult<usize> {
    move |ctx, JsNodeId(parent_id), JsNodeId(node_id), JsNodeId(child_id)| {
        let mut host = host.borrow_mut();
        let mut leaving = host.tree.nodes_to_move(node_id);
        leaving.push(child_id);
        let left = host.leaving(&leaving);
        let position = host
            .tree
            .replace_child(parent_id, node_id, child_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        host.settle(&left);
        Ok(position)
    }
}

fn remove_child<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(parent_id), JsNodeId(child_id)| {
        let mut host = host.borrow_mut();
        let left = host.leaving(&[child_id]);
        host.tree
            .remove_child(parent_id, child_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        host.settle(&left);
        Ok(())
    }
}

/// Takes `node_id` out of its parent when it has one, holds it, drops the listeners of its
/// subtree and returns `[callbackIds, nodeIds]`: the callback ids of those listeners and the ids
/// of the subtree in pre-order. An unknown id returns two empty lists.
fn remove_node<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Array<'js>> {
    move |ctx, JsNodeId(node_id)| {
        let mut host = host.borrow_mut();
        let Some(parent_id) = host.tree.get(node_id).map(inca_node::VirtualNode::parent) else {
            return pair(&ctx, &[], &[]);
        };
        if let Some(parent_id) = parent_id {
            let left = host.leaving(&[node_id]);
            host.tree
                .remove_child(parent_id, node_id)
                .map_err(|err| throw_tree_error(&ctx, err))?;
            host.settle(&left);
        }
        host.tree
            .hold(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let node_ids = host.tree.subtree(node_id);
        let callback_ids: Vec<u32> = node_ids
            .iter()
            .flat_map(|&id| host.listeners.release(id))
            .collect();
        crate::abort::settle_freed(&ctx, &mut host)?;
        callbacks::release(&ctx, &callback_ids)?;
        pair(&ctx, &callback_ids, &node_ids)
    }
}

/// The array `[first, second]` of two id lists.
pub(super) fn pair<'js>(ctx: &Ctx<'js>, first: &[u32], second: &[u32]) -> JsResult<Array<'js>> {
    let array = Array::new(ctx.clone())?;
    array.set(0, first.to_vec())?;
    array.set(1, second.to_vec())?;
    Ok(array)
}

/// Copies `node_id` and, when `deep`, its descendants, and returns the id of the copy of
/// `node_id`. The copies include the content of a template. An HTML `input` copy gets the typed
/// text of its original.
fn clone_node<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<bool>) -> JsResult<NodeId> {
    move |ctx, JsNodeId(node_id), Coerced(deep)| {
        let mut host = host.borrow_mut();
        let made = host
            .tree
            .clone_subtree(node_id, deep)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let mut inputs = host.inputs.borrow_mut();
        for &(original, copy) in &made {
            let Some(node) = host.tree.get(copy) else {
                return Err(Exception::throw_type(&ctx, "the copy is missing"));
            };
            let typed = inputs.get(&original).map(|input| input.value().to_owned());
            if let (true, Some(typed)) = (node.is_html("input"), typed) {
                inputs.entry(copy).or_default().set_value(&typed);
            }
        }
        made.first()
            .map(|&(_, copy)| copy)
            .ok_or_else(|| Exception::throw_type(&ctx, "the copy is missing"))
    }
}

/// Inserts `node_id` under `parent_id` before `reference_id`, or last without one, and returns
/// the position of the first inserted node.
fn insert_child(
    ctx: &Ctx<'_>,
    host: &mut Host,
    parent_id: NodeId,
    node_id: NodeId,
    reference_id: Option<NodeId>,
) -> JsResult<usize> {
    let left = host.leaving(&host.tree.nodes_to_move(node_id));
    let position = host
        .tree
        .insert_before(parent_id, node_id, reference_id)
        .map_err(|err| throw_tree_error(ctx, err))?;
    host.settle(&left);
    Ok(position)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Point, point, px};
    use inca_jsenv::Engine;
    use rstest::rstest;

    use crate::EventTargetId;
    use crate::Host;
    use crate::test_support::{LISTEN_JS, callback_ids, engine_with_bindings, error_name};
    use crate::tests::callback_stored;

    /// Builds `p > (a, b)`, `q > x`, `g > (g1, g2)` and the parentless `loose`, `loose2`, `t`
    /// (text), `c` (comment) and `f` (empty fragment).
    const SETUP: &str = r"
        globalThis.n = __inca_native__;
        const make = (tag) => n.createNode(tag);
        globalThis.p = make('div');
        globalThis.a = make('span');
        globalThis.b = make('span');
        globalThis.q = make('div');
        globalThis.x = make('span');
        globalThis.g = make('#fragment');
        globalThis.g1 = make('i');
        globalThis.g2 = make('i');
        globalThis.loose = make('span');
        globalThis.loose2 = make('span');
        globalThis.t = make('#text');
        globalThis.c = make('#comment');
        globalThis.f = make('#fragment');
        n.appendChild(p, a);
        n.appendChild(p, b);
        n.appendChild(q, x);
        n.appendChild(g, g1);
        n.appendChild(g, g2);
        ";

    fn engine_with_family() -> (Engine, Rc<RefCell<Host>>) {
        let (engine, host) = engine_with_bindings();
        engine.eval::<()>(LISTEN_JS).unwrap();
        engine.eval::<()>(SETUP).unwrap();
        (engine, host)
    }

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
        host.borrow_mut().focus.take_tab_dirty();

        assert_eq!(error_name(&engine, "n.removeChild(p, 999)"), "none");

        assert!(host.borrow_mut().focus.take_tab_dirty().is_empty());
    }

    #[rstest]
    #[case::append("n.appendChild(p, loose)", 2)]
    #[case::insert_in_the_middle("n.insertBefore(p, loose, b)", 1)]
    #[case::insert_with_null("n.insertBefore(p, loose, null)", 2)]
    #[case::insert_with_undefined("n.insertBefore(p, loose, undefined)", 2)]
    #[case::replace("n.replaceChild(p, loose, a)", 0)]
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
        host.borrow_mut().focus.take_tab_dirty();
        engine.eval::<()>("n.appendChild(p, g);").unwrap();
        let ids: Vec<u32> = engine.eval("[g1, g2]").unwrap();

        let queued = host.borrow_mut().focus.take_tab_dirty();

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

        assert!(host.borrow_mut().focus.take_tab_dirty().contains(&a));
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
        host.borrow_mut().focus.take_tab_dirty();

        assert_eq!(error_name(&engine, "n.removeChild(p, x)"), "NotFoundError");

        assert!(host.borrow_mut().focus.take_tab_dirty().is_empty());
        let x: u32 = engine.eval("x").unwrap();
        let host = host.borrow();
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

    #[test]
    fn clone_node_returns_the_id_of_a_new_node_with_the_name_of_the_original() {
        let (engine, _host) = engine_with_family();

        let got: String = engine
            .eval(
                r"
                n.appendChild(a, t);
                const copy = n.cloneNode(a, true);
                JSON.stringify([![p, a, b, t].includes(copy), n.attributeKey(copy, 'X')]);
                ",
            )
            .unwrap();

        assert_eq!(got, r#"[true,"x"]"#);
    }

    #[test]
    fn a_deep_clone_node_copies_the_descendants() {
        let (engine, _host) = engine_with_family();

        let count: u32 = engine
            .eval(
                r"
                n.appendChild(a, t);
                n.appendChild(a, c);
                n.childNodeIds(n.cloneNode(a, true)).length;
                ",
            )
            .unwrap();

        assert_eq!(count, 2);
    }

    #[test]
    fn a_shallow_clone_node_copies_no_descendant() {
        let (engine, _host) = engine_with_family();

        let count: u32 = engine
            .eval(
                r"
                n.appendChild(a, t);
                n.appendChild(a, c);
                n.childNodeIds(n.cloneNode(a, false)).length;
                ",
            )
            .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn a_deep_clone_node_copies_the_data_of_a_text_descendant() {
        let (engine, _host) = engine_with_family();

        let data: String = engine
            .eval(
                r"
                n.setCharacterData(t, 'abc');
                n.appendChild(a, t);
                n.textContent(n.cloneNode(a, true));
                ",
            )
            .unwrap();

        assert_eq!(data, "abc");
    }

    #[test]
    fn a_clone_node_of_the_document_is_a_document_copy() {
        let (engine, _host) = engine_with_family();

        let got: bool = engine
            .eval(
                r"
                const copy = n.cloneNode(n.documentNodeId(), true);
                copy !== n.documentNodeId() && n.childNodeIds(copy).length === 1;
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[test]
    fn a_cloned_input_has_the_typed_text_of_its_original() {
        let (engine, _host) = engine_with_family();

        let texts: Vec<String> = engine
            .eval(
                r"
                const field = n.createNode('input');
                n.setInputValue(field, 'typed');
                const copy = n.cloneNode(field, false);
                [n.inputValue(copy), n.inputValue(field)];
                ",
            )
            .unwrap();

        assert_eq!(texts, ["typed", "typed"]);
    }

    #[test]
    fn a_deep_cloned_input_keeps_its_own_typed_text() {
        let (engine, _host) = engine_with_family();

        let texts: Vec<String> = engine
            .eval(
                r"
                const box = n.createNode('div');
                const one = n.createNode('input');
                const two = n.createNode('input');
                n.appendChild(box, one);
                n.appendChild(box, two);
                n.setInputValue(one, 'first');
                n.setInputValue(two, 'second');
                n.childNodeIds(n.cloneNode(box, true)).map((id) => n.inputValue(id));
                ",
            )
            .unwrap();

        assert_eq!(texts, ["first", "second"]);
    }

    #[test]
    fn clone_node_of_an_unknown_id_throws_type_error() {
        let (engine, _host) = engine_with_family();

        assert_eq!(error_name(&engine, "n.cloneNode(999, true)"), "TypeError");
    }

    fn zero() -> Point<gpui::Pixels> {
        point(px(0.), px(0.))
    }

    /// Scrolls `div` and `inner` (a child of `div`) to a non-zero offset.
    fn scrolled(host: &Rc<RefCell<Host>>, ids: &[u32]) {
        for &id in ids {
            host.borrow()
                .scroll_handle(id)
                .set_offset(point(px(-3.), px(-40.)));
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
        let (engine, host) = engine_with_family();
        engine
            .eval::<()>(
                r"
                globalThis.inner = n.createNode('div');
                n.appendChild(a, inner);
                ",
            )
            .unwrap();
        let ids: Vec<u32> = engine.eval("[a, inner]").unwrap();
        scrolled(&host, &ids);

        engine.eval::<()>(call).unwrap();

        let host = host.borrow();
        assert_eq!(host.scroll_handle(ids[0]).offset(), zero());
        assert_eq!(host.scroll_handle(ids[1]).offset(), zero());
    }

    #[test]
    fn a_scrolled_node_that_stays_keeps_its_offset() {
        let (engine, host) = engine_with_family();
        let ids: Vec<u32> = engine.eval("[a, b]").unwrap();
        scrolled(&host, &ids);

        engine.eval::<()>("n.removeChild(p, a)").unwrap();

        let host = host.borrow();
        assert_ne!(host.scroll_handle(ids[1]).offset(), zero());
    }

    #[test]
    fn a_fragment_move_resets_the_scroll_offset_of_its_children() {
        let (engine, host) = engine_with_family();
        let ids: Vec<u32> = engine.eval("[g1]").unwrap();
        scrolled(&host, &ids);

        engine.eval::<()>("n.appendChild(p, g)").unwrap();

        assert_eq!(host.borrow().scroll_handle(ids[0]).offset(), zero());
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

        host.borrow_mut().tree.sweep();

        assert!(host.borrow().tree.get(top).is_some());
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

        let host = host.borrow();
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
            host.borrow().focus.active_node(&host.borrow().tree),
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

        assert_eq!(host.borrow().focus.active_node(&host.borrow().tree), None);
    }

    #[test]
    fn clone_node_copies_the_content_fragment_of_a_template() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const template = n.createNode('template');
                const copy = n.cloneNode(template, false);
                const content = n.templateContent(copy);
                content !== null && content !== n.templateContent(template);
                ",
            )
            .unwrap();

        assert!(got);
    }
}
