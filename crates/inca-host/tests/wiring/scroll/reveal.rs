// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scroll that the reveal of a focused or scrolled-to node applies to the containers above it,
//! over a real layout.

#![allow(clippy::float_cmp, clippy::unwrap_used)]

use crate::support::scroll::*;
use crate::support::{TestRoot, engine_for, mount_active};
use inca_test_macros::gpui_cases;

/// Container `box` (100 high, 5 border, 3 padding) with ten 50-high
/// `items` under a root padded by 20. `overflow` styles `box`; `nested`
/// puts `box` inside a scrolling `outer` that has 150 before and 400 after it.
fn reveal_fixture(overflow: &str, nested: bool) -> String {
    let (parent, before, after) = if nested {
        (
            "outer",
            r"
            globalThis.outer = mkBox(pad, 'scroll');
            mkSpacer(outer, 150);
            ",
            "mkSpacer(outer, 400);",
        )
    } else {
        ("pad", "", "")
    };
    format!(
        r"
            const n = __inca_native__;
            const pad = n.createNode('div');
            n.setStyle(pad, 'padding', 20);
            n.appendChild(n.rootNodeId(), pad);
            globalThis.mkBox = (parent, overflow) => {{
                const box = n.createNode('div');
                n.setStyle(box, 'width', 100);
                n.setStyle(box, 'height', 100);
                n.setStyle(box, 'border_width', 5);
                n.setStyle(box, 'padding', 3);
                n.setStyle(box, 'overflow_y', overflow);
                n.appendChild(parent, box);
                return box;
            }};
            globalThis.mkItem = (parent) => {{
                const item = n.createNode('div');
                n.setStyle(item, 'width', 50);
                n.setStyle(item, 'height', 50);
                n.setAttribute(item, 'tabindex', '0');
                n.appendChild(parent, item);
                return item;
            }};
            globalThis.mkSpacer = (parent, height) => {{
                const spacer = n.createNode('div');
                n.setStyle(spacer, 'height', height);
                n.appendChild(parent, spacer);
            }};
            // The item center minus the center of the box inside the border.
            globalThis.gap = (item, box) => {{
                const [, y, , height] = n.layoutOf(item);
                const [, by, , bh, bb] = n.layoutOf(box);
                return (y + height / 2) - (by + bb + (bh - 2 * bb) / 2);
            }};
            globalThis.top = (box) => n.scrollOf(box)[1];
            {before}
            globalThis.box = mkBox({parent}, '{overflow}');
            globalThis.items = [];
            for (let i = 0; i < 10; i++) {{
                items.push(mkItem(box));
            }}
            {after}
            "
    )
}

fn mount(cx: &mut TestAppContext, overflow: &str, nested: bool) -> Scene {
    Scene::with_bindings(cx, Rig::default(), "", &reveal_fixture(overflow, nested))
}

fn num(scene: &Scene, code: &str) -> f64 {
    scene.engine.eval::<f64>(code).unwrap()
}

fn holds(scene: &Scene, code: &str) -> bool {
    scene.engine.eval::<bool>(code).unwrap()
}

fn near(scene: &Scene, code: &str) -> bool {
    num(scene, code).abs() < 0.5
}

/// Runs `code`, then draws the frame that applies it.
fn run(scene: &mut Scene, code: &str) {
    scene.js(&format!("{{ {code} }}"));
    scene.draw();
}

#[gpui_cases]
#[gpui::test]
#[case::omitted("__inca_native__.focusNode(items[6]);")]
#[case::undefined("__inca_native__.focusNode(items[6], undefined);")]
fn focus_centers_a_node_outside_the_scroll_container(cx: &mut TestAppContext, focus: &str) {
    let mut h = mount(cx, "scroll", false);
    run(&mut h, focus);
    assert!(num(&h, "top(box)") > 0.0);
    assert!(near(&h, "gap(items[6], box)"));
}

#[gpui::test]
fn a_focus_that_prevents_scroll_keeps_the_offset(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", false);
    run(&mut h, "__inca_native__.focusNode(items[6], true);");
    assert_eq!(num(&h, "top(box)"), 0.0);
    assert!(holds(&h, "__inca_native__.focusedNode() === items[6]"));
}

#[gpui::test]
fn a_hidden_container_scrolls_to_the_focused_node(cx: &mut TestAppContext) {
    let mut h = mount(cx, "hidden", false);
    run(&mut h, "__inca_native__.focusNode(items[6]);");
    assert!(near(&h, "gap(items[6], box)"));
}

