// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The walk of a key event, the `keypress` event and the default actions of a key press.

use gpui::{App, Window};
use inca_document::geometry::Align;
use inca_document::{EventPayload, MousePayload};
use inca_gpui::modifiers_of;

use super::context_menu::shift_f10_opens_context_menu;
use crate::dispatch::EventDispatcher;
use inca_document::EventTargetId;

impl EventDispatcher {
    /// Walks the key event `event` at `target` and returns whether a listener canceled it.
    pub(crate) fn walk_key(
        &self,
        target: EventTargetId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
    ) -> bool {
        let (canceled, entered) = self.walk(target, event, payload, &[]);
        if entered {
            self.drain_observed(window);
        }
        canceled
    }

    /// Fires `keypress` after an uncanceled `keydown` of a key that types a character, outside a
    /// composition, and returns whether a listener canceled it. A canceled one also hands the key
    /// to the platform as handled, so no text is inserted.
    pub(crate) fn keypress_canceled(
        &self,
        keystroke: &gpui::Keystroke,
        target: EventTargetId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let composing = matches!(payload, EventPayload::Key(key) if key.is_composing);
        if composing || !types_character(keystroke, cfg!(target_os = "macos")) {
            return false;
        }
        let canceled = self.walk_key(target, "keypress", payload, window);
        if canceled {
            window.prevent_default();
            cx.stop_propagation();
        }
        canceled
    }

