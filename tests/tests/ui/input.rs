// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Text in an `input`: typing, the editing keys, the clipboard keys and `v-model`.

#![allow(clippy::unwrap_used)]

use super::common::{appended, click, click_with, load, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};
use inca_host::harness::Harness;

/// The value after `shift-home` then `shift-end` and a typed `x` over `a b c`, per platform.
const SHIFT_HOME_END_VALUE: &str = if cfg!(target_os = "macos") {
    "x"
} else {
    "abx"
};

/// The value after a Control press inside a selection and a typed `x` over `a b`, per platform.
const CTRL_PRESS_VALUE: &str = if cfg!(target_os = "macos") {
    "x"
} else {
    "abx"
};

/// The log of Alt+S in an input, per platform.
const ALT_S_LOG: &str = if cfg!(target_os = "macos") {
    "f:keydown:ß f:keypress:ß f:beforeinput:insertText:ß f:input:insertText:ß"
} else {
    "f:keydown:ß f:beforeinput:insertText:ß f:input:insertText:ß"
};

/// Mounts the fixture, focuses the node `id` and sends `keys` to it.
fn typed_in(cx: &mut TestAppContext, id: &str, keys: &str) -> Harness {
    let mut h = load(cx, "input-edit");
    click(&mut h, id);
    if !keys.is_empty() {
        h.keystrokes(keys);
    }
    h
}

/// Sends `keys` and returns what they added to `#log`.
fn added_by(h: &mut Harness, keys: &str) -> String {
    let before = text_of(h, "log");
    h.keystrokes(keys);
    appended(&before, &text_of(h, "log")).to_owned()
}

/// Clicks `id` and returns what the click added to `#log`.
fn added_by_click(h: &mut Harness, id: &str) -> String {
    let before = text_of(h, "log");
    click(h, id);
    appended(&before, &text_of(h, "log")).to_owned()
}

#[gpui::test]
fn typing_into_a_v_model_input_updates_the_model(cx: &mut TestAppContext) {
    let mut h = load(cx, "input");
    click(&mut h, "field");

    h.keystrokes("h i");

    assert_eq!(text_of(&mut h, "mirror"), "hi");
}

#[gpui::test]
fn a_model_change_rewrites_the_input(cx: &mut TestAppContext) {
    let mut h = load(cx, "input");
    click(&mut h, "field");
    h.keystrokes("h i");
    click(&mut h, "clear");
    click(&mut h, "field");

    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "mirror"), "x");
}

#[gpui::test]
fn a_typed_character_fires_the_key_and_input_events_in_order(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a");
    h.key_up("a");

    assert_eq!(
        text_of(&mut h, "log"),
        "f:keydown:a f:keypress:a f:beforeinput:insertText:a f:input:insertText:a f:keyup:a"
    );
    assert_eq!(text_of(&mut h, "value"), "a");
}

#[gpui::test]
fn a_canceled_beforeinput_leaves_the_value_and_fires_no_input(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "lock", "x");

    assert_eq!(
        text_of(&mut h, "log"),
        "lock:keydown:x lock:keypress:x lock:beforeinput:insertText:x"
    );
}

#[gpui::test]
fn backspace_removes_the_character_before_the_caret(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b");

    assert_eq!(
        added_by(&mut h, "backspace"),
        concat!(
            "f:keydown:Backspace f:beforeinput:deleteContentBackward:null ",
            "f:input:deleteContentBackward:null"
        )
    );
    assert_eq!(text_of(&mut h, "value"), "a");
}

#[gpui::test]
fn backspace_at_the_start_fires_no_input(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b home");

    assert_eq!(
        added_by(&mut h, "backspace"),
        "f:keydown:Backspace f:beforeinput:deleteContentBackward:null"
    );
    assert_eq!(text_of(&mut h, "value"), "ab");
}

#[gpui::test]
fn delete_removes_the_character_after_the_caret(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b home");

    assert_eq!(
        added_by(&mut h, "delete"),
        concat!(
            "f:keydown:Delete f:beforeinput:deleteContentForward:null ",
            "f:input:deleteContentForward:null"
        )
    );
    assert_eq!(text_of(&mut h, "value"), "b");
}