#[gpui::test]
fn tab_centers_the_node_it_focuses(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        r"
        for (let i = 1; i < 7; i++) {
            __inca_native__.setAttribute(items[i], 'tabindex', '-1');
        }
        __inca_native__.focusNode(items[0]);
        ",
    );
    h.cx.simulate_keystrokes("tab");
    h.draw();
    assert!(holds(&h, "__inca_native__.focusedNode() === items[7]"));
    assert!(near(&h, "gap(items[7], box)"));
}

#[gpui::test]
fn a_press_that_focuses_a_partly_shown_node_leaves_the_container_in_place(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", false);
    let [x, y, width, height] =
        [0, 1, 2, 3].map(|i| num(&h, &format!("__inca_native__.layoutOf(items[1])[{i}]")));
    #[allow(clippy::cast_possible_truncation)]
    let center = point(px((x + width / 2.0) as f32), px((y + height / 2.0) as f32));
    h.cx.simulate_click(center, gpui::Modifiers::none());
    h.draw();
    assert!(holds(&h, "__inca_native__.focusedNode() === items[1]"));
    assert_eq!(num(&h, "top(box)"), 0.0);
}

#[gpui::test]
fn a_node_that_gains_its_box_later_scrolls_on_the_retry_frame(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        r"
        globalThis.late = mkItem(box);
        mkSpacer(box, 300);
        __inca_native__.focusNode(late);
        ",
    );
    h.draw();
    assert!(num(&h, "top(box)") > 0.0);
    assert!(near(&h, "gap(late, box)"));
}

#[gpui::test]
fn a_scroll_into_view_call_gives_up_after_one_retry_frame(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        r"
        const n = __inca_native__;
        globalThis.late = mkItem(box);
        n.setStyle(late, 'display', 'none');
        n.scrollIntoViewNode(late, 'start', 'nearest');
        ",
    );
    h.draw();
    assert_eq!(num(&h, "top(box)"), 0.0);

    run(&mut h, "__inca_native__.removeStyle(late, 'display');");
    h.draw();
    assert_eq!(num(&h, "top(box)"), 0.0);
}

#[gpui::test]
fn focus_scrolls_every_ancestor_container(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", true);
    run(&mut h, "__inca_native__.focusNode(items[6]);");
    assert!(num(&h, "top(box)") > 0.0);
    assert!(num(&h, "top(outer)") > 0.0);
    assert!(near(&h, "gap(items[6], outer)"));
}

#[gpui::test]
fn scroll_into_view_node_aligns_the_start(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        "__inca_native__.scrollIntoViewNode(items[6], 'start', 'nearest');",
    );
    assert!(near(
        &h,
        r"
        (() => {
            const [, itemY] = __inca_native__.layoutOf(items[6]);
            const [, boxY, , , border] = __inca_native__.layoutOf(box);
            return itemY - (boxY + border);
        })()
        "
    ));
}

#[gpui_cases]
#[gpui::test]
#[case::drawn_node(true)]
#[case::node_gaining_its_box(false)]
fn focus_leaves_a_horizontally_partly_shown_node_in_place(
    cx: &mut TestAppContext,
    drawn_first: bool,
) {
    let mut h = mount(cx, "scroll", false);
    let create = r"
        const n = __inca_native__;
        globalThis.wide = n.createNode('div');
        n.setStyle(wide, 'width', 300);
        n.setStyle(wide, 'height', 20);
        n.setAttribute(wide, 'tabindex', '0');
        n.setStyle(box, 'overflow_x', 'scroll');
        n.insertBefore(box, wide, items[0]);
        ";
    let focus = "__inca_native__.focusNode(wide);";
    if drawn_first {
        run(&mut h, create);
        h.draw();
        run(&mut h, focus);
    } else {
        run(&mut h, &format!("{create}{focus}"));
    }
    h.draw();
    assert_eq!(num(&h, "__inca_native__.scrollOf(box)[0]"), 0.0);
    assert!(holds(&h, "__inca_native__.scrollOf(box)[2] > 0"));
}

#[gpui::test]
fn a_focus_after_scroll_into_view_in_one_tick_is_revealed_from_the_new_offset(
    cx: &mut TestAppContext,
) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        r"
        __inca_native__.scrollIntoViewNode(items[3], 'start', 'nearest');
        __inca_native__.focusNode(items[7]);
        ",
    );
    assert!(near(&h, "gap(items[7], box)"));
}

