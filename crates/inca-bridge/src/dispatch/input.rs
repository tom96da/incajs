// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The editing of an `input` node: the requests of the platform's text
//! input, the editing keys and the mouse selection.

use gpui::{App, ClipboardItem, Keystroke, Window};
use inca_gpui::input::{pasted, sanitized, typed};
use inca_gpui::{
    Edit, InputRequest, InputState, InputType, Motion, MousePayload, NodeId, is_disabled_control,
};

use super::EventDispatcher;

impl EventDispatcher {
    /// Whether `node` is an `input`.
    pub(super) fn is_input(&self, node: NodeId) -> bool {
        let host = self.host.borrow();
        host.tree
            .get(node)
            .is_some_and(|node| node.tag_name() == "input")
    }

    /// Whether `node` is a disabled button or input.
    fn is_disabled(&self, node: NodeId) -> bool {
        let host = self.host.borrow();
        host.tree.get(node).is_some_and(is_disabled_control)
    }

    /// Whether `node` is an `input` that takes edits: neither `readonly` nor disabled.
    fn editable(&self, node: NodeId) -> bool {
        let host = self.host.borrow();
        host.tree.get(node).is_some_and(|node| {
            node.tag_name() == "input"
                && !node.attributes().contains_key("readonly")
                && !is_disabled_control(node)
        })
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

    /// Writes `edit` into the text state of `node`.
    fn run_edit(&self, node: NodeId, edit: &Edit, window: &mut Window) {
        self.with_input(node, |state| state.apply(edit));
        Self::input_changed(window);
    }

    /// Applies a call of the platform's text input at `node`. A `readonly` or disabled input
    /// takes a selection and nothing else.
    pub(super) fn apply_input_request(
        &self,
        node: NodeId,
        request: InputRequest,
        window: &mut Window,
    ) {
        match request {
            InputRequest::Select(range) => {
                self.with_input(node, |state| {
                    let range = state.range_from_utf16(&range);
                    state.select(range.start, range.end);
                });
                Self::input_changed(window);
            }
            _ if !self.editable(node) => {}
            InputRequest::Replace { range, text } => {
                let state = self.input_state(node);
                let composing = state.composition().is_some();
                // The Tab and Enter the platform passes as text leave a selection in place.
                let text = if composing {
                    sanitized(&text)
                } else {
                    typed(&text)
                };
                if text.is_empty() && !composing {
                    return;
                }
                if let Some(edit) = state.replace(range, &text) {
                    self.run_edit(node, &edit, window);
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
                if text.is_empty() && state.composition().is_none() {
                    return;
                }
                let edit = state.compose(range, &text, selected);
                self.run_edit(node, &edit, window);
            }
            InputRequest::Unmark => {
                self.with_input(node, InputState::unmark);
                Self::input_changed(window);
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
        let Some(node) = focused
            .filter(|&node| self.is_input(node) && self.input_state(node).composition().is_none())
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
                    self.run_edit(node, &edit, window);
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
                        self.run_edit(node, &edit, window);
                    }
                }
            }
            "v" if secondary => {
                if editable {
                    self.paste(node, window, cx);
                }
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
        if let Some(edit) = edit {
            self.run_edit(node, &edit, window);
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