#[gpui::test]
fn delete_at_the_end_fires_no_input(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b");

    assert_eq!(
        added_by(&mut h, "delete"),
        "f:keydown:Delete f:beforeinput:deleteContentForward:null"
    );
}

#[gpui::test]
fn the_left_arrow_moves_the_caret_before_typing(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b left x");

    assert_eq!(text_of(&mut h, "value"), "axb");
}

#[gpui::test]
fn the_right_arrow_moves_the_caret_forward(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b left right x");

    assert_eq!(text_of(&mut h, "value"), "abx");
}

#[gpui::test]
fn end_moves_the_caret_to_the_end(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b home end x");

    assert_eq!(text_of(&mut h, "value"), "abx");
}

#[gpui::test]
fn up_moves_the_caret_to_the_start(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b up x");

    assert_eq!(text_of(&mut h, "value"), "xab");
}

#[gpui::test]
fn down_moves_the_caret_to_the_end(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b home down x");

    assert_eq!(text_of(&mut h, "value"), "abx");
}

#[gpui::test]
fn typing_over_a_shift_left_selection_replaces_it(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b c");

    assert_eq!(
        added_by(&mut h, "shift-left x"),
        concat!(
            "f:keydown:ArrowLeft f:keydown:x f:keypress:x f:beforeinput:insertText:x ",
            "f:input:insertText:x"
        )
    );
    assert_eq!(text_of(&mut h, "value"), "abx");
}

#[gpui::test]
fn shift_home_then_shift_end_selects_what_the_platform_keeps(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b c left shift-home shift-end x");

    assert_eq!(text_of(&mut h, "value"), SHIFT_HOME_END_VALUE);
}

#[gpui::test]
fn select_all_then_backspace_clears_the_value(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b secondary-a backspace");

    assert_eq!(text_of(&mut h, "value"), "");
}

#[gpui::test]
fn paste_inserts_the_copied_text_at_the_caret(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b secondary-a secondary-c right");

    assert_eq!(
        added_by(&mut h, "secondary-v"),
        "f:keydown:v f:beforeinput:insertFromPaste:ab f:input:insertFromPaste:ab"
    );
    assert_eq!(text_of(&mut h, "value"), "abab");
}

#[gpui::test]
fn an_edited_input_fires_change_before_blur_when_it_loses_focus(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a");

    assert_eq!(added_by_click(&mut h, "other"), "f:change f:blur");
}

#[gpui::test]
fn an_unedited_input_fires_no_change_on_blur(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a");
    click(&mut h, "other");
    click(&mut h, "f");

    assert_eq!(added_by_click(&mut h, "other"), "f:blur");
}

#[gpui::test]
fn enter_in_an_edited_input_fires_beforeinput_then_change(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a");

    assert_eq!(
        added_by(&mut h, "enter"),
        "f:keydown:Enter f:keypress:Enter f:beforeinput:insertLineBreak:null f:change"
    );
}

#[gpui::test]
fn enter_in_an_input_unedited_since_the_last_change_fires_no_change(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a enter");

    assert_eq!(
        added_by(&mut h, "enter"),
        "f:keydown:Enter f:keypress:Enter f:beforeinput:insertLineBreak:null"
    );
}

#[gpui::test]
fn typing_in_a_readonly_input_fires_beforeinput_only(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "ro", "x");

    assert_eq!(
        text_of(&mut h, "log"),
        "ro:keydown:x ro:keypress:x ro:beforeinput:insertText:x"
    );
}

#[gpui::test]
fn backspace_in_a_readonly_input_fires_no_beforeinput(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "ro", "backspace");

    assert_eq!(text_of(&mut h, "log"), "ro:keydown:Backspace");
}

#[gpui::test]
fn a_canceled_keydown_leaves_no_keypress_and_no_text(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "nokd", "x");

    assert_eq!(text_of(&mut h, "log"), "nokd:keydown:x");
}

#[gpui::test]
fn a_canceled_keypress_leaves_no_text(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "nokp", "x");

    assert_eq!(text_of(&mut h, "log"), "nokp:keydown:x nokp:keypress:x");
}

