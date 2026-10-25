// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::test_support::engine_with_bindings;
use gpui::{ClipboardItem, MouseButton, VisualTestContext};
use inca_gpui::InputLayout;
use inca_test_macros::gpui_cases;

/// The text and selection of the input as `(value, selection, reversed)`.
fn edited(s: &InputScene) -> (String, std::ops::Range<usize>, bool) {
    let state = s.state();
    (
        state.value().to_owned(),
        state.selection(),
        state.reversed(),
    )
}

fn replace(text: &str) -> InputRequest {
    InputRequest::Replace {
        range: None,
        text: text.to_owned(),
    }
}

fn mark(text: &str) -> InputRequest {
    InputRequest::Mark {
        range: None,
        text: text.to_owned(),
        selected: None,
    }
}

fn clipboard(cx: &TestAppContext) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

/// Sets each of `names` on the input of `s`.
fn set_attributes(s: &InputScene, names: &[&str]) {
    for name in names {
        s.set_attribute(name);
    }
}

fn write_clipboard(cx: &TestAppContext, text: &str) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
}

#[gpui::test]
fn typing_inserts_at_the_caret(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.focus(cx, s.input);
    s.select(1, 1);
    s.key(cx, "x");
    assert_eq!(edited(&s), ("axb".to_owned(), 2..2, false));
}

#[gpui::test]
fn typing_replaces_the_selection(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abc");
    s.focus(cx, s.input);
    s.select(0, 2);
    s.key(cx, "x");
    assert_eq!(edited(&s), ("xc".to_owned(), 1..1, false));
}

#[gpui::test]
fn a_typed_control_character_inserts_nothing(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.select(1, 3);
    let before = s.state();
    s.request(cx, replace("\n"));
    assert_eq!(s.state(), before);
}

#[gpui::test]
fn a_typed_text_drops_its_control_characters(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.select(1, 1);
    s.request(cx, replace("x\ty"));
    assert_eq!(edited(&s), ("axyb".to_owned(), 3..3, false));
}

#[gpui::test]
fn a_composition_text_drops_line_breaks(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    s.request(cx, mark("a\nb"));
    assert_eq!(s.state().value(), "ab");
    assert_eq!(s.state().composition(), Some(0..2));
}

#[gpui_cases]
#[gpui::test]
#[case::readonly("readonly")]
#[case::disabled("disabled")]
fn a_write_to_a_readonly_or_disabled_input_changes_nothing(
    cx: &mut TestAppContext,
    attribute: &str,
) {
    let s = input_scene(cx, "abcd");
    s.set_attribute(attribute);
    let before = s.state();
    s.request(cx, replace("x"));
    assert_eq!(s.state(), before);
}

#[gpui_cases]
#[gpui::test]
#[case::backspace("backspace", "acd", 1..1, false)]
#[case::delete("delete", "abd", 2..2, false)]
#[case::left("left", "abcd", 1..1, false)]
#[case::right("right", "abcd", 3..3, false)]
#[case::shift_left("shift-left", "abcd", 1..2, true)]
#[case::shift_right("shift-right", "abcd", 2..3, false)]
#[case::home("home", "abcd", 0..0, false)]
#[case::end("end", "abcd", 4..4, false)]
#[case::shift_home("shift-home", "abcd", 0..2, true)]
#[case::shift_end("shift-end", "abcd", 2..4, false)]
#[case::up("up", "abcd", 0..0, false)]
#[case::down("down", "abcd", 4..4, false)]
#[case::secondary_a("secondary-a", "abcd", 0..4, false)]
#[case::plain_a("a", "abacd", 3..3, false)]
fn a_key_edits_or_moves_in_the_focused_input(
    cx: &mut TestAppContext,
    keys: &str,
    value: &str,
    selection: std::ops::Range<usize>,
    reversed: bool,
) {
    let s = input_scene(cx, "abcd");
    s.focus(cx, s.input);
    s.select(2, 2);
    s.key(cx, keys);
    assert_eq!(edited(&s), (value.to_owned(), selection, reversed));
}

#[gpui_cases]
#[gpui::test]
#[case::ctrl_left("ctrl-left")]
#[case::alt_backspace("alt-backspace")]
fn a_key_with_control_or_alt_leaves_the_input(cx: &mut TestAppContext, keys: &str) {
    let s = input_scene(cx, "abcd");
    s.focus(cx, s.input);
    s.select(2, 2);
    let before = s.state();
    s.key(cx, keys);
    assert_eq!(s.state(), before);
}

#[gpui::test]
fn an_edit_key_is_handled_for_the_platform(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.focus(cx, s.input);
    assert!(s.key(cx, "backspace"));
}

#[gpui::test]
fn an_unhandled_key_reaches_the_platform(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.focus(cx, s.input);
    assert!(!s.key(cx, "f5"));
}

