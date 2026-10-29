// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use gpui::ClipboardItem;
use inca_test_macros::gpui_cases;
use serde_json::{Value, json};

fn row(
    kind: &str,
    input_type: Option<&str>,
    data: Option<&str>,
    composing: Option<bool>,
    cancelable: bool,
    value: &str,
) -> Value {
    json!([kind, input_type, data, composing, cancelable, value])
}

/// The `keydown` row of a key typed at an input holding `value`.
fn keydown(composing: bool, value: &str) -> Value {
    row("keydown", None, None, Some(composing), true, value)
}

/// The value and composition of the input.
fn edited_text(s: &InputScene) -> (String, Option<std::ops::Range<usize>>) {
    let state = s.state();
    (state.value().to_owned(), state.composition())
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

/// Runs `js` with `INPUT` replaced by the id of the input.
fn eval_input(s: &InputScene, js: &str) {
    s.engine
        .eval::<()>(&js.replace("INPUT", &s.input.to_string()))
        .unwrap();
}

fn cancel_beforeinput(s: &InputScene) {
    listen(
        &s.dispatcher,
        s.input,
        "beforeinput",
        "(e) => e.preventDefault()",
    );
}

/// Blurs the input through the script and draws the frame that applies it.
fn script_blur(s: &InputScene, cx: &mut TestAppContext) {
    eval_input(s, "__inca_native__.blurNode(INPUT);");
    s.draw(cx);
    cx.run_until_parked();
}

#[gpui::test]
fn typing_fires_beforeinput_then_input(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["keydown", "beforeinput", "input"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(
        log(&s),
        [
            keydown(false, ""),
            row(
                "beforeinput",
                Some("insertText"),
                Some("x"),
                Some(false),
                true,
                ""
            ),
            row(
                "input",
                Some("insertText"),
                Some("x"),
                Some(false),
                false,
                "x"
            ),
        ]
    );
}

#[gpui::test]
fn a_canceled_beforeinput_drops_the_edit(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["beforeinput", "input"]);
    cancel_beforeinput(&s);
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(types(&s), ["beforeinput"]);
    assert_eq!(s.state().value(), "");
}

#[gpui_cases]
#[gpui::test]
#[case::backspace("backspace", (2, 2), "deleteContentBackward", None, None, "acd")]
#[case::delete("delete", (2, 2), "deleteContentForward", None, None, "abd")]
#[case::cut("secondary-x", (1, 3), "deleteByCut", None, None, "ad")]
#[case::paste("secondary-v", (2, 2), "insertFromPaste", Some("x\ny"), Some("x y"), "abx ycd")]
fn a_key_edit_names_its_input_type(
    cx: &mut TestAppContext,
    keys: &str,
    (anchor, focus): (usize, usize),
    input_type: &str,
    before: Option<&str>,
    after: Option<&str>,
    value: &str,
) {
    let s = input_scene(cx, "abcd");
    cx.write_to_clipboard(ClipboardItem::new_string("x\ny".to_owned()));
    record(&s, s.input, &["beforeinput", "input"]);
    s.focus(cx, s.input);
    s.select(anchor, focus);
    s.key(cx, keys);
    assert_eq!(
        log(&s),
        [
            row(
                "beforeinput",
                Some(input_type),
                before,
                Some(false),
                true,
                "abcd"
            ),
            row("input", Some(input_type), after, Some(false), false, value),
        ]
    );
}

#[gpui_cases]
#[gpui::test]
#[case::backspace_at_start("backspace", 0, "deleteContentBackward")]
#[case::delete_at_end("delete", 4, "deleteContentForward")]
fn a_delete_with_nothing_to_delete_fires_only_beforeinput(
    cx: &mut TestAppContext,
    keys: &str,
    caret: usize,
    input_type: &str,
) {
    let s = input_scene(cx, "abcd");
    record(&s, s.input, &["keydown", "beforeinput", "input"]);
    s.focus(cx, s.input);
    s.select(caret, caret);
    s.key(cx, keys);
    assert_eq!(
        log(&s),
        [
            keydown(false, "abcd"),
            row(
                "beforeinput",
                Some(input_type),
                None,
                Some(false),
                true,
                "abcd"
            ),
        ]
    );
    assert_eq!(s.state().value(), "abcd");
}

#[gpui::test]
fn a_beforeinput_listener_that_changes_the_value_drops_the_edit(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["input"]);
    listen(
        &s.dispatcher,
        s.input,
        "beforeinput",
        &format!("() => __inca_native__.setInputValue({}, 'z')", s.input),
    );
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(s.state().value(), "z");
    assert_eq!(types(&s), Vec::<String>::new());
}

