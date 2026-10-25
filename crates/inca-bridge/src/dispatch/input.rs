// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The editing of an `input` node: the requests of the platform's text
//! input, the editing keys and the mouse selection.

use gpui::{App, ClipboardItem, Keystroke, Window};
use inca_gpui::input::{pasted, sanitized, typed};
use inca_gpui::{
    Edit, EventPayload, InputPayload, InputRequest, InputState, InputType, Motion, MousePayload,
    NodeId, is_disabled_control,
};

use crate::focus::FocusTransition;

use super::EventDispatcher;

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

    /// Fires the composition event `event` at `node` with `data`.
    fn fire_composition(
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

    /// Fires `beforeinput`, writes `edit` into the text state of `node` unless a listener
    /// canceled it or changed the value or the node, and fires `input`.
    fn run_edit(&self, node: NodeId, edit: &Edit, window: &mut Window, cx: &mut App) {
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
    pub(super) fn apply_input_request(
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
    pub(super) fn input_press(&self, target: NodeId, mouse: &MousePayload, window: &mut Window) {
        if mouse.button != 0 || !self.is_input(target) || self.is_disabled(target) {
            return;
        }
        self.with_input(target, |state| {
            state.place(state.offset_at(mouse.client_x), mouse.modifiers.shift);
        });
        self.state.input_drag.set(Some(target));
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
