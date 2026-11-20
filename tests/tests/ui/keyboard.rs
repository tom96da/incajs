// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Key events and the focused node that receives them, modifier keys pressed alone and held keys.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::{Modifiers, TestAppContext};
use inca_host::harness::Harness;
use inca_test_macros::gpui_cases;

/// Loads the fixture and focuses `#box`, then empties the logs.
fn focused_box(cx: &mut TestAppContext) -> Harness {
    let mut h = load(cx, "keyboard-held-keys");
    click(&mut h, "box");
    h.keystrokes("f9");
    h
}

#[gpui::test]
fn a_node_focused_in_on_mounted_fires_focus(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard");

    assert_eq!(text_of(&mut h, "log"), "a:focus");
}

#[gpui::test]
fn the_focused_node_receives_keydown_keypress_and_keyup(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard");

    h.keystrokes("x");
    h.key_up("x");

    assert_eq!(
        text_of(&mut h, "log"),
        "a:focus a:keydown:x a:keypress:x a:keyup:x"
    );
}

#[gpui::test]
fn focus_moves_blur_the_old_node_and_focus_the_new_one(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard");

    click(&mut h, "focus-b");

    assert_eq!(text_of(&mut h, "log"), "a:focus a:blur b:focus");
}

#[gpui::test]
fn keys_reach_only_the_focused_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard");
    click(&mut h, "focus-b");

    h.keystrokes("y");

    let log = text_of(&mut h, "log");
    assert!(log.ends_with("b:keydown:y"), "{log}");
    assert!(!log.contains("a:keydown"), "{log}");
}

#[gpui::test]
fn blur_leaves_no_node_to_receive_keys(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard");
    click(&mut h, "focus-b");
    click(&mut h, "blur-b");
    let before = text_of(&mut h, "log");

    h.keystrokes("z");

    assert!(before.ends_with("b:blur"), "{before}");
    assert_eq!(text_of(&mut h, "log"), before);
}

#[gpui::test]
fn a_window_keydown_listener_hears_keys_with_nothing_focused(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard-window");

    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "log"), "window:keydown:x");
}

#[gpui::test]
fn pressing_shift_fires_one_keydown_named_shift(cx: &mut TestAppContext) {
    let mut h = focused_box(cx);

    h.set_modifiers(Modifiers::shift());

    assert_eq!(
        text_of(&mut h, "log"),
        "keydown:Shift:false:true:false:false:false:16"
    );
}

#[gpui::test]
fn releasing_shift_fires_keyup_with_shift_key_false(cx: &mut TestAppContext) {
    let mut h = focused_box(cx);
    h.set_modifiers(Modifiers::shift());
    h.keystrokes("f9");

    h.set_modifiers(Modifiers::none());

    assert_eq!(
        text_of(&mut h, "log"),
        "keyup:Shift:false:false:false:false:false:16"
    );
}

#[gpui_cases]
#[gpui::test]
#[case::control(
    Modifiers::control(),
    "keydown:Control:false:false:true:false:false:17"
)]
#[case::alt(Modifiers::alt(), "keydown:Alt:false:false:false:true:false:18")]
#[case::meta(
    Modifiers { platform: true, ..Modifiers::none() },
    "keydown:Meta:false:false:false:false:true:91"
)]
fn each_modifier_key_fires_keydown_under_its_own_name(
    cx: &mut TestAppContext,
    modifiers: Modifiers,
    expected: &str,
) {
    let mut h = focused_box(cx);

    h.set_modifiers(modifiers);

    assert_eq!(text_of(&mut h, "log"), expected);
}

#[gpui::test]
fn a_modifier_pressed_while_another_is_held_reports_both(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard-held-keys");
    click(&mut h, "box");
    h.set_modifiers(Modifiers::shift());
    h.keystrokes("f9");

    h.set_modifiers(Modifiers::control_shift());

    assert_eq!(
        text_of(&mut h, "log"),
        "keydown:Control:false:true:true:false:false:17"
    );
}

#[gpui::test]
fn a_modifier_keydown_with_nothing_focused_reaches_window(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard-held-keys");
    h.keystrokes("f9");

    h.set_modifiers(Modifiers::shift());

    assert_eq!(text_of(&mut h, "window"), "window:Shift");
}

#[gpui::test]
fn two_modifiers_in_one_change_fire_shift_before_control(cx: &mut TestAppContext) {
    let mut h = focused_box(cx);

    h.set_modifiers(Modifiers::control_shift());

    assert_eq!(
        text_of(&mut h, "log"),
        "keydown:Shift:false:true:true:false:false:16 keydown:Control:false:true:true:false:false:17"
    );
}

#[gpui::test]
fn a_held_key_marks_its_key_events_as_repeat(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard-held-keys");
    click(&mut h, "box");
    h.keystrokes("a");
    h.keystrokes("f9");

    h.key_repeat("a");

    assert_eq!(
        text_of(&mut h, "log"),
        "keydown:a:true:false:false:false:false:65 keypress:a:true:false:false:false:false:97"
    );
}

#[gpui::test]
fn a_held_key_types_its_character_at_each_repeat(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard-held-keys");
    click(&mut h, "field");
    h.keystrokes("a");

    h.key_repeat("a");
    h.key_repeat("a");

    assert_eq!(text_of(&mut h, "mirror"), "aaa");
}

#[gpui::test]
fn holding_space_on_a_button_clicks_once_at_the_release(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard-held-keys");
    click(&mut h, "btn");
    h.keystrokes("f9");

    h.keystrokes("space");
    h.key_repeat("space");
    h.key_repeat("space");
    h.key_up("space");

    assert_eq!(text_of(&mut h, "clicks"), "1");
}

#[gpui::test]
fn holding_enter_on_a_button_clicks_at_each_repeat(cx: &mut TestAppContext) {
    let mut h = load(cx, "keyboard-held-keys");
    click(&mut h, "btn");
    h.keystrokes("f9");

    h.keystrokes("enter");
    h.key_repeat("enter");
    h.key_repeat("enter");
    h.key_up("enter");

    assert_eq!(text_of(&mut h, "clicks"), "3");
}
