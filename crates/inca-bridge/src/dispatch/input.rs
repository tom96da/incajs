// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The editing of an `input` node: the requests of the platform's text
//! input, the editing keys and the mouse selection.

use gpui::{App, ClipboardItem, Keystroke, MouseButton, Window};
use inca_gpui::{
    EventPayload, InputPayload, InputState, InputType, Motion, MousePayload, opens_context_menu,
};
use inca_node::input_text::pasted;
use inca_node::{NodeId, is_disabled_control};

use crate::focus::FocusTransition;

use super::EventDispatcher;

mod compose;
mod edit;
#[cfg(test)]
pub(crate) mod scene;

impl EventDispatcher {
    /// Whether `node` is an `input`.
    pub(super) fn is_input(&self, node: NodeId) -> bool {
        let host = self.host.borrow();
        host.tree
            .get(node)
            .is_some_and(|node| node.tag_name() == "input")
    }

    /// Reads `node` once: whether it is an `input`, has `readonly`, and is a disabled control.
    fn input_flags(&self, node: NodeId) -> Option<(bool, bool, bool)> {
        let host = self.host.borrow();
        host.tree.get(node).map(|node| {
            let attributes = node.attributes();
            (
                node.tag_name() == "input",
                attributes.contains_key("readonly"),
                is_disabled_control(node),
            )
        })
    }

    /// Whether `node` is a `readonly` input, not disabled and with no composition.
    fn readonly_idle(&self, node: NodeId) -> bool {
        let readonly = matches!(self.input_flags(node), Some((true, true, false)));
        readonly && !self.composing(node)
    }

    /// Whether `node` is an `input` that takes edits: neither `readonly` nor disabled.
    fn editable(&self, node: NodeId) -> bool {
        matches!(self.input_flags(node), Some((true, false, false)))
    }

    /// Whether the text state of `node` has a composition.
    fn composing(&self, node: NodeId) -> bool {
        let host = self.host.borrow();
        let inputs = host.inputs.borrow();
        inputs
            .get(&node)
            .is_some_and(|state| state.composition().is_some())
    }

    /// A copy of the text state of `node`, the default state without one.
    pub(super) fn input_state(&self, node: NodeId) -> InputState {
        let host = self.host.borrow();
        host.inputs.borrow().get(&node).cloned().unwrap_or_default()
    }

    /// Runs `f` on the text state of `node`, created when missing. The borrows of the host
    /// end when it returns.
    fn with_input<R>(&self, node: NodeId, f: impl FnOnce(&mut InputState) -> R) -> R {
        let host = self.host.borrow();
        let mut inputs = host.inputs.borrow_mut();
        f(inputs.entry(node).or_default())
    }

    /// Redraws after an edit or a selection change, and tells the platform to ask again where the
    /// text is.
    fn input_changed(window: &mut Window) {
        window.invalidate_character_coordinates();
        window.refresh();
    }

    /// Fires `change` at `node`.
    fn fire_change(&self, node: NodeId, window: &mut Window, cx: &mut App) {
        self.fire(node, "change", &EventPayload::None, window, cx);
    }

    /// Whether the focused node is an `input` with a composition.
    pub(super) fn composing_at_focus(&self, window: &Window, cx: &App) -> bool {
        let focused = self.host.borrow().focus.focused_node(window, cx);
        focused.is_some_and(|node| self.composing(node))
    }

    /// Fires `change` for an `input` the window leaves while it keeps its focus.
    pub(super) fn input_window_blur(&self, node: NodeId, window: &mut Window, cx: &mut App) {
        if self.is_input(node) && self.with_input(node, InputState::commit) {
            self.fire_change(node, window, cx);
        }
    }