#[gpui::test]
fn a_canceled_keypress_in_a_readonly_input_fires_no_beforeinput(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "ro-nokp", "x");

    assert_eq!(
        text_of(&mut h, "log"),
        "ro-nokp:keydown:x ro-nokp:keypress:x"
    );
}

#[gpui::test]
fn cut_in_a_readonly_input_fires_no_beforeinput(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "ro", "secondary-a");

    assert_eq!(added_by(&mut h, "secondary-x"), "ro:keydown:x");
}

#[gpui::test]
fn a_click_inside_an_input_places_the_caret(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b");
    click(&mut h, "f");
    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "value"), "abx");
}

#[gpui::test]
fn a_control_press_inside_a_selection_keeps_what_the_platform_keeps(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "a b secondary-a");
    click_with(&mut h, "f", MouseButton::Left, Modifiers::control(), 1);
    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "value"), CTRL_PRESS_VALUE);
}

#[gpui::test]
fn alt_with_a_character_key_fires_keypress_where_the_platform_composes(cx: &mut TestAppContext) {
    let mut h = typed_in(cx, "f", "alt-s->ß");

    assert_eq!(text_of(&mut h, "log"), ALT_S_LOG);
}

#[gpui::test]
fn copy_replaces_the_text_already_on_the_clipboard(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-c");

    assert_eq!(h.clipboard().as_deref(), Some("abcd"));
}

#[gpui::test]
fn copy_with_nothing_selected_keeps_the_clipboard(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");

    h.keystrokes("end secondary-c");

    assert_eq!(h.clipboard().as_deref(), Some("keep"));
}

#[gpui::test]
fn cut_writes_the_selection_to_the_clipboard(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-x");

    assert_eq!(h.clipboard().as_deref(), Some("abcd"));
}

#[gpui::test]
fn cut_removes_the_selection_from_the_value(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-x");

    assert_eq!(text_of(&mut h, "value"), "");
}

#[gpui::test]
fn cut_fires_its_input_events_as_delete_by_cut(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
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
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("keep");
    click(&mut h, "ro");

    h.keystrokes("secondary-a secondary-x");

    assert_eq!(h.clipboard().as_deref(), Some("keep"));
}

#[gpui::test]
fn paste_inserts_the_clipboard_text_at_the_caret(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
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
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("x\ny");
    click(&mut h, "field");

    h.keystrokes("end secondary-v");

    assert_eq!(text_of(&mut h, "value"), "abcdx y");
}

#[gpui::test]
fn paste_into_a_readonly_input_fires_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("x");
    click(&mut h, "ro");
    h.keystrokes("f9");

    h.keystrokes("secondary-v");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn copy_in_a_readonly_input_writes_the_selection(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("keep");
    click(&mut h, "ro");

    h.keystrokes("secondary-a secondary-c");

    assert_eq!(h.clipboard().as_deref(), Some("abcd"));
}

#[gpui::test]
fn paste_with_a_canceled_beforeinput_keeps_the_value(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("x");
    click(&mut h, "locked");

    h.keystrokes("end secondary-v f3");

    assert_eq!(text_of(&mut h, "locked-value"), "abcd");
}

#[gpui::test]
fn paste_replaces_the_selection(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("xy");
    click(&mut h, "field");

    h.keystrokes("secondary-a secondary-v");

    assert_eq!(text_of(&mut h, "value"), "xy");
}

#[gpui::test]
fn cut_with_nothing_selected_fires_no_input_event(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("keep");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("end secondary-x");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn a_multi_line_paste_gives_beforeinput_the_line_breaks_and_input_spaces(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    h.set_clipboard("x\ny");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("end secondary-v");

    assert_eq!(text_of(&mut h, "data"), r#"beforeinput:"x\ny" input:"x y""#);
}

#[gpui::test]
fn paste_with_an_empty_clipboard_fires_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "input-clipboard");
    click(&mut h, "field");
    h.keystrokes("f9");

    h.keystrokes("secondary-a secondary-v");

    assert_eq!(text_of(&mut h, "log"), "");
}