#[gpui::test]
fn a_beforeinput_listener_that_removes_the_input_drops_the_edit(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["input"]);
    listen(
        &s.dispatcher,
        s.input,
        "beforeinput",
        &format!(
            "() => __inca_native__.removeChild({}, {})",
            s.parent, s.input
        ),
    );
    s.request(cx, replace("x"));
    assert_eq!(types(&s), Vec::<String>::new());
}

#[gpui::test]
fn enter_after_a_beforeinput_that_destroys_the_input_leaves_no_state(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["change"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    listen(
        &s.dispatcher,
        s.input,
        "beforeinput",
        &format!("() => __inca_native__.destroyNode({})", s.input),
    );
    s.key(cx, "enter");
    assert_eq!(types(&s), Vec::<String>::new());
    assert!(s.host.borrow().inputs.borrow().get(&s.input).is_none());
}

#[gpui::test]
fn a_composition_fires_start_update_and_input_pairs(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(
        &s,
        s.input,
        &[
            "compositionstart",
            "compositionupdate",
            "beforeinput",
            "input",
        ],
    );
    s.request(cx, mark("k"));
    s.request(cx, mark("か"));
    let composition = |kind, data, value| row(kind, None, Some(data), None, true, value);
    let edit = |kind, data, cancelable, value| {
        row(
            kind,
            Some("insertCompositionText"),
            Some(data),
            Some(true),
            cancelable,
            value,
        )
    };
    assert_eq!(
        log(&s),
        [
            composition("compositionstart", "", "ab"),
            composition("compositionupdate", "k", "ab"),
            edit("beforeinput", "k", false, "ab"),
            edit("input", "k", false, "abk"),
            composition("compositionupdate", "か", "abk"),
            edit("beforeinput", "か", false, "abk"),
            edit("input", "か", false, "abか"),
        ]
    );
}

#[gpui::test]
fn a_commit_ends_the_composition(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    record(
        &s,
        s.input,
        &[
            "compositionupdate",
            "beforeinput",
            "input",
            "compositionend",
        ],
    );
    s.request(cx, replace("か"));
    let edit = |kind, cancelable, value| {
        row(
            kind,
            Some("insertCompositionText"),
            Some("か"),
            Some(true),
            cancelable,
            value,
        )
    };
    assert_eq!(
        log(&s),
        [
            row("compositionupdate", None, Some("か"), None, true, "abk"),
            edit("beforeinput", false, "abk"),
            edit("input", false, "abか"),
            row("compositionend", None, Some("か"), None, true, "abか"),
        ]
    );
}

#[gpui::test]
fn compositionstart_carries_the_selected_text(cx: &mut TestAppContext) {
    let s = input_scene(cx, "abcd");
    record(&s, s.input, &["compositionstart"]);
    s.select(1, 3);
    s.request(cx, mark("x"));
    assert_eq!(
        log(&s),
        [row(
            "compositionstart",
            None,
            Some("bc"),
            None,
            true,
            "abcd"
        )]
    );
}

#[gpui::test]
fn an_empty_mark_ends_the_composition_with_no_text(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    record(
        &s,
        s.input,
        &[
            "compositionupdate",
            "beforeinput",
            "input",
            "compositionend",
        ],
    );
    s.request(cx, mark(""));
    assert_eq!(
        log(&s),
        [
            row("compositionupdate", None, Some(""), None, true, "abk"),
            row(
                "beforeinput",
                Some("insertCompositionText"),
                Some(""),
                Some(true),
                false,
                "abk"
            ),
            row(
                "input",
                Some("insertCompositionText"),
                None,
                Some(true),
                false,
                "ab"
            ),
            row("compositionend", None, Some(""), None, true, "ab"),
        ]
    );
}

#[gpui::test]
fn an_unmark_fires_only_compositionend(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    record(
        &s,
        s.input,
        &[
            "compositionupdate",
            "beforeinput",
            "input",
            "compositionend",
        ],
    );
    s.request(cx, InputRequest::Unmark);
    assert_eq!(
        log(&s),
        [row("compositionend", None, Some("k"), None, true, "abk")]
    );
}

#[gpui::test]
fn a_composition_beforeinput_cannot_be_canceled(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    cancel_beforeinput(&s);
    s.request(cx, mark("k"));
    assert_eq!(s.state().value(), "abk");
}

#[gpui_cases]
#[gpui::test]
#[case::compositionstart("compositionstart")]
#[case::compositionupdate("compositionupdate")]
fn canceling_a_composition_start_or_update_keeps_the_mark(cx: &mut TestAppContext, event: &str) {
    let s = input_scene(cx, "ab");
    listen(&s.dispatcher, s.input, event, "(e) => e.preventDefault()");
    s.request(cx, mark("k"));
    assert_eq!(edited_text(&s), ("abk".to_owned(), Some(2..3)));
}

#[gpui::test]
fn canceling_compositionend_keeps_the_commit(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    listen(
        &s.dispatcher,
        s.input,
        "compositionend",
        "(e) => e.preventDefault()",
    );
    s.request(cx, mark("k"));
    s.request(cx, replace("か"));
    assert_eq!(edited_text(&s), ("abか".to_owned(), None));
}

#[gpui_cases]
#[gpui::test]
#[case::unmark(InputRequest::Unmark)]
#[case::replace(replace("か"))]
#[case::mark(mark("c"))]
#[case::empty_mark(mark(""))]
fn a_request_to_a_readonly_input_with_a_composition_changes_nothing(
    cx: &mut TestAppContext,
    request: InputRequest,
) {
    let s = input_scene(cx, "ab");
    s.request(cx, mark("k"));
    record(
        &s,
        s.input,
        &[
            "compositionstart",
            "compositionupdate",
            "compositionend",
            "beforeinput",
            "input",
        ],
    );
    s.set_attribute("readonly");
    s.request(cx, request);
    assert_eq!(types(&s), Vec::<String>::new());
    assert_eq!(edited_text(&s), ("abk".to_owned(), Some(2..3)));
}

#[gpui::test]
fn a_script_that_disables_a_composing_input_ends_the_composition(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["compositionend", "blur"]);
    s.focus(cx, s.input);
    s.request(cx, mark("k"));
    eval_input(&s, "__inca_native__.setAttribute(INPUT, 'disabled', true);");
    s.draw(cx);
    cx.run_until_parked();
    assert_eq!(types(&s), ["compositionend", "blur"]);
    assert_eq!(edited_text(&s), ("abk".to_owned(), None));
}

