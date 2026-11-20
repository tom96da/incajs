// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `scroll` event of a script write and of a wheel step.

use super::common::{appended, by_id, click, load, text_of};
use gpui::TestAppContext;

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
    let box_ = by_id(&h.snapshot(), "box").id;

    h.scroll(box_, 0.0, -30.0);

    assert_eq!(text_of(&mut h, "log"), "capture@outer(box) scroll:30");
}

#[gpui::test]
fn a_wheel_step_at_the_end_fires_no_scroll(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-events");
    let box_ = by_id(&h.snapshot(), "box").id;
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
