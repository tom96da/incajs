// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The clipboard keys of an input in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn copy_replaces_the_text_already_on_the_clipboard(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-c");

    assert_eq!(h.clipboard().as_deref(), Some("abcd"));
}

#[gpui::test]
fn copy_with_nothing_selected_keeps_the_clipboard(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");

    h.keystrokes("end secondary-c");

    assert_eq!(h.clipboard().as_deref(), Some("keep"));
}

#[gpui::test]
fn cut_writes_the_selection_to_the_clipboard(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-x");

    assert_eq!(h.clipboard().as_deref(), Some("abcd"));
}

#[gpui::test]
fn cut_removes_the_selection_from_the_value(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-x");

    assert_eq!(text_of(&mut h, "value"), "");
}

#[gpui::test]
fn cut_fires_its_input_events_as_delete_by_cut(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("secondary-a secondary-x");

    assert_eq!(
        text_of(&mut h, "log"),
        "beforeinput:deleteByCut:null input:deleteByCut:null"
    );
}

#[gpui::test]
fn cut_in_a_readonly_input_keeps_the_clipboard(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("keep");
    click(&mut h, "ro");

    h.keystrokes("secondary-a secondary-x");

    assert_eq!(h.clipboard().as_deref(), Some("keep"));
}

#[gpui::test]
fn paste_inserts_the_clipboard_text_at_the_caret(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("xy");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("end secondary-v");

    assert_eq!(
        text_of(&mut h, "log"),
        "beforeinput:insertFromPaste:xy input:insertFromPaste:xy"
    );
}

#[gpui::test]
fn paste_turns_each_line_break_into_a_space(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("x\ny");
    click(&mut h, "field");

    h.keystrokes("end secondary-v");

    assert_eq!(text_of(&mut h, "value"), "abcdx y");
}

#[gpui::test]
fn paste_into_a_readonly_input_fires_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("x");
    click(&mut h, "ro");
    h.keystrokes("f9");

    h.keystrokes("secondary-v");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn copy_in_a_readonly_input_writes_the_selection(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("keep");
    click(&mut h, "ro");

    h.keystrokes("secondary-a secondary-c");

    assert_eq!(h.clipboard().as_deref(), Some("abcd"));
}

#[gpui::test]
fn paste_with_a_canceled_beforeinput_keeps_the_value(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("x");
    click(&mut h, "locked");

    h.keystrokes("end secondary-v f3");

    assert_eq!(text_of(&mut h, "locked-value"), "abcd");
}

#[gpui::test]
fn paste_replaces_the_selection(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("xy");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-v");

    assert_eq!(text_of(&mut h, "value"), "xy");
}

#[gpui::test]
fn cut_with_nothing_selected_fires_no_input_event(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("end secondary-x");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn a_multi_line_paste_gives_beforeinput_the_line_breaks_and_input_spaces(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    h.set_clipboard("x\ny");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("end secondary-v");

    assert_eq!(text_of(&mut h, "data"), r#"beforeinput:"x\ny" input:"x y""#);
}

#[gpui::test]
fn paste_with_an_empty_clipboard_fires_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "clipboard");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("secondary-a secondary-v");

    assert_eq!(text_of(&mut h, "log"), "");
}
