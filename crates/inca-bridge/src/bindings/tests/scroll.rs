// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use gpui::{point, px};

#[test]
fn scroll_node_writes_the_given_axes_negated_and_clamps_below_zero() {
    let (engine, host) = engine_with_bindings();
    let node: u32 = engine
        .eval("globalThis.n = __inca_native__.createNode('div')")
        .unwrap();
    engine
        .eval::<()>("__inca_native__.setStyle(n, 'overflow', 'scroll')")
        .unwrap();
    let offset = |host: &Rc<RefCell<Host>>| {
        let offset = host.borrow().scroll_handle(node).offset();
        (f32::from(offset.x), f32::from(offset.y))
    };

    engine
        .eval::<()>("__inca_native__.scrollNode(n, 5, 7)")
        .unwrap();
    assert_eq!(offset(&host), (-5., -7.));
    engine
        .eval::<()>("__inca_native__.scrollNode(n, null, 9)")
        .unwrap();
    assert_eq!(offset(&host), (-5., -9.));
    engine
        .eval::<()>("__inca_native__.scrollNode(n, 3, undefined)")
        .unwrap();
    assert_eq!(offset(&host), (-3., -9.));
    engine
        .eval::<()>("__inca_native__.scrollNode(n, -4, -1)")
        .unwrap();
    assert_eq!(offset(&host), (0., 0.));
}

#[test]
fn scroll_of_and_scroll_node_raise_type_error_for_an_unknown_node() {
    let (engine, _host) = engine_with_bindings();

    let caught: Vec<bool> = engine
        .eval(
            r"
            const caught = (f) => { try { f(); } catch (e) { return e instanceof TypeError; } return false; };
            [caught(() => __inca_native__.scrollOf(999)), caught(() => __inca_native__.scrollNode(999, 1, 1))];
            ",
        )
        .unwrap();

    assert_eq!(caught, [true, true]);
}

#[test]
fn a_scroll_into_view_of_a_boxless_node_retries_on_one_frame() {
    let (engine, host) = engine_with_bindings();
    engine
        .eval::<()>("globalThis.n = __inca_native__.createNode('div'); __inca_native__.scrollIntoViewNode(n, 'start', 'start')")
        .unwrap();
    let mut host = host.borrow_mut();

    assert!(host.reveal_scrolls());
    assert!(!host.reveal_scrolls());
}

#[test]
fn destroying_a_node_drops_its_waiting_scroll_into_view() {
    let (engine, host) = engine_with_bindings();
    engine
        .eval::<()>(
            "const n = __inca_native__; const node = n.createNode('div'); \
             n.appendChild(0, node); n.scrollIntoViewNode(node, 'start', 'start'); \
             n.destroyNode(node);",
        )
        .unwrap();

    assert!(!host.borrow_mut().reveal_scrolls());
}

#[test]
fn scroll_into_view_node_rejects_an_unknown_node_and_an_unknown_alignment() {
    let (engine, _host) = engine_with_bindings();

    let caught: Vec<bool> = engine
        .eval(
            r"
            const caught = (f) => { try { f(); } catch (e) { return e instanceof TypeError; } return false; };
            const n = __inca_native__;
            const node = n.createNode('div');
            [
                caught(() => n.scrollIntoViewNode(999, 'start', 'start')),
                caught(() => n.scrollIntoViewNode(node, 'middle', 'start')),
                caught(() => n.scrollIntoViewNode(node, 'start', 'middle')),
                caught(() => n.scrollIntoViewNode(node, 'start', 'nearest')),
            ];
            ",
        )
        .unwrap();

    assert_eq!(caught, [true, true, true, false]);
}

#[test]
fn a_scroll_moves_the_recorded_boxes_below_the_container() {
    let mut host = Host::default();
    let container = host.root;
    let child = host.tree.create_node("div").unwrap();
    let grandchild = host.tree.create_node("div").unwrap();
    host.tree.append_child(container, child).unwrap();
    host.tree.append_child(child, grandchild).unwrap();
    let record = |id| {
        let layout = inca_gpui::NodeLayout {
            bounds: Bounds::new(point(px(10.0), px(50.0)), gpui::size(px(5.0), px(5.0))),
            border: px(0.0),
            scroll: point(px(1.0), px(2.0)),
        };
        host.layout.borrow_mut().insert(id, layout);
    };
    for id in [container, child, grandchild] {
        record(id);
    }

    host.shift_descendants(container, 3.0, 20.0);

    let layout = host.layout.borrow();
    let read = |id| {
        let l = layout[&id];
        (
            f32::from(l.bounds.origin.x),
            f32::from(l.bounds.origin.y),
            f32::from(l.scroll.x),
            f32::from(l.scroll.y),
        )
    };
    assert_eq!(read(container), (10.0, 50.0, 1.0, 2.0));
    assert_eq!(read(child), (7.0, 30.0, 4.0, 22.0));
    assert_eq!(read(grandchild), (7.0, 30.0, 1.0, 2.0));
}
