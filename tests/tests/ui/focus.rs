// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The focus a script and a focus listener request, read through `document.activeElement` and
//! the focus events.

use super::common::{appended, click, load, settled, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn focus_in_a_handler_is_visible_at_once(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-active");

    click(&mut h, "focus-a");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn focusing_another_node_moves_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-active");
    click(&mut h, "focus-a");

    click(&mut h, "focus-b");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn blur_gives_the_mount_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-active");
    click(&mut h, "focus-a");

    click(&mut h, "blur-a");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn focusing_a_node_with_no_tabindex_reads_the_mount_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-active");

    click(&mut h, "focus-n");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn a_click_that_focuses_a_node_is_visible_in_its_click_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-active");

    click(&mut h, "click-target");

    assert_eq!(text_of(&mut h, "result"), "true");
}

#[gpui::test]
fn tab_focus_is_visible_in_the_focus_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-active");
    click(&mut h, "focus-a");

    h.keystrokes("tab");

    assert_eq!(text_of(&mut h, "focus-result"), "true");
}

#[gpui::test]
fn removing_the_focused_node_makes_the_active_element_the_mount_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-active");
    click(&mut h, "doomed");
    click(&mut h, "probe-doomed-focused");
    assert_eq!(text_of(&mut h, "result"), "true");

    click(&mut h, "kill");
    click(&mut h, "probe-doomed");

    assert_eq!(text_of(&mut h, "result2"), "true");
}

#[gpui::test]
fn focus_requested_in_a_focus_listener_ends_on_the_requested_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-chain");

    click(&mut h, "go-a");
    settled(&mut h, 2);
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "b");
}

#[gpui::test]
fn a_focus_moved_by_a_focus_listener_fires_after_the_listener_returns(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-chain");

    click(&mut h, "go-a");
    settled(&mut h, 2);

    assert_eq!(
        text_of(&mut h, "log"),
        "focus:a request:b focusin:a blur:a focusout:a focus:b focusin:b"
    );
}

#[gpui::test]
fn blur_requested_in_a_focus_listener_ends_with_nothing_focused(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-chain");

    click(&mut h, "go-c");
    settled(&mut h, 2);
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "");
}

#[gpui::test]
fn a_blur_requested_by_a_focus_listener_fires_after_the_listener_returns(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-chain");

    click(&mut h, "go-c");
    settled(&mut h, 2);

    assert_eq!(
        text_of(&mut h, "log"),
        "focus:c request:blur focusin:c blur:c focusout:c"
    );
}

#[gpui::test]
fn a_key_after_a_focus_chain_reaches_the_node_that_ended_focused(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus-chain");
    click(&mut h, "go-a");
    settled(&mut h, 2);
    let before = text_of(&mut h, "log");

    h.keystrokes("x");

    assert_eq!(appended(&before, &text_of(&mut h, "log")), "keydown:b");
}
