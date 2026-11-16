// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer events of a press on a scrollbar. They go to the scroll container and change
//! nothing else: no focus, no pressed state and no `click`.

use gpui::{App, Window};
use inca_gpui::EventPayload;
use inca_node::NodeId;

use super::{EventDispatcher, pointer_payload};

impl EventDispatcher {
    /// Fires `pointerdown` for the first button pressed and `pointermove` for another one, then
    /// `mousedown`, at the container `target` of a bar.
    pub(in crate::dispatch) fn bar_press(
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
        if !self.state.pointerdown_canceled.get() {
            self.fire_inner(target, "mousedown", payload, window, cx);
        }
    }

    /// Fires `pointerup` for the last button released and `pointermove` for another one, then
    /// `mouseup`, at the container `target` of a bar.
    pub(in crate::dispatch) fn bar_release(
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
        if last {
            self.state.pointerdown_canceled.set(false);
            if self.host.borrow().capture_active.is_some() {
                self.host.borrow_mut().capture_pending = None;
                self.process_capture(payload, button, window, cx);
                self.report_hover_with(self.deepest_hovered(), button, window, cx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::pointer::scene;
    use gpui::TestAppContext;

    #[gpui::test]
    fn a_bar_press_and_release_fire_the_pointer_and_mouse_events_at_the_container(
        cx: &mut TestAppContext,
    ) {
        let mut s = scene(cx);
        s.bar_down(s.a, 0);
        s.bar_up(s.a, 0);
        let a = s.a;
        assert_eq!(
            s.take(),
            [
                format!("pointerdown@{a}"),
                format!("mousedown@{a}"),
                format!("pointerup@{a}"),
                format!("mouseup@{a}"),
            ]
        );
    }

    #[gpui::test]
    fn a_canceled_pointerdown_on_a_bar_suppresses_the_mouse_events(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.listen(s.a, "pointerdown", "(e) => e.preventDefault()");
        s.bar_down(s.a, 0);
        s.bar_up(s.a, 0);
        let a = s.a;
        assert_eq!(
            s.take(),
            [format!("pointerdown@{a}"), format!("pointerup@{a}")]
        );
    }

    #[gpui::test]
    fn a_second_button_on_a_bar_fires_pointermove_until_the_last_release(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.bar_down(s.a, 0);
        s.bar_down(s.a, 2);
        s.bar_up(s.a, 2);
        s.bar_up(s.a, 0);
        let a = s.a;
        let got: Vec<_> = s
            .take()
            .into_iter()
            .filter(|e| e.starts_with("pointer"))
            .collect();
        assert_eq!(
            got,
            [
                format!("pointerdown@{a}"),
                format!("pointermove@{a}"),
                format!("pointermove@{a}"),
                format!("pointerup@{a}"),
            ]
        );
    }

    #[gpui::test]
    fn the_last_release_on_a_bar_ends_a_pointer_capture(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.listen(
            s.a,
            "pointerdown",
            &format!("(e) => __inca_native__.setPointerCapture({}, 1)", s.a),
        );
        s.bar_down(s.a, 0);
        s.bar_up(s.a, 0);
        let a = s.a;
        assert!(s.take().contains(&format!("lostpointercapture@{a}")));
        let host = s.host.borrow();
        assert_eq!((host.capture_pending, host.capture_active), (None, None));
    }
}
