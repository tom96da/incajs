// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer events of the mouse and the capture of the pointer.

use gpui::{App, Window};
use inca_gpui::{EventPayload, NO_BUTTON, NodeId, PointerSource};

use super::EventDispatcher;
use crate::EventTargetId;

/// `payload` as a pointer event about `button`: the mouse pointer and no click count.
pub(super) fn pointer_payload(payload: &EventPayload, button: u8) -> EventPayload {
    let mut payload = payload.clone();
    // Every caller passes a mouse payload.
    payload.mouse_mut().into_iter().for_each(|mouse| {
        mouse.button = button;
        mouse.detail = 0;
        mouse.pointer = Some(PointerSource::Mouse);
    });
    payload
}

impl EventDispatcher {
    /// Fires `pointermove` and `mousemove` for a raw move. Boundary events precede them.
    pub(super) fn pointer_move(
        &self,
        target: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.begin_pointer(payload, NO_BUTTON, window, cx);
        let target = self.captured_or(target);
        let pointer = pointer_payload(payload, NO_BUTTON);
        self.fire_inner(target, "pointermove", &pointer, window, cx);
        if !self.state.pointerdown_canceled.get() {
            self.fire_inner(target, "mousemove", payload, window, cx);
        }
        if let EventPayload::Mouse(mouse) = payload {
            self.input_drag_to(mouse, window);
        }
    }

    /// Fires `pointerdown` for the first button pressed and `pointermove` for another one, then
    /// `mousedown` and the default action of a press.
    pub(super) fn pointer_down(
        &self,
        target: NodeId,
        button: u8,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        let first = self.host.borrow().held_buttons == 0;
        self.update_held_buttons("mousedown", button);
        self.begin_pointer(payload, button, window, cx);
        let target = self.captured_or(target);
        self.state.press_target.set(Some(target));
        let pointer = pointer_payload(payload, button);
        if first {
            let canceled = self.fire_inner(target, "pointerdown", &pointer, window, cx);
            self.state.pointerdown_canceled.set(canceled);
        } else {
            self.fire_inner(target, "pointermove", &pointer, window, cx);
        }
        let mut prevented = self.state.pointerdown_canceled.get();
        if !prevented {
            prevented = self.fire_inner(target, "mousedown", payload, window, cx);
        }
        self.focus_on_press(target, prevented, window, cx);
        self.hold_button(target, window, cx);
        if let (false, EventPayload::Mouse(mouse)) = (prevented, payload) {
            self.input_press(target, mouse, cfg!(target_os = "macos"), window);
        }
    }

    /// Fires `pointerup` for the last button released and `pointermove` for another one, then
    /// `mouseup`. The last release ends the capture.
    pub(super) fn pointer_up(
        &self,
        target: NodeId,
        button: u8,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        let last = self.update_held_buttons("mouseup", button) == 0;
        self.begin_pointer(payload, button, window, cx);
        let target = self.captured_or(target);
        let pointer = pointer_payload(payload, button);
        let event = if last { "pointerup" } else { "pointermove" };
        self.fire_inner(target, event, &pointer, window, cx);
        if !self.state.pointerdown_canceled.get() {
            self.fire_inner(target, "mouseup", payload, window, cx);
        }
        self.state.mouse_held.set(None);
        if button == 0 {
            self.state.input_drag.set(None);
        }
        if last {
            self.state.pointerdown_canceled.set(false);
            if self.host.borrow().capture_active.is_some() {
                self.host.borrow_mut().capture_pending = None;
                self.process_capture(payload, button, window, cx);
                self.state.captured_release.set(Some((target, button)));
            }
        }
    }

    /// Applies a capture change a script requested, and reports the boundary events it causes.
    fn begin_pointer(&self, payload: &EventPayload, button: u8, window: &mut Window, cx: &mut App) {
        if self.process_capture(payload, button, window, cx) {
            self.report_hover_with(self.deepest_hovered(), button, window, cx);
        }
    }

    /// The node holding the capture.
    fn captured(&self) -> Option<NodeId> {
        self.host.borrow().capture_active
    }

    /// The node the pointer events of an input over `target` go to.
    fn captured_or(&self, target: NodeId) -> NodeId {
        self.captured().unwrap_or(target)
    }

    /// The deepest container the hover events follow: the capturing node, else `physical`.
    pub(super) fn hover_target(&self, physical: Option<NodeId>) -> Option<NodeId> {
        self.captured().or(physical)
    }

    /// Makes the pending capture the active one. Fires `lostpointercapture` at the old target,
    /// which is `document` when that node left the tree, then `gotpointercapture` at the new one.
    /// The events copy `payload` with the given `button`. Returns whether the capture changed.
    pub(super) fn process_capture(
        &self,
        payload: &EventPayload,
        button: u8,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let (lost, got) = {
            let mut host = self.host.borrow_mut();
            let pending = host
                .capture_pending
                .filter(|&node| host.tree.is_connected(node));
            host.capture_pending = pending;
            let lost = host.capture_active.map(|node| {
                if host.tree.is_connected(node) {
                    EventTargetId::Node(node)
                } else {
                    EventTargetId::Document
                }
            });
            let got = pending.map(EventTargetId::Node);
            if lost == got {
                return false;
            }
            host.capture_active = pending;
            (lost, got)
        };
        let capture = pointer_payload(payload, button);
        for (event, target) in [("lostpointercapture", lost), ("gotpointercapture", got)] {
            match target {
                Some(EventTargetId::Node(node)) => {
                    self.fire(node, event, &capture, window, cx);
                }
                Some(global) => self.fire_at_global(global, event, &capture, window, cx),
                None => {}
            }
        }
        true
    }
}
