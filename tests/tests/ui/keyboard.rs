// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Focus and keys in real `.vue` apps.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_node_focused_in_on_mounted_fires_focus(cx: &mut TestAppContext) {
    let mut h = load(cx, "keys");

    assert_eq!(text_of(&mut h, "log"), "a:focus");
}

#[gpui::test]
fn the_focused_node_receives_keydown_and_keyup(cx: &mut TestAppContext) {
    let mut h = load(cx, "keys");

    h.keystrokes("x");
    h.key_up("x");

    assert_eq!(text_of(&mut h, "log"), "a:focus a:keydown:x a:keyup:x");
}

#[gpui::test]
fn enter_fires_click_on_key_up_before_the_keyup_listeners(cx: &mut TestAppContext) {
    let mut h = load(cx, "keys");

    h.keystrokes("enter");
    assert_eq!(text_of(&mut h, "log"), "a:focus a:keydown:Enter");

    h.key_up("enter");
    assert_eq!(
        text_of(&mut h, "log"),
        "a:focus a:keydown:Enter a:click a:keyup:Enter"
    );
}

#[gpui::test]
fn focus_moves_blur_the_old_node_and_focus_the_new_one(cx: &mut TestAppContext) {
    let mut h = load(cx, "keys");

    click(&mut h, "focus-b");

    assert_eq!(text_of(&mut h, "log"), "a:focus a:blur b:focus");
}

#[gpui::test]
fn keys_reach_only_the_focused_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "keys");
    click(&mut h, "focus-b");

    h.keystrokes("y");

    let log = text_of(&mut h, "log");
    assert!(log.ends_with("b:keydown:y"), "{log}");
    assert!(!log.contains("a:keydown"), "{log}");
}

#[gpui::test]
fn blur_leaves_no_node_to_receive_keys(cx: &mut TestAppContext) {
    let mut h = load(cx, "keys");
    click(&mut h, "focus-b");
    click(&mut h, "blur-b");
    let before = text_of(&mut h, "log");

    h.keystrokes("z");

    assert!(before.ends_with("b:blur"), "{before}");
    assert_eq!(text_of(&mut h, "log"), before);
}