    /// Ends the composition and fires `change` of an `input` that loses focus, and records the
    /// text of one that takes it.
    pub(super) fn input_focus_moved(
        &self,
        transition: &FocusTransition,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(node) = transition.blurred().filter(|&node| self.is_input(node)) {
            let state = self.input_state(node);
            if let Some(range) = state.composition() {
                self.with_input(node, InputState::unmark);
                let composed = state.value()[range].to_owned();
                self.fire_composition(node, "compositionend", &composed, window, cx);
            }
            if self.with_input(node, InputState::end_focus) {
                self.fire_change(node, window, cx);
            }
        }
        if let Some(node) = transition.focused().filter(|&node| self.is_input(node)) {
            self.with_input(node, InputState::begin_focus);
        }
    }

    /// Runs the default action of a key press at the focused `input`. Returns whether the key
    /// was handled. A composition leaves every key to the platform's input method.
    pub(super) fn input_key(
        &self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let focused = self.host.borrow().focus.focused_node(window, cx);
        let Some(node) = focused.filter(|&node| self.is_input(node) && !self.composing(node))
        else {
            return false;
        };
        let modifiers = keystroke.modifiers;
        let plain = !(modifiers.control || modifiers.alt || modifiers.platform);
        let secondary = modifiers.secondary();
        let editable = self.editable(node);
        let key = keystroke.key.as_str();
        match key {
            "left" | "right" | "home" | "end" | "up" | "down" if plain => {
                let motion = match key {
                    "left" => Motion::Back,
                    "right" => Motion::Forward,
                    "home" | "up" => Motion::Start,
                    _ => Motion::End,
                };
                let macos = cfg!(target_os = "macos");
                self.with_input(node, |state| state.motion(motion, modifiers.shift, macos));
                Self::input_changed(window);
            }
            key @ ("backspace" | "delete") if plain => {
                if editable {
                    let edit = self.input_state(node).delete(key == "delete");
                    self.run_edit(node, &edit, window, cx);
                }
            }
            "a" if secondary => {
                self.with_input(node, InputState::select_all);
                Self::input_changed(window);
            }
            "c" if secondary => self.copy(node, cx),
            "x" if secondary => {
                if editable {
                    self.copy(node, cx);
                    if let Some(edit) = self.input_state(node).cut() {
                        self.run_edit(node, &edit, window, cx);
                    }
                }
            }
            "v" if secondary => {
                if editable {
                    self.paste(node, window, cx);
                }
            }
            "enter" if plain => {
                let payload = EventPayload::Input(InputPayload {
                    data: None,
                    input_type: InputType::InsertLineBreak,
                    is_composing: false,
                });
                self.fire(node, "beforeinput", &payload, window, cx);
                if self.is_input(node) && self.with_input(node, InputState::commit) {
                    self.fire_change(node, window, cx);
                }
                return false;
            }
            _ => return false,
        }
        true
    }

    /// Writes the selected text of `node` to the clipboard. An empty selection leaves it.
    fn copy(&self, node: NodeId, cx: &mut App) {
        let text = self.input_state(node).selected_text().to_owned();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Inserts the clipboard text, with its line breaks as spaces, at the caret of `node`.
    fn paste(&self, node: NodeId, window: &mut Window, cx: &mut App) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let edit = self
            .input_state(node)
            .insert(&pasted(&text), InputType::InsertFromPaste);
        if let Some(mut edit) = edit {
            edit.data = Some(text);
            self.run_edit(node, &edit, window, cx);
        }
    }

    /// Puts the caret of the `input` `target` under the mouse press `mouse`, extends the
    /// selection with Shift and starts a drag. Only the left button does.
    /// A press that opens the context menu on `macos` keeps a selection it lands in or on
    /// its edge and starts no drag.
    pub(super) fn input_press(
        &self,
        target: NodeId,
        mouse: &MousePayload,
        macos: bool,
        window: &mut Window,
    ) {
        let enabled_input = matches!(self.input_flags(target), Some((true, _, false)));
        if mouse.button != 0 || !enabled_input {
            return;
        }
        let menu = opens_context_menu(MouseButton::Left, mouse.modifiers, macos);
        self.with_input(target, |state| {
            let offset = state.offset_at(mouse.client_x);
            if menu {
                let selection = state.selection();
                if !(selection.start..=selection.end).contains(&offset) {
                    state.place(offset, false);
                }
            } else {
                state.place(offset, mouse.modifiers.shift);
            }
        });
        if !menu {
            self.state.input_drag.set(Some(target));
        }
        Self::input_changed(window);
    }

