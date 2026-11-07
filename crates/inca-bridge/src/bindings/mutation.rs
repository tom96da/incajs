// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The classic mutation natives and the host steps that follow a node leaving its place.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult};

use inca_gpui::NodeId;

use super::Natives;
use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;

/// The classic mutation members: `appendChild`, `insertBefore`, `removeChild` and
/// `replaceChild`. Each moved or removed node drops the focus it held, moves the Tab start,
/// queues a focus re-check and resets the scroll offsets of its subtree.
pub(super) fn install_mutation(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("appendChild", append_child)?;
    n.set("insertBefore", insert_before)?;
    n.set("removeChild", remove_child)?;
    n.set("replaceChild", replace_child)
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
        let mut left = host.leaving(&host.tree.nodes_to_move(node_id));
        left.extend(host.leaving(&[child_id]));
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

    use crate::Host;
    use crate::test_support::{LISTEN_JS, engine_with_bindings, error_name};

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
    #[case::root("n.appendChild(loose2, n.rootNodeId())", "HierarchyRequestError")]
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
}
