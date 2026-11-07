// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composition events of an `input`.

use crate::dispatch::EventDispatcher;
use gpui::App;
use gpui::Window;
use inca_gpui::EventPayload;
use inca_node::NodeId;
impl EventDispatcher {
    /// Fires the composition event `event` at `node` with `data`.
    pub(super) fn fire_composition(
        &self,
        node: NodeId,
        event: &str,
        data: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let payload = EventPayload::Composition {
            data: data.to_owned(),
        };
        self.fire(node, event, &payload, window, cx);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::dispatch::input::scene::{InputScene, input_scene, log, record, types};
    use crate::dispatch::input::scene::{cancel_beforeinput, eval_input, mark, replace, row};
    use crate::tests::listen;
    use gpui::TestAppContext;
    use inca_gpui::{EventSink as _, InputRequest};
    use inca_test_macros::gpui_cases;
    use serde_json::json;

    /// The value and composition of the input.
    fn edited_text(s: &InputScene) -> (String, Option<std::ops::Range<usize>>) {
        let state = s.state();
        (state.value().to_owned(), state.composition())
    }

    #[gpui::test]
    fn a_composition_text_drops_line_breaks(cx: &mut TestAppContext) {
        let s = input_scene(cx, "");
        s.request(cx, mark("a\nb"));
        assert_eq!(s.state().value(), "ab");
        assert_eq!(s.state().composition(), Some(0..2));
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
    fn canceling_a_composition_start_or_update_keeps_the_mark(
        cx: &mut TestAppContext,
        event: &str,
    ) {
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
    fn a_composition_ends_before_change_on_blur(cx: &mut TestAppContext) {
        let s = input_scene(cx, "ab");
        record(&s, s.input, &["compositionend", "change", "blur"]);
        s.focus(cx, s.input);
        s.request(cx, mark("k"));
        log(&s);
        s.focus(cx, s.button);
        assert_eq!(types(&s), ["compositionend", "change", "blur"]);
    }
}
