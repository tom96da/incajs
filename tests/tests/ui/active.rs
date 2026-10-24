// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `document.activeElement` reads the focused node from a real `.vue` app.

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn focus_in_a_handler_is_visible_at_once(cx: &mut TestAppContext) {
    let mut h = load(cx, "active");

    click(&mut h, "focus-a");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn focusing_another_node_moves_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "active");
    click(&mut h, "focus-a");

    click(&mut h, "focus-b");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn blur_gives_null(cx: &mut TestAppContext) {
    let mut h = load(cx, "active");
    click(&mut h, "focus-a");

    click(&mut h, "blur-a");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn focusing_a_node_with_no_tabindex_reads_null(cx: &mut TestAppContext) {
    let mut h = load(cx, "active");

    click(&mut h, "focus-n");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn a_click_that_focuses_a_node_is_visible_in_its_click_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "active");

    click(&mut h, "click-target");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn tab_focus_is_visible_in_the_focus_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "active");
    click(&mut h, "focus-a");

    h.keystrokes("tab");

    assert_eq!(text_of(&mut h, "focus-result"), "true");
}

#[gpui::test]
fn removing_the_focused_node_makes_the_active_element_null(cx: &mut TestAppContext) {
    let mut h = load(cx, "active");
    click(&mut h, "doomed");
    click(&mut h, "probe-doomed-focused");
    assert_eq!(text_of(&mut h, "result"), "true");

    click(&mut h, "kill");
    click(&mut h, "probe-doomed");

    assert_eq!(text_of(&mut h, "result2"), "true");
}
