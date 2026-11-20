// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Composition events of an `input` in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::{Modifiers, TestAppContext};
use inca_host::harness::Harness;

/// Mounts the fixture and focuses the input `id`, with the caret at its end.
fn composing(cx: &mut TestAppContext, id: &str) -> Harness {
    let mut h = load(cx, "composition");
    click(&mut h, id);
    h
}

const MARK_K: &str = concat!(
    "compositionstart: compositionupdate:k ",
    "beforeinput:insertCompositionText:k:true:false input:insertCompositionText:k:true:false"
);

#[gpui::test]
fn a_first_mark_starts_the_composition(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.keystrokes("f9");

    h.ime_mark("k");

    assert_eq!(text_of(&mut h, "log"), MARK_K);
}

#[gpui::test]
fn a_second_mark_updates_the_composition(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    h.ime_mark("か");

    assert_eq!(
        text_of(&mut h, "log"),
        concat!(
            "compositionupdate:か ",
            "beforeinput:insertCompositionText:か:true:false input:insertCompositionText:か:true:false"
        )
    );
}

#[gpui::test]
fn a_commit_ends_the_composition(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    h.ime_commit("か");

    assert_eq!(
        text_of(&mut h, "log"),
        concat!(
            "compositionupdate:か ",
            "beforeinput:insertCompositionText:か:true:false input:insertCompositionText:か:true:false ",
            "compositionend:か"
        )
    );
}

#[gpui::test]
fn an_empty_mark_ends_the_composition_with_no_text(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    h.ime_mark("");

    assert_eq!(
        text_of(&mut h, "log"),
        concat!(
            "compositionupdate: beforeinput:insertCompositionText::true:false ",
            "input:insertCompositionText:null:true:false compositionend:"
        )
    );
}

#[gpui::test]
fn compositionstart_carries_the_text_it_replaces(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.keystrokes("home right shift-right");
    h.keystrokes("f9");

    h.ime_mark("x");

    assert_eq!(
        text_of(&mut h, "comp"),
        "compositionstart:b compositionupdate:x"
    );
}

#[gpui::test]
fn a_keydown_during_a_composition_reports_is_composing(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    h.keystrokes("f5");

    assert_eq!(text_of(&mut h, "log"), "keydown:F5:true");
}

#[gpui::test]
fn a_modifier_keydown_during_a_composition_reports_is_composing(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    h.set_modifiers(Modifiers::shift());

    assert_eq!(text_of(&mut h, "log"), "keydown:Shift:true");
}

#[gpui::test]
fn canceling_the_beforeinput_of_a_composition_keeps_the_text(cx: &mut TestAppContext) {
    let mut h = composing(cx, "locked");

    h.ime_mark("k");

    assert_eq!(text_of(&mut h, "locked-value"), "abk");
}

#[gpui::test]
fn moving_the_focus_fires_compositionend_first(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    click(&mut h, "other");

    assert_eq!(text_of(&mut h, "log"), "compositionend:k change blur");
}

#[gpui::test]
fn an_unmark_ends_the_composition_with_compositionend_alone(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    h.ime_unmark();

    assert_eq!(text_of(&mut h, "log"), "compositionend:k");
}

#[gpui::test]
fn an_unmark_keeps_the_composed_text(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");

    h.ime_mark("k");
    h.ime_unmark();

    assert_eq!(text_of(&mut h, "value"), "abk");
}

#[gpui::test]
fn a_commit_with_no_composition_inserts_plain_text(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.keystrokes("f9");

    h.ime_commit("x");

    assert_eq!(
        text_of(&mut h, "log"),
        "beforeinput:insertText:x:false:true input:insertText:x:false:false"
    );
}

#[gpui::test]
fn a_composition_text_drops_its_line_breaks(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");

    h.ime_mark("k\nl");

    assert_eq!(text_of(&mut h, "value"), "abkl");
}

#[gpui::test]
fn an_empty_mark_with_no_composition_fires_nothing(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.keystrokes("f9");

    h.ime_mark("");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn an_empty_commit_with_no_composition_fires_nothing(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.keystrokes("secondary-a");
    h.keystrokes("f9");

    h.ime_commit("");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn canceling_compositionend_keeps_the_commit(cx: &mut TestAppContext) {
    let mut h = composing(cx, "keep");

    h.ime_mark("k");
    h.ime_commit("か");
    h.keystrokes("f3");

    assert_eq!(text_of(&mut h, "keep-value"), "abか");
}

#[gpui::test]
fn disabling_a_composing_input_ends_the_composition(cx: &mut TestAppContext) {
    let mut h = composing(cx, "field");
    h.ime_mark("k");
    h.keystrokes("f9");

    h.keystrokes("f8");

    assert_eq!(text_of(&mut h, "comp"), "compositionend:k");
}

#[gpui::test]
fn composition_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = composing(cx, "flag");

    h.ime_mark("k");
    h.ime_commit("か");

    assert_eq!(
        text_of(&mut h, "flags"),
        concat!(
            "compositionstart:true:true:true:true compositionupdate:true:true:true:true ",
            "compositionupdate:true:true:true:true compositionend:true:true:true:true"
        )
    );
}