    /// Extends the selection of the `input` a held left press started in to the mouse at `mouse`.
    pub(super) fn input_drag_to(&self, mouse: &MousePayload, window: &mut Window) {
        let Some(node) = self.state.input_drag.get() else {
            return;
        };
        self.with_input(node, |state| {
            state.place(state.offset_at(mouse.client_x), true);
        });
        Self::input_changed(window);
    }

    /// Selects all the text of `node` when it is an `input`.
    pub(super) fn select_input(&self, node: NodeId) {
        if self.is_input(node) {
            self.with_input(node, InputState::select_all);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::dispatch::input::scene::{InputScene, input_scene, log, mouse_of, record, types};
    use crate::dispatch::input::scene::{
        cancel_beforeinput, eval_input, line_break_rows, replace, set_attributes,
    };
    use crate::test_support::engine_with_bindings;
    use crate::tests::{at, dispatcher_with_engine, eval_json, listen, listen_global, read_out};
    use gpui::{MouseButton, TestAppContext, VisualTestContext};
    use inca_gpui::{EventSink as _, InputLayout, InputRequest};
    use inca_test_macros::gpui_cases;

    impl InputScene {
        fn move_to(&self, cx: &mut TestAppContext, x: f32) {
            self.pointer(cx, "mousemove", self.input, mouse_of(x, 0, false));
        }
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

    /// Presses the left button with Control held at x 13 on the input, which holds `selection`.
    fn control_press(
        s: &InputScene,
        cx: &mut TestAppContext,
        macos: bool,
        selection: (usize, usize),
    ) {
        s.select(selection.0, selection.1);
        let mut mouse = mouse_of(13.0, 0, false);
        mouse.modifiers.control = true;
        s.in_window(cx, |scene, window, _| {
            scene
                .dispatcher
                .input_press(scene.input, &mouse, macos, window);
        });
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::mac_outside(true, (0, 1), (2, 2))]
    #[case::mac_inside(true, (0, 3), (0, 3))]
    #[case::mac_empty(true, (1, 1), (2, 2))]
    #[case::linux(false, (0, 3), (2, 2))]
    fn a_control_press_follows_the_platform(
        cx: &mut TestAppContext,
        macos: bool,
        selection: (usize, usize),
        want: (usize, usize),
    ) {
        let s = input_scene(cx, "abcd");
        control_press(&s, cx, macos, selection);
        assert_eq!(s.state().selection(), want.0..want.1);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::mac(true, (0, 1), false)]
    #[case::linux(false, (0, 3), true)]
    fn a_control_press_starts_a_drag_off_macos_only(
        cx: &mut TestAppContext,
        macos: bool,
        selection: (usize, usize),
        dragged: bool,
    ) {
        let s = input_scene(cx, "abcd");
        control_press(&s, cx, macos, selection);
        let pressed = s.state().selection();
        s.move_to(cx, 25.0);
        assert_eq!(s.state().selection() != pressed, dragged);
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
    fn a_press_on_a_disabled_input_reaches_no_listener_of_it_or_its_parent(
        cx: &mut TestAppContext,
    ) {
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
    fn enter_fires_change_after_a_canceled_line_break(cx: &mut TestAppContext) {
        let s = input_scene(cx, "");
        s.focus(cx, s.input);
        s.key(cx, "x");
        record(&s, s.input, &["beforeinput", "change"]);
        cancel_beforeinput(&s);
        s.key(cx, "enter");
        assert_eq!(log(&s), line_break_rows());
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

    /// Blurs the input through the script and draws the frame that applies it.
    fn script_blur(s: &InputScene, cx: &mut TestAppContext) {
        eval_input(s, "__inca_native__.blurNode(INPUT);");
        s.draw(cx);
        cx.run_until_parked();
    }
}