#[gpui_cases]
#[gpui::test]
#[case::backspace("backspace", "abcd", 2..2)]
#[case::left("left", "abcd", 1..1)]
fn a_readonly_input_ignores_edit_keys(
    cx: &mut TestAppContext,
    keys: &str,
    value: &str,
    selection: std::ops::Range<usize>,
) {
    let s = input_scene(cx, "abcd");
    s.set_attribute("readonly");
    s.focus(cx, s.input);
    s.select(2, 2);
    s.key(cx, keys);
    assert_eq!(edited(&s), (value.to_owned(), selection, false));
}

#[gpui::test]
fn a_key_while_composing_is_left_to_the_platform(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.focus(cx, s.input);
    s.request(cx, mark("k"));
    assert!(!s.key(cx, "backspace"));
    assert_eq!(s.state().value(), "abk");
}

#[gpui::test]
fn a_canceled_keydown_leaves_the_input(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.engine
        .eval::<()>(&format!(
            "__inca_test_listen__({}, 'keydown', (e) => e.preventDefault());",
            s.input
        ))
        .unwrap();
    s.focus(cx, s.input);
    s.key(cx, "backspace");
    assert_eq!(s.state().value(), "abcd");
}

#[gpui_cases]
#[gpui::test]
#[case::selected(1, 3, "bc", &[])]
#[case::empty(2, 2, "keep", &[])]
#[case::readonly(1, 3, "bc", &["readonly"])]
fn copy_writes_the_selection_to_the_clipboard(
    cx: &mut TestAppContext,
    anchor: usize,
    focus: usize,
    want: &str,
    attributes: &[&str],
) {
    let s = input_scene(cx, "abcd");
    set_attributes(&s, attributes);
    write_clipboard(cx, "keep");
    s.focus(cx, s.input);
    s.select(anchor, focus);
    s.key(cx, "secondary-c");
    assert_eq!(clipboard(cx).as_deref(), Some(want));
}

#[gpui_cases]
#[gpui::test]
#[case::editable(&[], "ad", "bc")]
#[case::readonly(&["readonly"], "abcd", "keep")]
fn cut_moves_the_selection_to_the_clipboard(
    cx: &mut TestAppContext,
    attributes: &[&str],
    value: &str,
    want: &str,
) {
    let s = input_scene(cx, "abcd");
    set_attributes(&s, attributes);
    write_clipboard(cx, "keep");
    s.focus(cx, s.input);
    s.select(1, 3);
    s.key(cx, "secondary-x");
    assert_eq!(s.state().value(), value);
    assert_eq!(clipboard(cx).as_deref(), Some(want));
}

#[gpui_cases]
#[gpui::test]
#[case::text(&[], "abx ycd")]
#[case::readonly(&["readonly"], "abcd")]
fn paste_takes_the_clipboard_text_into_an_editable_input(
    cx: &mut TestAppContext,
    attributes: &[&str],
    want: &str,
) {
    let s = input_scene(cx, "abcd");
    set_attributes(&s, attributes);
    write_clipboard(cx, "x\ny");
    s.focus(cx, s.input);
    s.select(2, 2);
    s.key(cx, "secondary-v");
    assert_eq!(s.state().value(), want);
}

#[gpui::test]
fn a_press_places_the_caret_at_the_nearest_stop(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.press(cx, 13.0, false);
    assert_eq!(s.state().selection(), 2..2);
}

#[gpui::test]
fn a_shift_press_extends_the_selection(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.select(1, 1);
    s.press(cx, 19.0, true);
    assert_eq!(s.state().selection(), 1..3);
}

#[gpui::test]
fn a_drag_with_the_left_button_extends_the_selection(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.press(cx, 13.0, false);
    s.move_to(cx, 25.0);
    assert_eq!(s.state().selection(), 2..4);
}

#[gpui::test]
fn a_move_after_the_release_leaves_the_selection(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.press(cx, 13.0, false);
    s.release(cx, 13.0);
    s.move_to(cx, 25.0);
    assert_eq!(s.state().selection(), 2..2);
}

#[gpui::test]
fn a_right_release_keeps_the_drag(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.press(cx, 13.0, false);
    s.pointer(cx, "mousedown", s.input, mouse_of(13.0, 2, false));
    s.pointer(cx, "mouseup", s.input, mouse_of(13.0, 2, false));
    s.move_to(cx, 25.0);
    assert_eq!(s.state().selection(), 2..4);
}

#[gpui::test]
fn a_move_after_a_window_deactivation_leaves_the_selection(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.press(cx, 13.0, false);
    s.deactivate(cx);
    s.move_to(cx, 25.0);
    assert_eq!(s.state().selection(), 2..2);
}