/// Presses Shift, which fires a `keydown` at the focused input.
fn press_shift(s: &InputScene, cx: &mut TestAppContext) {
    s.in_window(cx, |scene, window, cx| {
        let shift = gpui::Modifiers {
            shift: true,
            ..gpui::Modifiers::none()
        };
        scene.dispatcher.modifiers_changed(shift, window, cx);
    });
}

#[gpui::test]
fn a_key_event_reports_an_active_composition(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["keydown"]);
    s.focus(cx, s.input);
    s.request(cx, mark("k"));
    s.key(cx, "f5");
    assert_eq!(log(&s)[0][3], json!(true));
}

#[gpui::test]
fn a_key_event_reports_no_composition_when_idle(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["keydown"]);
    s.focus(cx, s.input);
    s.key(cx, "f5");
    assert_eq!(log(&s)[0][3], json!(false));
}

#[gpui::test]
fn a_modifier_key_event_reports_an_active_composition(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["keydown"]);
    s.focus(cx, s.input);
    s.request(cx, mark("k"));
    press_shift(&s, cx);
    assert_eq!(log(&s)[0][3], json!(true));
}

#[gpui::test]
fn change_fires_before_blur_when_focus_moves(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["change", "blur", "focusout"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    s.focus(cx, s.button);
    assert_eq!(types(&s), ["change", "blur", "focusout"]);
}

#[gpui::test]
fn change_fires_before_blur_on_a_script_blur(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["change", "blur", "focusout"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    script_blur(&s, cx);
    assert_eq!(types(&s), ["change", "blur", "focusout"]);
}

#[gpui::test]
fn no_change_fires_for_a_value_back_at_its_focus_value(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["change"]);
    s.focus(cx, s.input);
    s.key(cx, "x backspace");
    s.focus(cx, s.button);
    assert_eq!(types(&s), Vec::<String>::new());
}

#[gpui::test]
fn enter_fires_change_once(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["change"]);
    s.focus(cx, s.input);
    s.key(cx, "x enter");
    s.focus(cx, s.button);
    assert_eq!(types(&s), ["change"]);
}

#[gpui::test]
fn enter_without_an_edit_fires_no_change(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["change"]);
    s.focus(cx, s.input);
    s.key(cx, "enter");
    assert_eq!(types(&s), Vec::<String>::new());
}

