// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// Container `box` (100 high, 5 border, 3 padding) with ten 50-high
/// `items` under a root padded by 20. `overflow` styles `box`; `nested`
/// puts `box` inside a scrolling `outer` that has 150 before and 400 after it.
fn reveal_fixture(overflow: &str, nested: bool) -> String {
    let (parent, before, after) = if nested {
        (
            "outer",
            "globalThis.outer = mkBox(pad, 'scroll'); mkSpacer(outer, 150);",
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
            // The item centre minus the centre of the box inside the border.
            globalThis.gap = (item, box) => {{
                const [, y, , height] = n.layoutOf(item);
                const [, by, , bh, bb] = n.layoutOf(box);
                return (y + height / 2) - (by + bb + (bh - 2 * bb) / 2);
            }};
            globalThis.top = (box) => n.scrollOf(box)[1];
            {before}
            globalThis.box = mkBox({parent}, '{overflow}');
            globalThis.items = [];
            for (let i = 0; i < 10; i++) items.push(mkItem(box));
            {after}
            "
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn node_of(h: &Harness, expression: &str) -> NodeId {
    eval_f64(h, expression) as NodeId
}

fn revealing(cx: &mut TestAppContext, overflow: &str) -> Harness {
    Harness::load(cx, ENTRY, &reveal_fixture(overflow, false))
}

fn near(h: &Harness, code: &str) -> bool {
    eval_f64(h, code).abs() < 0.5
}

#[gpui::test]
fn focus_centres_a_node_outside_the_scroll_container(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    run_js(&mut h, "__inca_native__.focusNode(items[6]);");
    assert!(eval_f64(&h, "top(box)") > 0.0);
    assert!(near(&h, "gap(items[6], box)"));
}

#[gpui::test]
fn prevent_scroll_focuses_and_keeps_the_offset(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    run_js(&mut h, "__inca_native__.focusNode(items[6], true);");
    assert_eq!(eval_f64(&h, "top(box)"), 0.0);
    assert_eq!(
        eval_f64(&h, "__inca_native__.focusedNode() === items[6] ? 1 : 0"),
        1.0
    );
}

#[gpui::test]
fn a_hidden_container_scrolls_to_the_focused_node(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "hidden");
    run_js(&mut h, "__inca_native__.focusNode(items[6]);");
    assert!(near(&h, "gap(items[6], box)"));
}

#[gpui::test]
fn tab_centres_the_node_it_focuses(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    run_js(
        &mut h,
        "for (let i = 1; i < 7; i++) __inca_native__.setAttribute(items[i], 'tabindex', '-1'); \
         __inca_native__.focusNode(items[0]);",
    );
    h.keystrokes("tab");
    assert_eq!(
        eval_f64(&h, "__inca_native__.focusedNode() === items[7] ? 1 : 0"),
        1.0
    );
    assert!(near(&h, "gap(items[7], box)"));
}

#[gpui::test]
fn a_press_that_focuses_a_partly_shown_node_leaves_the_container_in_place(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    let item = node_of(&h, "items[1]");
    h.click(item);
    assert_eq!(
        eval_f64(&h, "__inca_native__.focusedNode() === items[1] ? 1 : 0"),
        1.0
    );
    assert_eq!(eval_f64(&h, "top(box)"), 0.0);
}

#[gpui::test]
fn a_node_that_gains_its_box_later_scrolls_on_the_retry_frame(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    run_js(
        &mut h,
        "globalThis.late = mkItem(box); mkSpacer(box, 300); \
         __inca_native__.focusNode(late);",
    );
    assert!(eval_f64(&h, "top(box)") > 0.0);
    assert!(near(&h, "gap(late, box)"));
}

#[gpui::test]
fn a_node_that_gets_its_box_after_the_retry_frame_keeps_the_offset(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    run_js(
        &mut h,
        "const n = __inca_native__; \
         globalThis.late = n.createNode('div'); \
         n.setStyle(late, 'width', 50); \
         n.setStyle(late, 'height', 50); \
         n.setAttribute(late, 'tabindex', '0'); \
         n.focusNode(late);",
    );
    run_js(
        &mut h,
        "mkSpacer(box, 300); __inca_native__.appendChild(box, late);",
    );
    h.settle();
    assert_eq!(eval_f64(&h, "top(box)"), 0.0);
    assert_eq!(
        eval_f64(&h, "__inca_native__.focusedNode() === late ? 1 : 0"),
        1.0
    );
}

#[gpui::test]
fn focus_scrolls_every_ancestor_container(cx: &mut TestAppContext) {
    let mut h = Harness::load(cx, ENTRY, &reveal_fixture("scroll", true));
    run_js(&mut h, "__inca_native__.focusNode(items[6]);");
    assert!(eval_f64(&h, "top(box)") > 0.0);
    assert!(eval_f64(&h, "top(outer)") > 0.0);
    assert!(near(&h, "gap(items[6], outer)"));
}

#[gpui::test]
fn scroll_into_view_node_aligns_the_start(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    run_js(
        &mut h,
        "__inca_native__.scrollIntoViewNode(items[6], 'start', 'nearest');",
    );
    assert!(near(
        &h,
        "__inca_native__.layoutOf(items[6])[1] - (__inca_native__.layoutOf(box)[1] \
         + __inca_native__.layoutOf(box)[4])"
    ));
}

#[gpui::test]
fn focus_leaves_a_horizontally_partly_shown_node_in_place(cx: &mut TestAppContext) {
    let mut h = revealing(cx, "scroll");
    run_js(
        &mut h,
        "const n = __inca_native__; \
         globalThis.wide = n.createNode('div'); \
         n.setStyle(wide, 'width', 300); \
         n.setStyle(wide, 'height', 20); \
         n.setAttribute(wide, 'tabindex', '0'); \
         n.setStyle(box, 'overflow_x', 'scroll'); \
         n.insertBefore(box, wide, items[0]); \
         n.focusNode(wide);",
    );
    assert_eq!(eval_f64(&h, "__inca_native__.scrollOf(box)[0]"), 0.0);
    assert_eq!(
        eval_f64(&h, "__inca_native__.scrollOf(box)[2] > 0 ? 1 : 0"),
        1.0
    );
}

#[gpui::test]
fn a_focus_after_scroll_into_view_in_one_tick_is_revealed_from_the_new_offset(
    cx: &mut TestAppContext,
) {
    let mut h = revealing(cx, "scroll");
    run_js(
        &mut h,
        "__inca_native__.scrollIntoViewNode(items[3], 'start', 'nearest'); \
         __inca_native__.focusNode(items[7]);",
    );
    assert!(near(&h, "gap(items[7], box)"));
}