#[gpui_cases]
#[gpui::test]
#[case::right_button(&[], 2)]
#[case::disabled(&["disabled"], 0)]
fn a_press_that_the_input_ignores_leaves_the_caret(
    cx: &mut TestAppContext,
    attributes: &[&str],
    button: u8,
) {
    let s = input_scene(cx, "abcd");
    set_attributes(&s, attributes);
    s.select(1, 1);
    s.pointer(cx, "mousedown", s.input, mouse_of(13.0, button, false));
    assert_eq!(s.state().selection(), 1..1);
}

#[gpui::test]
fn a_canceled_press_leaves_the_caret(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.engine
        .eval::<()>(&format!(
            "__inca_test_listen__({}, 'mousedown', (e) => e.preventDefault());",
            s.input
        ))
        .unwrap();
    s.select(1, 1);
    s.pointer(cx, "mousedown", s.input, mouse_of(13.0, 0, false));
    assert_eq!(s.state().selection(), 1..1);
}

#[gpui::test]
fn a_press_on_another_node_leaves_the_caret(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.select(1, 1);
    s.pointer(cx, "mousedown", s.button, mouse_of(13.0, 0, false));
    assert_eq!(s.state().selection(), 1..1);
}

#[gpui::test]
fn tab_into_an_input_selects_its_text(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.focus(cx, s.button);
    s.key(cx, "shift-tab");
    assert_eq!(s.state().selection(), 0..4);
}

#[gpui::test]
fn a_mark_request_marks_the_text(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    assert_eq!(s.state().value(), "abk");
    assert_eq!(s.state().composition(), Some(2..3));
}

#[gpui::test]
fn a_replace_while_composing_commits(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    s.request(cx, replace("か"));
    assert_eq!(s.state().value(), "abか");
    assert_eq!(s.state().composition(), None);
}

#[gpui::test]
fn an_unmark_keeps_the_composed_text(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    s.request(cx, InputRequest::Unmark);
    assert_eq!(s.state().value(), "abk");
    assert_eq!(s.state().composition(), None);
}

#[gpui::test]
fn an_empty_mark_ends_a_composition(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    s.request(cx, mark(""));
    assert_eq!(s.state().value(), "ab");
    assert_eq!(s.state().composition(), None);
}

#[gpui::test]
fn an_empty_mark_without_a_composition_changes_nothing(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.select(0, 1);
    let before = s.state();
    s.request(cx, mark(""));
    assert_eq!(s.state(), before);
}

#[gpui_cases]
#[gpui::test]
#[case::editable(&[])]
#[case::readonly(&["readonly"])]
fn a_select_request_sets_the_selection(cx: &mut TestAppContext, attributes: &[&str]) {
    let s = input_scene(cx, "ab");
    set_attributes(&s, attributes);
    s.request(cx, InputRequest::Select(0..1));
    assert_eq!(s.state().selection(), 0..1);
}

#[gpui::test]
fn the_sink_reports_input_state_for_an_input_node(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    assert!(s.dispatcher.input(s.input).is_some());
}

#[gpui::test]
fn the_sink_reports_no_input_state_for_a_div(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    assert!(s.dispatcher.input(s.parent).is_none());
}

#[gpui::test]
fn record_input_stores_the_layout(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    let layout = InputLayout {
        x: 1.0,
        y: 2.0,
        height: 3.0,
        scroll: 4.0,
        stops: vec![(0, 0.0)],
    };
    s.dispatcher.record_input(s.input, layout.clone());
    assert_eq!(s.dispatcher.input(s.input).unwrap().layout(), &layout);
}

#[test]
fn a_freed_input_drops_its_state() {
    let (engine, host) = engine_with_bindings();
    engine
        .eval::<()>(
            r"
            const n = __inca_native__;
            const node = n.createNode('input');
            n.appendChild(0, node);
            n.setInputValue(node, 'a');
            n.destroyNode(node);
            ",
        )
        .unwrap();
    assert!(host.borrow().inputs.borrow().is_empty());
}

#[gpui::test]
fn a_press_on_a_disabled_input_reaches_no_listener_of_it_or_its_parent(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    s.set_attribute("disabled");
    for node in [s.input, s.parent] {
        listen(&s.dispatcher, node, "mousedown", "() => out.push('down')");
    }
    s.draw(cx);
    let mut visual = VisualTestContext::from_window(s.window.into(), cx);
    visual.simulate_mouse_down(at(13.0, 5.0), MouseButton::Left, gpui::Modifiers::none());
    assert_eq!(read_out(&s.dispatcher), serde_json::json!([]));
}

#[test]
fn click_on_a_disabled_input_runs_nothing() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let input = host.borrow_mut().tree.create_node("input").unwrap();
    host.borrow_mut()
        .tree
        .set_attribute(input, "disabled", true)
        .unwrap();
    listen(&dispatcher, input, "click", "() => out.push('click')");
    let got = eval_json(
        &dispatcher,
        &format!("(__inca_native__.click({input}), out)"),
    );
    assert_eq!(got, serde_json::json!([]));
}
