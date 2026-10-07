// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Event and key modifiers in real `.vue` apps.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};

#[gpui::test]
fn click_stop_keeps_the_ancestor_handler_from_running(cx: &mut TestAppContext) {
    let mut h = load(cx, "modifiers");

    click(&mut h, "inner");

    assert_eq!(text_of(&mut h, "log"), "inner");
}

#[gpui::test]
fn a_click_on_the_ancestor_alone_runs_its_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "modifiers");

    click(&mut h, "outer");

    assert_eq!(text_of(&mut h, "log"), "outer");
}

#[gpui::test]
fn keydown_enter_runs_for_enter(cx: &mut TestAppContext) {
    let mut h = load(cx, "modifiers");

    h.keystrokes("enter");

    assert_eq!(text_of(&mut h, "log"), "enter");
}

#[gpui::test]
fn keydown_enter_skips_other_keys(cx: &mut TestAppContext) {
    let mut h = load(cx, "modifiers");

    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn self_runs_only_for_the_node_itself_and_a_listener_reads_the_deep_target(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "self");

    click(&mut h, "inner");
    assert_eq!(text_of(&mut h, "log"), "");
    click(&mut h, "outer");
    click(&mut h, "kid");
    click(&mut h, "wrap");

    assert_eq!(text_of(&mut h, "log"), "self delegated wrap");
}

#[gpui::test]
fn button_and_key_modifiers_work_on_a_clicked_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "pointer");
    let boxed = by_id(&h.snapshot(), "box").id;
    let ctrl = Modifiers {
        control: true,
        ..Modifiers::none()
    };
    // (button, modifiers, click count, the log it adds)
    let steps = [
        (MouseButton::Left, Modifiers::none(), 1, "left "),
        (MouseButton::Left, ctrl, 1, "left ctrl "),
        (MouseButton::Middle, Modifiers::none(), 1, "middle "),
        (MouseButton::Right, Modifiers::none(), 1, "right menu:true "),
    ];
    let mut want = String::new();
    let mut mismatches = Vec::new();
    for (button, modifiers, count, added) in steps {
        h.click_with(boxed, button, modifiers, count);
        want += added;
        let got = text_of(&mut h, "log");
        if got != want.trim_end() {
            mismatches.push(format!("{button:?}: {got:?} != {want:?}"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:?}");
}
