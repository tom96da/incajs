// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scrolling a container by wheel and by script, and the `scroll` event it fires.

#![allow(clippy::unwrap_used)]

use super::common::{appended, click, load, near, node_id, text_of, top_in, x, y};
use gpui::{Modifiers, ScrollDelta, TestAppContext, point};

#[gpui::test]
fn scrolling_down_moves_the_children_up_by_the_delta(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let (first, second) = (y(&mut h, "first"), y(&mut h, "second"));
    let box_ = node_id(&mut h, "box");

    h.scroll(box_, 0.0, -30.0);

    near(y(&mut h, "first"), first - 30.0);
    near(y(&mut h, "second"), second - 30.0);
}

#[gpui::test]
fn scrolling_right_moves_the_children_left(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let first = x(&mut h, "wide-first");
    let wide = node_id(&mut h, "wide");

    h.scroll(wide, -30.0, 0.0);

    near(x(&mut h, "wide-first"), first - 30.0);
}

#[gpui::test]
fn scrolling_back_restores_the_children(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let first = y(&mut h, "first");
    let box_ = node_id(&mut h, "box");

    h.scroll(box_, 0.0, -30.0);
    h.scroll(box_, 0.0, 30.0);

    near(y(&mut h, "first"), first);
}

#[gpui::test]
fn scrolling_stops_at_the_end_of_the_content(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let first = y(&mut h, "first");
    let box_ = node_id(&mut h, "box");

    h.scroll(box_, 0.0, -500.0);

    // 180 of content in a 100 box with a 15 horizontal strip leaves 95 to scroll.
    near(y(&mut h, "first"), first - 95.0);
}

#[gpui::test]
fn a_wheel_handler_sees_the_scroll_delta(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    assert_eq!(text_of(&mut h, "seen"), "none");
    let box_ = node_id(&mut h, "box");

    h.scroll(box_, 0.0, -30.0);

    assert_eq!(text_of(&mut h, "seen"), "30");
}

#[gpui::test]
fn prevent_default_on_wheel_keeps_the_container_from_scrolling(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let first = y(&mut h, "locked-first");
    let locked = node_id(&mut h, "locked");

    h.scroll(locked, 0.0, -30.0);

    near(y(&mut h, "locked-first"), first);
}

#[gpui::test]
fn script_scroll_top_moves_the_children_up(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-position");
    let first = y(&mut h, "first");

    click(&mut h, "scroll40");

    near(y(&mut h, "first"), first - 40.0);
}

#[gpui::test]
fn a_wheel_step_scrolls_the_innermost_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-wheel-chain");
    let (item, tail) = (y(&mut h, "i0-1"), y(&mut h, "tail-1"));
    let inner = node_id(&mut h, "inner-1");

    h.scroll(inner, 0.0, -30.0);

    near(y(&mut h, "i0-1"), item - 30.0);
    near(y(&mut h, "tail-1"), tail);
}

#[gpui::test]
fn a_wheel_step_over_an_inner_container_at_its_end_scrolls_the_outer(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-wheel-chain");
    let (item, tail) = (y(&mut h, "i0-1"), y(&mut h, "tail-1"));
    click(&mut h, "end-1");
    let inner = node_id(&mut h, "inner-1");

    h.scroll(inner, 0.0, -30.0);

    near(y(&mut h, "tail-1"), tail - 30.0);
    near(y(&mut h, "i0-1"), item - 90.0);
}

#[gpui::test]
fn prevent_default_in_an_ancestor_listener_cancels_the_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-wheel-chain");
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
    let mut h = load(cx, "scroll-wheel-chain");
    let item = y(&mut h, "i0-3");
    let inner = node_id(&mut h, "inner-3");

    h.scroll(inner, 0.0, -30.0);

    assert_eq!(text_of(&mut h, "log"), "wheel@inner-3:0:30:0");
    near(y(&mut h, "i0-3"), item - 30.0);
}

#[gpui::test]
fn a_passive_listener_cannot_cancel_the_wheel(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-wheel-chain");
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
    let mut h = load(cx, "scroll-wheel-chain");
    let inner = node_id(&mut h, "inner-1");

    h.scroll(inner, -25.0, 0.0);

    assert_eq!(
        text_of(&mut h, "log"),
        "wheel@inner-1:25:0:0 wheel@outer-1:25:0:0"
    );
}

#[gpui::test]
fn a_horizontal_wheel_step_scrolls_a_vertical_only_container_down(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-wheel-chain");
    let vonly = node_id(&mut h, "vonly");

    h.scroll(vonly, -25.0, 0.0);

    near(top_in(&mut h, "v0", "vonly"), -25.0);
}

#[gpui::test]
fn an_upward_wheel_step_at_the_start_of_the_inner_container_scrolls_the_outer(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "scroll-wheel-chain");
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
    let mut h = load(cx, "scroll-wheel-passive");
    let content = y(&mut h, "content");
    let scroller = node_id(&mut h, "box");

    h.scroll(scroller, 0.0, -30.0);

    near(y(&mut h, "content"), content - 30.0);
}

#[gpui::test]
fn a_line_wheel_down_moves_the_content_up(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let before = y(&mut h, "first");
    let id = node_id(&mut h, "box");

    h.scroll_with(id, ScrollDelta::Lines(point(0.0, -1.0)), Modifiers::none());

    assert!(y(&mut h, "first") < before);
}

#[gpui::test]
fn a_shift_wheel_scrolls_the_vertical_axis(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let before = y(&mut h, "cell");
    let id = node_id(&mut h, "grid");

    h.scroll_with(
        id,
        ScrollDelta::Pixels(point(gpui::px(0.0), gpui::px(-30.0))),
        Modifiers::shift(),
    );

    near(before - y(&mut h, "cell"), 30.0);
}

#[gpui::test]
fn a_script_scroll_write_fires_scroll_after_the_writing_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");

    click(&mut h, "set40");

    assert_eq!(text_of(&mut h, "log"), "write capture@outer(box) scroll:40");
}

#[gpui::test]
fn writing_the_same_offset_twice_in_one_handler_fires_one_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");

    click(&mut h, "set40-twice");

    assert_eq!(
        text_of(&mut h, "log"),
        "write-1 write-2 capture@outer(box) scroll:40"
    );
}

#[gpui::test]
fn a_write_past_the_end_fires_scroll_with_the_clamped_offset(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");

    click(&mut h, "set999");

    assert_eq!(text_of(&mut h, "log"), "write capture@outer(box) scroll:80");
}

#[gpui::test]
fn a_wheel_step_fires_scroll_with_the_new_offset(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");
    let box_ = node_id(&mut h, "box");

    h.scroll(box_, 0.0, -30.0);

    assert_eq!(text_of(&mut h, "log"), "capture@outer(box) scroll:30");
}

#[gpui::test]
fn a_wheel_step_at_the_end_fires_no_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");
    let box_ = node_id(&mut h, "box");
    h.scroll(box_, 0.0, -500.0);
    let before = text_of(&mut h, "log");

    h.scroll(box_, 0.0, -30.0);

    assert_eq!(appended(&before, &text_of(&mut h, "log")), "");
}

#[gpui::test]
fn an_ancestor_bubble_listener_does_not_hear_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");

    click(&mut h, "set40");

    assert_eq!(text_of(&mut h, "bub"), "");
}

#[gpui::test]
fn writing_the_current_offset_in_a_later_click_fires_no_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");
    click(&mut h, "set40");
    let before = text_of(&mut h, "log");

    click(&mut h, "set40");

    assert_eq!(appended(&before, &text_of(&mut h, "log")), "write");
}