#[gpui::test]
fn focus_centers_a_node_outside_the_container_horizontally(cx: &mut TestAppContext) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        r"
        const n = __inca_native__;
        globalThis.far = n.createNode('div');
        n.setStyle(far, 'width', 20);
        n.setStyle(far, 'height', 20);
        n.setStyle(far, 'margin_left', 200);
        globalThis.wide = n.createNode('div');
        n.setStyle(wide, 'width', 600);
        n.setStyle(wide, 'height', 10);
        n.appendChild(box, wide);
        n.setAttribute(far, 'tabindex', '0');
        n.setStyle(box, 'overflow_x', 'scroll');
        n.insertBefore(box, far, items[0]);
        ",
    );
    h.draw();
    run(&mut h, "__inca_native__.focusNode(far);");
    assert!(num(&h, "__inca_native__.scrollOf(box)[0]") > 0.0);
    assert!(near(
        &h,
        r"
        (() => {
            const [x, , w] = __inca_native__.layoutOf(far);
            const [bx, , bw, , bb] = __inca_native__.layoutOf(box);
            return (x + w / 2) - (bx + bb + (bw - 2 * bb) / 2);
        })()
        "
    ));
}

#[gpui_cases]
#[gpui::test]
#[case::focus("__inca_native__.focusNode(late);")]
#[case::scroll_into_view("__inca_native__.scrollIntoViewNode(late, 'start', 'nearest');")]
fn a_pending_retry_asks_for_one_more_frame(cx: &mut TestAppContext, request: &str) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        &format!(
            r"
            globalThis.late = mkItem(box);
            {request}
            "
        ),
    );
    let pending = h.cx.update(gpui::Window::simulate_next_frame);
    h.draw();
    let after_retry = h.cx.update(gpui::Window::simulate_next_frame);
    assert_eq!(pending, 1);
    assert_eq!(after_retry, 0);
}

#[gpui::test]
fn scroll_into_view_aligns_to_the_client_rectangle_beside_a_strip(cx: &mut TestAppContext) {
    let mut h = Scene::in_mode(
        cx,
        Rig::default(),
        &reveal_fixture("scroll", false),
        Scrollbars::Classic,
    );
    run(
        &mut h,
        "__inca_native__.setStyle(box, 'overflow_x', 'scroll');",
    );
    run(
        &mut h,
        "__inca_native__.scrollIntoViewNode(items[1], 'nearest', 'nearest');",
    );
    // The item ends 103 below the padding edge. The client height is 100 less the two borders
    // and the 15 of the horizontal strip.
    assert_eq!(num(&h, "top(box)"), 28.0);
}

/// A focus of `items[6]` in `box` and a `scrollIntoView` of a new item at the end of a second
/// container, queued in one frame and followed by `then`, applied by two frames: the new item has
/// its box at the second.
#[gpui_cases]
#[gpui::test]
#[case::both_run("", true)]
#[case::a_later_focus_clears_only_the_focus_reveal("n.focusNode(items[1], true);", false)]
fn a_focus_reveal_and_a_scroll_into_view_of_one_frame(
    cx: &mut TestAppContext,
    then: &str,
    focus_scrolls: bool,
) {
    let mut h = mount(cx, "scroll", false);
    run(
        &mut h,
        r"
        globalThis.other = mkBox(pad, 'scroll');
        for (let i = 0; i < 10; i++) {
            mkItem(other);
        }
        ",
    );
    run(
        &mut h,
        &format!(
            r"
            const n = __inca_native__;
            n.focusNode(items[6]);
            n.scrollIntoViewNode(mkItem(other), 'start', 'start');
            {then}
            "
        ),
    );
    h.draw();

    assert_eq!(num(&h, "top(box)") > 0.0, focus_scrolls);
    assert!(num(&h, "top(other)") > 0.0);
}

#[test]
fn a_scroll_into_view_of_a_boxless_node_retries_on_one_frame() {
    let host = Rig::default();
    let engine = host.engine();
    engine
            .eval::<()>("globalThis.n = __inca_native__.createNode('div'); __inca_native__.scrollIntoViewNode(n, 'start', 'start')")
            .unwrap();

    assert!(host.cells.reveal_all());
    assert!(!host.cells.reveal_all());
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

    assert!(!host.cells.reveal_all());
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