    // Tab moves focus and dispatches nothing here. The next frame reports it.
    pub(crate) fn key_default(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        canceled: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let key = keystroke.key.as_str();
        let modifiers = keystroke.modifiers;
        let window_id = window.window_handle().window_id();
        let pending = if up {
            (key == "space")
                .then(|| self.state.space_down.take())
                .flatten()
        } else {
            if key == "space" {
                self.state.space_down.set(None);
            }
            None
        };
        if canceled {
            return;
        }
        if !up && self.input_key(keystroke, window, cx) {
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        let focused_button = || self.focused_enabled_button(window, cx);
        let menu = inca_gpui::is_menu_key(key);
        let plain = !(modifiers.control || modifiers.alt || modifiers.platform);
        let click = match (key, up) {
            ("tab", false) if plain => {
                let moved = self.cells.tab_move(modifiers.shift, window, cx);
                if let Some(node) = moved {
                    self.cells.reveal(node, Align::Reveal, Align::CenterOutside);
                    self.select_input(node);
                }
                None
            }
            ("enter", false) if plain => {
                let button = focused_button();
                if button.is_some() {
                    self.state.space_down.set(None);
                }
                button.or_else(|| self.focused_link(window, cx))
            }
            ("space", false) => {
                self.state
                    .space_down
                    .set(focused_button().map(|button| (window_id, button)));
                None
            }
            // Menu fires alone and F10 with Shift.
            (_, false)
                if (menu && !modifiers.shift)
                    || (key == "f10"
                        && modifiers.shift
                        && shift_f10_opens_context_menu(cfg!(target_os = "macos"))) =>
            {
                self.fire_key_context_menu(modifiers, window, cx);
                None
            }
            ("space", true) => focused_button().filter(|b| {
                let held = self.state.mouse_held.get();
                pending == Some((window_id, *b)) || held == Some((window_id, *b))
            }),
            _ => None,
        };
        if let Some(button) = click {
            self.state.mouse_held.set(None);
            let payload =
                EventPayload::Mouse(MousePayload::keyboard_click(modifiers_of(modifiers)));
            self.fire(button, "click", &payload, window, cx);
        }
    }
}

/// Whether the key press types a character: no Control or Meta held, and the key is Enter without
/// Alt or `key_char` starts with a printable character. Alt may be held on macOS only.
fn types_character(keystroke: &gpui::Keystroke, macos: bool) -> bool {
    let modifiers = keystroke.modifiers;
    if modifiers.control || modifiers.platform {
        return false;
    }
    if keystroke.key == "enter" {
        return !modifiers.alt;
    }
    // Alt+letter fires no keypress outside macOS, where Option composes a character.
    (macos || !modifiers.alt)
        && keystroke
            .key_char
            .as_deref()
            .and_then(|text| text.chars().next())
            .is_some_and(|c| !c.is_control())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::keys::{Scene, input_scene, scene};
    use gpui::{Keystroke, TestAppContext};
    use inca_gpui::InputRequest;
    use inca_test_macros::gpui_cases;
    use rstest::rstest;

    /// The names of the `events` that reached the field of `s`.
    fn at_field(s: &Scene, events: &[&str]) -> Vec<String> {
        s.types(s.field, events)
    }

    #[gpui::test]
    fn a_typed_key_fires_keydown_keypress_beforeinput_input_in_order(cx: &mut TestAppContext) {
        let s = input_scene(cx, "");
        s.focus(cx, s.field);
        s.press(cx, "x");
        s.release(cx, "x");
        assert_eq!(
            at_field(
                &s,
                &["keydown", "keypress", "beforeinput", "input", "keyup"]
            ),
            ["keydown", "keypress", "beforeinput", "input", "keyup"]
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::tab("tab")]
    #[case::escape("escape")]
    #[case::backspace("backspace")]
    fn a_key_that_types_no_character_fires_no_keypress(cx: &mut TestAppContext, pressed: &str) {
        let s = input_scene(cx, "ab");
        s.focus(cx, s.field);
        s.press(cx, pressed);
        assert_eq!(
            at_field(&s, &["keydown", "keypress"]),
            ["keydown"],
            "{pressed}"
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::control("ctrl-enter")]
    #[case::alt("alt-enter")]
    #[case::meta("cmd-enter")]
    fn enter_held_with_control_alt_or_meta_fires_no_keypress(
        cx: &mut TestAppContext,
        pressed: &str,
    ) {
        let s = input_scene(cx, "ab");
        s.focus(cx, s.field);
        s.press(cx, pressed);
        assert_eq!(
            at_field(&s, &["keydown", "keypress"]),
            ["keydown"],
            "{pressed}"
        );
    }

    #[rstest]
    #[case::option_s_on_macos("alt-s->ß", true, true)]
    #[case::alt_a_off_macos("alt-a->a", false, false)]
    #[case::control_option_s_on_macos("ctrl-alt-s->ß", true, false)]
    #[case::command_s_on_macos("cmd-s->s", true, false)]
    #[case::alt_enter_on_macos("alt-enter", true, false)]
    #[case::enter("enter", false, true)]
    #[case::letter("x->x", false, true)]
    fn types_character_follows_the_modifiers_and_the_platform(
        #[case] pressed: &'static str,
        #[case] macos: bool,
        #[case] want: bool,
    ) {
        let keystroke = Keystroke::parse(pressed).unwrap();

        assert_eq!(super::types_character(&keystroke, macos), want, "{pressed}");
    }

    #[test]
    fn an_empty_key_char_types_no_character() {
        let keystroke = Keystroke {
            modifiers: gpui::Modifiers::none(),
            key: "x".to_owned(),
            key_char: Some(String::new()),
        };

        assert!(!super::types_character(&keystroke, true));
    }

    #[rstest]
    #[case::delete("\u{7f}")]
    #[case::c1_control("\u{85}")]
    fn a_control_character_as_key_char_types_no_character(#[case] key_char: &str) {
        let keystroke = Keystroke {
            modifiers: gpui::Modifiers::none(),
            key: "x".to_owned(),
            key_char: Some(key_char.to_owned()),
        };

        assert!(!super::types_character(&keystroke, true));
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui_cases]
    #[gpui::test]
    #[case::alt_a("alt-a->a")]
    fn an_alt_key_fires_no_keypress_off_macos(cx: &mut TestAppContext, pressed: &str) {
        let s = input_scene(cx, "");
        s.focus(cx, s.field);
        s.press(cx, pressed);
        assert_eq!(
            at_field(&s, &["keydown", "keypress"]),
            ["keydown"],
            "{pressed}"
        );
    }

    #[gpui::test]
    fn a_canceled_keydown_fires_no_keypress(cx: &mut TestAppContext) {
        let s = input_scene(cx, "");
        s.cancel_at(s.field, "keydown");
        s.focus(cx, s.field);
        s.press(cx, "x");
        assert_eq!(at_field(&s, &["keydown", "keypress"]), ["keydown"]);
    }

    #[gpui::test]
    fn a_canceled_keypress_types_nothing(cx: &mut TestAppContext) {
        let s = input_scene(cx, "");
        s.cancel_at(s.field, "keypress");
        s.focus(cx, s.field);
        s.press(cx, "x");
        assert_eq!(
            at_field(&s, &["keydown", "keypress", "beforeinput"]),
            ["keydown", "keypress"]
        );
        assert_eq!(s.state().value(), "");
    }

    #[gpui::test]
    fn enter_in_an_input_fires_keypress_then_beforeinput(cx: &mut TestAppContext) {
        let s = input_scene(cx, "ab");
        s.focus(cx, s.field);
        s.press(cx, "enter");
        assert_eq!(
            at_field(&s, &["keydown", "keypress", "beforeinput"]),
            ["keydown", "keypress", "beforeinput"]
        );
    }

    #[gpui::test]
    fn a_canceled_keypress_of_enter_in_an_input_skips_beforeinput(cx: &mut TestAppContext) {
        let s = input_scene(cx, "ab");
        s.cancel_at(s.field, "keypress");
        s.focus(cx, s.field);
        s.press(cx, "enter");
        assert_eq!(
            at_field(&s, &["keydown", "keypress", "beforeinput"]),
            ["keydown", "keypress"]
        );
    }

    #[gpui::test]
    fn a_canceled_keypress_of_space_on_a_button_leaves_the_click_on_key_up(
        cx: &mut TestAppContext,
    ) {
        let s = input_scene(cx, "");
        s.cancel_at(s.first, "keypress");
        s.focus(cx, s.first);
        s.press(cx, "space");
        s.release(cx, "space");
        assert_eq!(
            s.types(s.first, &["keypress", "click"]),
            ["keypress", "click"]
        );
    }

    #[gpui::test]
    fn no_keypress_fires_during_a_composition(cx: &mut TestAppContext) {
        let s = input_scene(cx, "");
        s.focus(cx, s.field);
        s.request(
            cx,
            InputRequest::Mark {
                range: None,
                text: "k".to_owned(),
                selected: None,
            },
        );
        s.press(cx, "x");
        assert_eq!(at_field(&s, &["keydown", "keypress"]), ["keydown"]);
    }

    /// The focus a `keydown` asks for applies at the next frame, so `keypress` goes to the node
    /// that had the focus.
    #[gpui::test]
    fn a_keydown_that_asks_for_another_focus_sends_keypress_to_the_old_target(
        cx: &mut TestAppContext,
    ) {
        let s = input_scene(cx, "");
        let (cells, button, field) = (s.cells.clone(), s.first, s.field);
        s.on_walk(move |walked| {
            if walked.name == "keydown" && walked.target_text() == field.to_string() {
                cells.document.borrow_mut().request_focus(button, false);
            }
            false
        });
        s.focus(cx, s.field);
        s.press(cx, "x");
        assert_eq!(at_field(&s, &["keypress"]), ["keypress"]);
        assert!(s.types(s.first, &["keypress"]).is_empty());
    }

    #[gpui::test]
    fn tab_moves_focus_to_the_next_stop(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "tab", false, false);
        assert_eq!(s.focused(cx), Some(s.second));
    }

    #[gpui::test]
    fn shift_tab_moves_focus_to_the_previous_stop(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.second);
        s.key(cx, "shift-tab", false, false);
        assert_eq!(s.focused(cx), Some(s.first));
    }

    #[gpui::test]
    fn tab_up_leaves_focus_where_it_is(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "tab", true, false);
        assert_eq!(s.focused(cx), Some(s.first));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::control("ctrl-tab")]
    #[case::alt("alt-tab")]
    #[case::platform("cmd-tab")]
    fn tab_with_a_command_modifier_leaves_focus_where_it_is(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, key, false, false);
        assert_eq!(s.focused(cx), Some(s.first));
    }

    #[gpui::test]
    fn a_canceled_tab_down_leaves_focus_where_it_is(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "tab", false, true);
        assert_eq!(s.focused(cx), Some(s.first));
    }

    #[gpui::test]
    fn a_tab_move_reports_its_focus_events_on_the_next_frame(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.forget();
        s.key(cx, "tab", false, false);
        s.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        let (first, second) = (s.first, s.second);
        let moves: Vec<_> = s
            .walks()
            .iter()
            .filter(|walked| matches!(walked.name.as_str(), "blur" | "focus"))
            .map(|walked| (walked.label(), walked.related()))
            .collect();
        assert_eq!(
            moves,
            [
                (format!("blur@{first}"), Some(second)),
                (format!("focus@{second}"), Some(first))
            ]
        );
    }

    impl Scene {
        /// Moves `plain` between `first` and `second`.
        fn plain_between_the_buttons(&self) {
            self.cells
                .document
                .borrow_mut()
                .insert_before(
                    &*self.cells.window.borrow(),
                    self.parent,
                    self.plain,
                    Some(self.second),
                )
                .unwrap();
        }
    }

    #[gpui::test]
    fn a_press_on_a_node_that_takes_no_focus_sets_where_tab_starts(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.plain_between_the_buttons();
        s.mouse_down(cx, s.plain);
        s.key(cx, "tab", false, false);
        assert_eq!(s.focused(cx), Some(s.second));
    }

    #[gpui::test]
    fn a_canceled_press_sets_where_tab_starts(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.plain_between_the_buttons();
        s.cancel_at(s.plain, "mousedown");
        s.mouse_down(cx, s.plain);
        s.key(cx, "tab", false, false);
        assert_eq!(s.focused(cx), Some(s.second));
    }

    #[gpui::test]
    fn a_press_on_the_root_clears_where_tab_starts(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.plain_between_the_buttons();
        s.mouse_down(cx, s.plain);
        let root = s.cells.document.borrow().root();
        s.mouse_down(cx, root);
        s.key(cx, "shift-tab", false, false);
        assert_eq!(s.focused(cx), Some(s.field));
    }

    #[gpui::test]
    fn shift_tab_after_a_pressed_button_loses_focus_focuses_it_again(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.mouse_down(cx, s.first);
        s.blur(cx, s.first);
        s.key(cx, "shift-tab", false, false);
        assert_eq!(s.focused(cx), Some(s.first));
    }

    #[gpui::test]
    fn a_press_on_the_focused_node_leaves_a_later_focus_as_the_start(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.second);
        s.mouse_down(cx, s.second);
        s.blur(cx, s.second);
        s.focus(cx, s.second);
        s.blur(cx, s.second);
        s.key(cx, "shift-tab", false, false);
        assert_eq!(s.focused(cx), Some(s.first));
    }

    #[gpui::test]
    fn space_down_with_modifiers_still_starts_a_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "ctrl-space", false, false);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), [s.first]);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::shift_enter("shift-enter", false, "true,false,false,false")]
    #[case::control_space_up("ctrl-space", true, "false,true,false,false")]
    #[case::alt_space_up("alt-space", true, "false,false,true,false")]
    #[case::meta_space_up("cmd-space", true, "false,false,false,true")]
    fn a_key_click_carries_the_modifiers_of_its_key(
        cx: &mut TestAppContext,
        key: &str,
        up: bool,
        want: &str,
    ) {
        let s = scene(cx);
        s.focus(cx, s.first);
        if up {
            s.key(cx, "space", false, false);
        }
        s.key(cx, key, up, false);
        let click = s
            .walks()
            .into_iter()
            .find(|walked| walked.name == "click")
            .unwrap()
            .mouse();
        let m = click.modifiers;
        let got = format!("{},{},{},{}", m.shift(), m.control(), m.alt(), m.platform());
        assert_eq!(got, want);
    }
}
