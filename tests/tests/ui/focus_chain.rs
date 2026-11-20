// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A focus request made inside a focus listener.

use super::common::{appended, click, load, settled, text_of};
use gpui::TestAppContext;

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
