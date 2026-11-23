// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::rc::Rc;

use gpui::{App, MouseUpEvent, ScrollHandle, Window};
use inca_document::InputState;
use inca_document::{EventPayload, KeyPayload};
use inca_gpui::{EventSink, InputLayout, InputRequest, modifiers_of};
use inca_node::NodeId;

use crate::dispatch::EventDispatcher;
use inca_document::EventTargetId;

impl EventSink for EventDispatcher {
    fn tracks_pointer(&self) -> bool {
        self.tracks_pointer()
    }

    fn clear_layout(&self) {
        self.cells.window.borrow().layout.borrow_mut().clear();
    }

    fn record_layout(&self, node_id: NodeId, layout: inca_gpui::NodeLayout) {
        self.cells
            .window
            .borrow()
            .layout
            .borrow_mut()
            .insert(node_id, layout);
    }

    fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let pressed = (window.window_handle().window_id(), start);
        if event == "blur" {
            for slot in [&self.state.space_down, &self.state.mouse_held] {
                if slot.get() == Some(pressed) {
                    slot.set(None);
                }
            }
        }
        // The `contextmenu` of a press goes to the target of its `pointerdown`.
        let start = self
            .state
            .press_target
            .get()
            .filter(|_| event == "contextmenu")
            .unwrap_or(start);
        self.fire(start, event, payload, window, cx)
    }

    fn focus_handle(&self, node_id: NodeId) -> Option<gpui::FocusHandle> {
        let document = self.cells.document.borrow();
        let window = self.cells.window.borrow();
        window.focus.handle(node_id).or_else(|| {
            (Some(node_id) == document.tree().document_element())
                .then(|| window.focus.parked_handle())
                .flatten()
        })
    }

    fn scroll_handle(&self, node_id: NodeId) -> Option<ScrollHandle> {
        Some(self.cells.window.borrow().scroll_handle(node_id))
    }

    fn wheel_axes(&self, node_id: NodeId) -> Option<inca_style::WheelAxes> {
        let document = self.cells.document.borrow();
        inca_style::wheel_axes(document.tree(), document.style(), node_id)
    }

    /// The first call of an update computes the delta and defers clearing it,
    /// so later calls in that update keep it.
    fn pointer_moved(&self, position: gpui::Point<gpui::Pixels>, cx: &mut App) {
        if self.state.current_movement.get().is_some() {
            return;
        }
        let here = (f32::from(position.x), f32::from(position.y));
        let movement = self
            .state
            .last_position
            .replace(Some(here))
            .map_or((0.0, 0.0), |(x, y)| (here.0 - x, here.1 - y));
        self.state.current_movement.set(Some(movement));
        let current = Rc::clone(&self.state);
        cx.defer(move |_| current.current_movement.set(None));
    }

    /// Changed flags fire in the order Shift, Control, Alt, Meta.
    fn modifiers_changed(&self, modifiers: gpui::Modifiers, window: &mut Window, cx: &mut App) {
        let before = self
            .state
            .held_modifiers
            .replace(Some(modifiers))
            .unwrap_or(modifiers);
        let start = self.key_target(window, cx);
        let is_composing = self.composing_at_focus(window, cx);
        for (key, was, is) in [
            ("Shift", before.shift, modifiers.shift),
            ("Control", before.control, modifiers.control),
            ("Alt", before.alt, modifiers.alt),
            ("Meta", before.platform, modifiers.platform),
        ] {
            if was != is {
                let payload = EventPayload::Key(KeyPayload {
                    key: key.to_owned(),
                    repeat: false,
                    modifiers: modifiers_of(modifiers),
                    is_composing,
                });
                let event = if is { "keydown" } else { "keyup" };
                self.fire(start, event, &payload, window, cx);
            }
        }
    }

    fn pointer_left(&self) {
        self.state.last_position.set(None);
    }

    fn pointer_over(&self, node_id: NodeId, window: &mut Window, cx: &mut App) {
        self.state.hover_settled.set(true);
        self.report_hover(Some(node_id), window, cx);
        self.schedule_hover_report(window, cx);
    }

    fn pointer_released(
        &self,
        event: &MouseUpEvent,
        pressed: Option<NodeId>,
        released: NodeId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let captured = self.state.captured_release.take();
        let target = captured.map(|(target, _)| target);
        self.fire_clicks(event, pressed, released, target, window, cx);
        if let Some((_, button)) = captured {
            self.report_hover_with(self.deepest_hovered(), button, window, cx);
        }
    }

    fn pointer_input(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        match (event, payload) {
            ("mousemove", _) => self.pointer_move(target, payload, window, cx),
            ("mousedown", EventPayload::Mouse(mouse)) => {
                self.pointer_down(target, mouse.button, payload, window, cx);
            }
            ("mouseup", EventPayload::Mouse(mouse)) => {
                self.pointer_up(target, mouse.button, payload, window, cx);
            }
            // The wheel step reads `Window::default_prevented`.
            _ => {
                if self.fire_inner(target, event, payload, window, cx) && event == "wheel" {
                    window.prevent_default();
                }
            }
        }
    }

    fn hover_changed(&self, node_id: NodeId, hovered: bool, window: &mut Window, cx: &mut App) {
        {
            let mut set = self.state.hovered.borrow_mut();
            if hovered {
                set.insert(node_id);
            } else {
                set.remove(&node_id);
            }
        }
        self.schedule_hover_report(window, cx);
    }

    fn bar_pressed(
        &self,
        node_id: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let EventPayload::Mouse(mouse) = payload {
            self.bar_press(node_id, mouse.button, payload, window, cx);
        }
    }

    fn bar_released(
        &self,
        node_id: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let EventPayload::Mouse(mouse) = payload {
            self.bar_release(node_id, mouse.button, payload, window, cx);
        }
    }

    fn bar_drag(&self, node_id: NodeId, active: bool, window: &mut Window, cx: &mut App) {
        if active {
            self.state.bar_drag.set(Some(node_id));
        } else {
            self.end_bar_drag(Some(node_id), window, cx);
        }
    }

    fn key_input(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.turn(window, cx, |window, cx| {
            let target = self.key_target(window, cx);
            let event = if up { "keyup" } else { "keydown" };
            let target = EventTargetId::Node(target);
            let mut payload = payload.clone();
            if let EventPayload::Key(key) = &mut payload {
                key.is_composing = self.composing_at_focus(window, cx);
            }
            let canceled = self.walk_key(target, event, &payload, window);
            if canceled {
                // Handled for the platform: a shortcut or an input method sees nothing.
                window.prevent_default();
                cx.stop_propagation();
            }
            // A canceled keypress keeps the default of Space, which runs on key up.
            let typed_canceled = !up
                && !canceled
                && (keystroke.key != "enter" || self.focused_link(window, cx).is_none())
                && self.keypress_canceled(keystroke, target, &payload, window, cx)
                && keystroke.key != "space";
            self.key_default(keystroke, up, canceled || typed_canceled, window, cx);
        });
    }

    fn input(&self, node_id: NodeId) -> Option<InputState> {
        self.is_input(node_id).then(|| self.input_state(node_id))
    }

    fn input_layout(&self, node_id: NodeId) -> InputLayout {
        EventDispatcher::input_layout(self, node_id)
    }

    fn record_input(&self, node_id: NodeId, layout: InputLayout) {
        let window = self.cells.window.borrow();
        window.input_layouts.borrow_mut().insert(node_id, layout);
    }

    fn input_request(
        &self,
        node_id: NodeId,
        request: InputRequest,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.apply_input_request(node_id, request, window, cx);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{fake_dispatcher, tree_of_three};
    use gpui::TestAppContext;

    /// `inca-gpui` only ever produces a `button` index in `0..=4`, but
    /// `update_held_buttons` takes a plain `u8`. A value outside that
    /// range must not panic, only fail to set any bit.
    #[test]
    fn an_out_of_range_button_does_not_panic() {
        let (dispatcher, _host, _fake) = fake_dispatcher();
        assert_eq!(dispatcher.update_held_buttons("mousedown", 200), 0);
        assert_eq!(dispatcher.cells.window.borrow().held_buttons, 0);
    }

    #[test]
    fn a_right_mousedown_sets_the_right_buttons_bit() {
        let (dispatcher, _host, _fake) = fake_dispatcher();
        assert_eq!(dispatcher.update_held_buttons("mousedown", 2), 0b0_0010);
    }

    #[test]
    fn a_middle_mousedown_sets_the_middle_buttons_bit() {
        let (dispatcher, _host, _fake) = fake_dispatcher();
        assert_eq!(dispatcher.update_held_buttons("mousedown", 1), 0b0_0100);
    }

    /// Builds the parked handle the way a frame does.
    fn park(cx: &mut TestAppContext, dispatcher: &crate::EventDispatcher) {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let _ = crate::tests::apply_focus(&dispatcher.cells, window, cx);
        });
    }

    #[gpui::test]
    fn the_element_of_the_document_answers_with_the_parked_handle(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake) = fake_dispatcher();
        park(cx, &dispatcher);
        let root = cells.document.borrow().root();

        assert!(inca_gpui::EventSink::focus_handle(&dispatcher, root).is_some());
    }

    #[gpui::test]
    fn the_root_outside_the_document_answers_with_no_handle(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake) = fake_dispatcher();
        park(cx, &dispatcher);
        let root = {
            let state = &mut *cells.document.borrow_mut();
            let (document, root) = (state.document(), state.root());
            state
                .remove_child(&*cells.window.borrow(), document, root)
                .unwrap();
            root
        };

        assert!(inca_gpui::EventSink::focus_handle(&dispatcher, root).is_none());
    }

    #[test]
    fn a_target_gone_from_the_tree_falls_back_to_the_current_target() {
        let (dispatcher, cells, _fake) = fake_dispatcher();
        let [_, parent, child] = tree_of_three(&cells);
        assert_eq!(dispatcher.resolve(Some(child), parent), child);
        drop(
            cells
                .document
                .borrow_mut()
                .destroy_node(&mut *cells.window.borrow_mut(), child),
        );
        assert_eq!(dispatcher.resolve(Some(child), parent), parent);
        assert_eq!(dispatcher.resolve(None, parent), parent);
    }
}