#[gpui::test]
fn a_script_write_after_an_edit_fires_one_change(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["change"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    eval_input(&s, "__inca_native__.setInputValue(INPUT, 'z');");
    s.focus(cx, s.button);
    assert_eq!(types(&s), ["change"]);
}

#[gpui::test]
fn a_script_write_alone_fires_no_change(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    record(&s, s.input, &["change"]);
    s.focus(cx, s.input);
    eval_input(&s, "__inca_native__.setInputValue(INPUT, 'z');");
    s.focus(cx, s.button);
    assert_eq!(types(&s), Vec::<String>::new());
}

#[gpui::test]
fn typing_into_a_readonly_input_fires_only_beforeinput(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    s.set_attribute("readonly");
    record(&s, s.input, &["keydown", "beforeinput", "input"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(
        log(&s),
        [
            keydown(false, ""),
            row(
                "beforeinput",
                Some("insertText"),
                Some("x"),
                Some(false),
                true,
                ""
            ),
        ]
    );
    assert_eq!(s.state().value(), "");
}

#[gpui::test]
fn a_typed_control_character_into_a_readonly_input_fires_nothing(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    s.set_attribute("readonly");
    record(&s, s.input, &["beforeinput"]);
    s.request(cx, replace("\n"));
    assert_eq!(types(&s), Vec::<String>::new());
}

#[gpui::test]
fn enter_in_a_readonly_input_fires_beforeinput_for_a_line_break(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    s.set_attribute("readonly");
    record(&s, s.input, &["beforeinput", "change"]);
    s.focus(cx, s.input);
    s.key(cx, "enter");
    assert_eq!(
        log(&s),
        [row(
            "beforeinput",
            Some("insertLineBreak"),
            None,
            Some(false),
            true,
            "ab"
        )]
    );
}

/// The rows of an `Enter` at an input holding "x": `beforeinput` for a line break, then `change`.
fn line_break_rows() -> Vec<Value> {
    vec![
        row(
            "beforeinput",
            Some("insertLineBreak"),
            None,
            Some(false),
            true,
            "x",
        ),
        row("change", None, None, None, false, "x"),
    ]
}

#[gpui::test]
fn enter_fires_beforeinput_for_a_line_break(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    s.focus(cx, s.input);
    s.key(cx, "x");
    record(&s, s.input, &["beforeinput", "change"]);
    s.key(cx, "enter");
    assert_eq!(log(&s), line_break_rows());
}

#[gpui::test]
fn enter_fires_change_after_a_canceled_line_break(cx: &mut TestAppContext) {
    let s = input_scene(cx, "");
    s.focus(cx, s.input);
    s.key(cx, "x");
    record(&s, s.input, &["beforeinput", "change"]);
    cancel_beforeinput(&s);
    s.key(cx, "enter");
    assert_eq!(log(&s), line_break_rows());
}

#[gpui::test]
fn a_composition_ends_before_change_on_blur(cx: &mut TestAppContext) {
    let s = input_scene(cx, "ab");
    record(&s, s.input, &["compositionend", "change", "blur"]);
    s.focus(cx, s.input);
    s.request(cx, mark("k"));
    log(&s);
    s.focus(cx, s.button);
    assert_eq!(types(&s), ["compositionend", "change", "blur"]);
}

/// Types in the input and deactivates the window, recording the events of the input and the
/// window's `blur`.
fn typed_then_deactivated(cx: &mut TestAppContext) -> InputScene {
    let s = input_scene(cx, "");
    record(&s, s.input, &["change", "blur", "focusout"]);
    listen_global(
        &s.dispatcher,
        "$window",
        "blur",
        false,
        "() => globalThis.log.push(['window blur'])",
    );
    s.focus(cx, s.input);
    s.key(cx, "x");
    s.deactivate(cx);
    s
}

fn activate(s: &InputScene, cx: &mut TestAppContext) {
    s.in_window(cx, |scene, window, cx| {
        scene.dispatcher.window_activated(true, window, cx);
    });
    log(s);
}

#[gpui::test]
fn a_window_deactivation_fires_change_before_blur(cx: &mut TestAppContext) {
    let s = typed_then_deactivated(cx);
    assert_eq!(types(&s), ["change", "blur", "focusout", "window blur"]);
}

#[gpui::test]
fn a_second_window_deactivation_with_the_same_value_fires_no_change(cx: &mut TestAppContext) {
    let s = typed_then_deactivated(cx);
    activate(&s, cx);
    s.deactivate(cx);
    assert_eq!(types(&s), ["blur", "focusout", "window blur"]);
}

#[gpui::test]
fn an_edit_after_the_return_fires_change_at_the_next_deactivation(cx: &mut TestAppContext) {
    let s = typed_then_deactivated(cx);
    activate(&s, cx);
    s.key(cx, "y");
    s.deactivate(cx);
    assert_eq!(types(&s), ["change", "blur", "focusout", "window blur"]);
}
