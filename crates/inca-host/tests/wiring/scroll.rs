// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the scroll natives do to the window: the offsets they write and the reveals they wait for.

#![allow(clippy::unwrap_used, unused_imports)]

use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_host::wiring::Rig;
use inca_test_macros::gpui_cases;

use super::support::*;

/// A node with `overflow: scroll`, the global `n`, and its id.
fn scroller() -> (Rc<inca_jsenv::Engine>, Rig, u32) {
    let host = Rig::default();
    let engine = host.engine();
    let node: u32 = engine
        .eval("globalThis.n = __inca_native__.createNode('div')")
        .unwrap();
    engine
        .eval::<()>("__inca_native__.setStyle(n, 'overflow', 'scroll')")
        .unwrap();
    (engine, host, node)
}

fn offset(host: &Rig, node: u32) -> (f32, f32) {
    let offset = host.cells.window.borrow().scroll_handle(node).offset();
    (f32::from(offset.x), f32::from(offset.y))
}

#[test]
fn scroll_node_writes_only_the_given_axes_negated() {
    let (engine, host, node) = scroller();

    engine
        .eval::<()>("__inca_native__.scrollNode(n, 5, 7)")
        .unwrap();
    assert_eq!(offset(&host, node), (-5., -7.));
    engine
        .eval::<()>("__inca_native__.scrollNode(n, null, 9)")
        .unwrap();
    assert_eq!(offset(&host, node), (-5., -9.));
    engine
        .eval::<()>("__inca_native__.scrollNode(n, 3, undefined)")
        .unwrap();
    assert_eq!(offset(&host, node), (-3., -9.));
}

#[test]
fn scroll_node_clamps_a_negative_value_to_zero() {
    let (engine, host, node) = scroller();

    engine
        .eval::<()>("__inca_native__.scrollNode(n, -4, -1)")
        .unwrap();

    assert_eq!(offset(&host, node), (0., 0.));
}

#[test]
fn a_scroll_into_view_of_a_boxless_node_retries_on_one_frame() {
    let host = Rig::default();
    let engine = host.engine();
    engine
            .eval::<()>("globalThis.n = __inca_native__.createNode('div'); __inca_native__.scrollIntoViewNode(n, 'start', 'start')")
            .unwrap();

    assert!(host.cells.reveal_scrolls());
    assert!(!host.cells.reveal_scrolls());
}

#[test]
fn destroying_a_node_drops_its_waiting_scroll_into_view() {
    let host = Rig::default();
    let engine = host.engine();
    engine
        .eval::<()>(
            r"
                const n = __inca_native__; const node = n.createNode('div');
                n.appendChild(0, node); n.scrollIntoViewNode(node, 'start', 'start');
                n.destroyNode(node);
                ",
        )
        .unwrap();

    assert!(!host.cells.reveal_scrolls());
}

/// Draws the window once more, the frame that applies a focus request.
fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<TestRoot>) {
    cx.update_window(window.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    })
    .unwrap();
}

/// Mounts a scroll container 100 high with ten focusable children 50 high under the root and
/// runs `call`, whose `el` is the seventh child, then draws the frame that applies it. Returns
/// the vertical scroll offset of the container and whether `el` is focused.
fn scroll_after(cx: &mut TestAppContext, call: &str) -> (f32, bool) {
    let host = Rig::default();
    let engine = engine_for(&host);
    let container: u32 = engine
        .eval(
            r"
                const n = __inca_native__;
                const box = n.createNode('div');
                n.setStyle(box, 'height', 100);
                n.setStyle(box, 'overflow_y', 'scroll');
                n.appendChild(n.rootNodeId(), box);
                globalThis.ids = [];
                for (let i = 0; i < 10; i++) {
                    const item = n.createNode('div');
                    n.setStyle(item, 'height', 50);
                    n.setAttribute(item, 'tabindex', '0');
                    n.appendChild(box, item);
                    ids.push(item);
                }
                globalThis.el = n.nodeHandle(ids[6])[0];
                box;
                ",
        )
        .unwrap();
    let root = host.cells.document.borrow().root();
    let window = mount_active(cx, &host, &engine, root);

    engine.eval::<()>(call).unwrap();
    draw(cx, window);

    let offset = host.cells.window.borrow().scroll_handle(container).offset();
    let focused = engine
        .eval::<bool>("__inca_native__.focusedNode() === ids[6]")
        .unwrap();
    (f32::from(offset.y), focused)
}

// The child spans 300 to 350 in a container 100 high, so its center is revealed at 275.
#[gpui_cases]
#[gpui::test]
#[case::no_argument("el.focus()")]
#[case::a_primitive("el.focus(true)")]
fn focus_scrolls_the_node_into_view(cx: &mut TestAppContext, call: &str) {
    assert_eq!(scroll_after(cx, call), (-275.0, true));
}

#[gpui::test]
fn focus_that_prevents_scroll_keeps_the_offset(cx: &mut TestAppContext) {
    assert_eq!(
        scroll_after(cx, "el.focus({ preventScroll: true })"),
        (0.0, true)
    );
}
