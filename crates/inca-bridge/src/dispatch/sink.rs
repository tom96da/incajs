// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::rc::Rc;

use gpui::{App, MouseButton, MouseUpEvent, ScrollHandle, Window};
use inca_gpui::{EventPayload, EventSink, KeyPayload, MousePayload, NodeId};

use super::EventDispatcher;
use inca_gpui::element::scroll::Align;

impl EventSink for EventDispatcher {
    fn tracks_pointer(&self) -> bool {
        self.tracks_pointer()
    }

    fn clear_layout(&self) {
        self.host.borrow().layout.borrow_mut().clear();
    }

    fn record_layout(&self, node_id: NodeId, layout: inca_gpui::NodeLayout) {
        self.host
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
        self.fire(start, event, payload, window, cx)
    }

    fn focus_handle(&self, node_id: NodeId) -> Option<gpui::FocusHandle> {
        let host = self.host.borrow();
        host.focus.handle(node_id).or_else(|| {
            (node_id == host.root())
                .then(|| host.focus.parked_handle())
                .flatten()
        })
    }

    fn scroll_handle(&self, node_id: NodeId) -> Option<ScrollHandle> {
        Some(self.host.borrow().scroll_handle(node_id))
    }

    fn wheel_axes(&self, node_id: NodeId) -> Option<gpui::Point<bool>> {
        inca_gpui::wheel_axes(&self.host.borrow().tree, node_id)
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
                    modifiers,
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
        let Some(pressed) = pressed else {
            return;
        };
        let Some(start) = self.common_ancestor(pressed, released) else {
            return;
        };
        let mut payload = EventPayload::click(event);
        if event.button != MouseButton::Left {
            self.fire(start, "auxclick", &payload, window, cx);
            return;
        }
        self.fire(start, "click", &payload, window, cx);
        if event.click_count == 2 {
            // `dblclick` carries the mouse fields only.
            if let Some(mouse) = payload.mouse_mut() {
                mouse.pointer = None;
            }
            self.fire(start, "dblclick", &payload, window, cx);
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
        // The default action reads the tree the listeners' queued jobs leave.
        let canceled = self.fire_inner(target, event, payload, window, cx);
        match event {
            "mousedown" => {
                self.focus_on_press(target, canceled, window, cx);
                self.hold_button(target, window, cx);
            }
            "mouseup" => self.state.mouse_held.set(None),
            // The wheel step reads `Window::default_prevented`.
            "wheel" if canceled => window.prevent_default(),
            _ => {}
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

    fn key_input(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.host.borrow_mut().focus.observe(window, cx);
        let target = self.key_target(window, cx);
        let event = if up { "keyup" } else { "keydown" };
        let (canceled, entered) = self.walk(target, event, payload, &[]);
        if entered {
            self.drain_observed(window);
        }
        if canceled {
            // Handled for the platform: a shortcut or an input method sees nothing.
            window.prevent_default();
            cx.stop_propagation();
        }
        self.key_default(keystroke, up, canceled, window, cx);
    }
}

impl EventDispatcher {
    fn focused_enabled_button(&self, window: &Window, cx: &App) -> Option<NodeId> {
        let host = self.host.borrow();
        host.focus.focused_node(window, cx).filter(|id| {
            host.tree.get(*id).is_some_and(|node| {
                node.tag_name() == "button" && !inca_gpui::is_disabled_button(node)
            })
        })
    }

    /// Records the focused enabled button a mouse press landed on.
    fn hold_button(&self, target: NodeId, window: &Window, cx: &App) {
        let window_id = window.window_handle().window_id();
        let held = self
            .focused_enabled_button(window, cx)
            .filter(|button| self.path_from(target).contains(button));
        self.state
            .mouse_held
            .set(held.map(|button| (window_id, button)));
    }

    // Tab moves focus and dispatches nothing here. The next frame reports it.
    fn key_default(
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
        let focused_button = || self.focused_enabled_button(window, cx);
        let plain = !(modifiers.control || modifiers.alt || modifiers.platform);
        let click = match (key, up) {
            ("tab", false) if plain => {
                let host = &mut *self.host.borrow_mut();
                let moved =
                    host.focus
                        .tab_move(&host.tree, host.root(), window, cx, modifiers.shift);
                if let Some(node) = moved {
                    host.reveal(node, Align::Reveal, Align::CenterOutside);
                }
                None
            }
            ("enter", false) if plain => {
                let button = focused_button();
                if button.is_some() {
                    self.state.space_down.set(None);
                }
                button
            }
            ("space", false) => {
                self.state
                    .space_down
                    .set(focused_button().map(|button| (window_id, button)));
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
            let payload = EventPayload::Mouse(MousePayload::keyboard_click(modifiers));
            self.fire(button, "click", &payload, window, cx);
        }
    }
}
