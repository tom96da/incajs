// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The edits of an `input` and the `beforeinput` and `input` events.

use crate::dispatch::EventDispatcher;
use gpui::App;
use gpui::Window;
use inca_gpui::Edit;
use inca_gpui::EventPayload;
use inca_gpui::InputPayload;
use inca_gpui::InputRequest;
use inca_gpui::InputState;
use inca_gpui::InputType;
use inca_gpui::NodeId;
use inca_gpui::input::sanitized;
use inca_gpui::input::typed;
impl EventDispatcher {
    /// Fires `beforeinput`, writes `edit` into the text state of `node` unless a listener
    /// canceled it or changed the value or the node, and fires `input`.
    pub(super) fn run_edit(&self, node: NodeId, edit: &Edit, window: &mut Window, cx: &mut App) {
        let before = self.input_state(node).value().to_owned();
        let payload = EventPayload::Input(edit.payload(true));
        if self.fire(node, "beforeinput", &payload, window, cx) {
            return;
        }
        if edit.range.is_empty() && edit.text.is_empty() {
            return;
        }
        let connected = self.host.borrow().tree.is_connected(node);
        if !connected || self.input_state(node).value() != before {
            return;
        }
        self.with_input(node, |state| state.apply(edit));
        Self::input_changed(window);
        let payload = EventPayload::Input(edit.payload(false));
        self.fire(node, "input", &payload, window, cx);
    }

    /// Applies a call of the platform's text input at `node`. A disabled input, and a `readonly`
    /// one with a running composition, take a selection only. An idle `readonly` one also fires
    /// `beforeinput` for typed text.
    pub(crate) fn apply_input_request(
        &self,
        node: NodeId,
        request: InputRequest,
        window: &mut Window,
        cx: &mut App,
    ) {
        match request {
            InputRequest::Select(range) => {
                self.with_input(node, |state| {
                    let range = state.range_from_utf16(&range);
                    state.select(range.start, range.end);
                });
                Self::input_changed(window);
            }
            InputRequest::Replace { text, .. } if self.readonly_idle(node) => {
                let text = typed(&text);
                if !text.is_empty() {
                    let payload = EventPayload::Input(InputPayload {
                        data: Some(text),
                        input_type: InputType::InsertText,
                        is_composing: false,
                    });
                    self.fire(node, "beforeinput", &payload, window, cx);
                }
            }
            _ if !self.editable(node) => {}
            InputRequest::Replace { range, text } => {
                let composing = self.composing(node);
                // The Tab and Enter the platform passes as text leave a selection in place.
                let text = if composing {
                    sanitized(&text)
                } else {
                    typed(&text)
                };
                if composing {
                    self.fire_composition(node, "compositionupdate", &text, window, cx);
                } else if text.is_empty() {
                    return;
                }
                if let Some(edit) = self.input_state(node).replace(range, &text) {
                    self.run_edit(node, &edit, window, cx);
                }
                if composing {
                    self.fire_composition(node, "compositionend", &text, window, cx);
                }
            }
            InputRequest::Mark {
                range,
                text,
                selected,
            } => {
                // `shape_line` takes one line.
                let text = sanitized(&text);
                let state = self.input_state(node);
                let composing = state.composition().is_some();
                if text.is_empty() && !composing {
                    return;
                }
                if !composing {
                    let selected_text = state.selected_text().to_owned();
                    self.fire_composition(node, "compositionstart", &selected_text, window, cx);
                }
                self.fire_composition(node, "compositionupdate", &text, window, cx);
                let edit = self.input_state(node).compose(range, &text, selected);
                self.run_edit(node, &edit, window, cx);
                if text.is_empty() {
                    self.fire_composition(node, "compositionend", "", window, cx);
                }
            }
            InputRequest::Unmark => {
                let state = self.input_state(node);
                self.with_input(node, InputState::unmark);
                Self::input_changed(window);
                if let Some(range) = state.composition() {
                    let composed = &state.value()[range];
                    self.fire_composition(node, "compositionend", composed, window, cx);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::dispatch::input::scene::{InputScene, input_scene, log, record, types};
    use crate::dispatch::input::scene::{
        cancel_beforeinput, keydown, line_break_rows, mark, replace, row, set_attributes,
    };
    use crate::tests::listen;
    use gpui::{ClipboardItem, TestAppContext};
    use inca_test_macros::gpui_cases;

    fn write_clipboard(cx: &TestAppContext, text: &str) {
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
    }

    fn clipboard(cx: &TestAppContext) -> Option<String> {
        cx.read_from_clipboard().and_then(|item| item.text())
    }

    /// The text and selection of the input as `(value, selection, reversed)`.
    fn edited(s: &InputScene) -> (String, std::ops::Range<usize>, bool) {
        let state = s.state();
        (
            state.value().to_owned(),
            state.selection(),
            state.reversed(),
        )
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
    fn paste_of_an_empty_clipboard_changes_nothing(cx: &mut TestAppContext) {
        let s = input_scene(cx, "abcd");
        s.focus(cx, s.input);
        s.select(1, 3);
        s.key(cx, "secondary-v");
        assert_eq!(s.state().value(), "abcd");
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

    #[gpui::test]
    fn enter_fires_beforeinput_for_a_line_break(cx: &mut TestAppContext) {
        let s = input_scene(cx, "");
        s.focus(cx, s.input);
        s.key(cx, "x");
        record(&s, s.input, &["beforeinput", "change"]);
        s.key(cx, "enter");
        assert_eq!(log(&s), line_break_rows());
    }
}
