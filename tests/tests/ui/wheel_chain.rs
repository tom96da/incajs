// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which container takes a wheel step when scroll containers nest, and what
//! the `wheel` listeners on the way see and can cancel.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, near, node_id, text_of, top_in, y};
use gpui::TestAppContext;

#[gpui::test]
fn a_wheel_step_scrolls_the_innermost_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-chain");
    let (item, tail) = (y(&mut h, "i0-1"), y(&mut h, "tail-1"));
    let inner = node_id(&mut h, "inner-1");

    h.scroll(inner, 0.0, -30.0);

    near(y(&mut h, "i0-1"), item - 30.0);
    near(y(&mut h, "tail-1"), tail);
}

#[gpui::test]
fn a_wheel_step_over_an_inner_container_at_its_end_scrolls_the_outer(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-chain");
    let (item, tail) = (y(&mut h, "i0-1"), y(&mut h, "tail-1"));
    click(&mut h, "end-1");
    let inner = node_id(&mut h, "inner-1");

    h.scroll(inner, 0.0, -30.0);

    near(y(&mut h, "tail-1"), tail - 30.0);
    near(y(&mut h, "i0-1"), item - 90.0);
}

#[gpui::test]
fn prevent_default_in_an_ancestor_listener_cancels_the_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-chain");
    let (item, tail) = (y(&mut h, "i0-2"), y(&mut h, "tail-2"));
    let inner = node_id(&mut h, "inner-2");

    h.scroll(inner, 0.0, -30.0);

    assert_eq!(
        text_of(&mut h, "log"),
        "wheel@inner-2:0:30:0 wheel@outer-2:0:30:0"
    );
    near(y(&mut h, "i0-2"), item);
    near(y(&mut h, "tail-2"), tail);
}

#[gpui::test]
fn stop_propagation_on_wheel_does_not_cancel_the_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-chain");
    let item = y(&mut h, "i0-3");
    let inner = node_id(&mut h, "inner-3");

    h.scroll(inner, 0.0, -30.0);

    assert_eq!(text_of(&mut h, "log"), "wheel@inner-3:0:30:0");
    near(y(&mut h, "i0-3"), item - 30.0);
}

#[gpui::test]
fn a_passive_listener_cannot_cancel_the_wheel(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-chain");
    let item = y(&mut h, "i0-4");
    let inner = node_id(&mut h, "inner-4");

    h.scroll(inner, 0.0, -30.0);

    assert_eq!(
        text_of(&mut h, "log"),
        "wheel@inner-4:0:30:0 passive:false wheel@outer-4:0:30:0"
    );
    near(y(&mut h, "i0-4"), item - 30.0);
}

#[gpui::test]
fn a_horizontal_wheel_step_reports_delta_x_with_pixel_mode(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-chain");
    let inner = node_id(&mut h, "inner-1");

    h.scroll(inner, -25.0, 0.0);

    assert_eq!(
        text_of(&mut h, "log"),
        "wheel@inner-1:25:0:0 wheel@outer-1:25:0:0"
    );
}

#[gpui::test]
fn a_horizontal_wheel_step_scrolls_a_vertical_only_container_down(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-chain");
    let vonly = node_id(&mut h, "vonly");

    h.scroll(vonly, -25.0, 0.0);

    near(top_in(&mut h, "v0", "vonly"), -25.0);
}

#[gpui::test]
fn an_upward_wheel_step_at_the_start_of_the_inner_container_scrolls_the_outer(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "wheel-chain");
    click(&mut h, "end-5");
    let (head, item) = (y(&mut h, "head-5"), y(&mut h, "i0-5"));
    let inner = node_id(&mut h, "inner-5");

    h.scroll(inner, 0.0, 30.0);

    near(y(&mut h, "head-5"), head + 30.0);
    near(y(&mut h, "i0-5"), item + 30.0);
}

#[gpui::test]
fn a_wheel_step_scrolls_a_box_under_wheel_listeners_on_document_and_window(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "wheel-passive");
    let content = y(&mut h, "content");
    let scroller = node_id(&mut h, "box");

    h.scroll(scroller, 0.0, -30.0);

    near(y(&mut h, "content"), content - 30.0);
}
