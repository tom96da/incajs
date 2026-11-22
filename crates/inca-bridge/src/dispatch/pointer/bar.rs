// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer events of a press on a scrollbar. They go to the scroll container and change
//! nothing else: no focus, no pressed state and no `click`.

use gpui::{App, Window};
use inca_document::{EventPayload, NO_BUTTON};
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
        let first = self.host.window.borrow().held_buttons == 0;
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

    /// Ends the freeze of the pointer moves and the hover state for a thumb drag: the freeze of
    /// the container `only`, or the freeze of any container for `None`. It fires the boundary
    /// events for the pointer's position.
    pub(in crate::dispatch) fn end_bar_drag(
        &self,
        only: Option<NodeId>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let held = self.state.bar_drag.get();
        if held.is_some() && only.is_none_or(|node| held == Some(node)) {
            self.state.bar_drag.set(None);
            self.report_hover_with(self.deepest_hovered(), NO_BUTTON, window, cx);
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
            if self.host.window.borrow().capture_active.is_some() {
                self.host.document.borrow_mut().capture_pending = None;
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
        s.cancel("pointerdown", std::rc::Rc::new(std::cell::Cell::new(true)));
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
        s.capture_on_pointerdown(s.a);
        s.bar_down(s.a, 0);
        s.bar_up(s.a, 0);
        let a = s.a;
        assert!(s.take().contains(&format!("lostpointercapture@{a}")));
        let pending = s.host.document.borrow().capture_pending;
        let active = s.host.window.borrow().capture_active;
        assert_eq!((pending, active), (None, None));
    }

    /// The events of a move over `b` with the left button held during a drag of `a`.
    fn held_move_during_a_drag(cx: &mut TestAppContext) -> Vec<String> {
        let mut s = scene(cx);
        s.hover(s.a);
        s.take();
        s.bar_drag(s.a, true);
        s.moved_held(s.b, (5.0, 5.0));
        s.take()
    }

    #[gpui::test]
    fn a_thumb_drag_fires_no_pointer_or_mouse_move(cx: &mut TestAppContext) {
        let got = held_move_during_a_drag(cx);
        let moves = |e: &String| e.starts_with("pointermove") || e.starts_with("mousemove");
        assert!(!got.iter().any(moves), "{got:?}");
    }

    #[gpui::test]
    fn a_thumb_drag_fires_no_boundary_event(cx: &mut TestAppContext) {
        let got = held_move_during_a_drag(cx);
        assert!(got.is_empty(), "{got:?}");
    }

    #[gpui::test]
    fn a_move_with_no_button_held_fires_mousemove_during_a_drag(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.hover(s.a);
        s.take();
        s.bar_drag(s.a, true);
        s.moved(s.b, (5.0, 5.0));
        let b = s.b;
        assert!(s.take().contains(&format!("mousemove@{b}")));
    }

    #[gpui::test]
    fn the_end_of_a_drag_fires_the_boundary_events_after_the_mouseup(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.hover(s.a);
        s.take();
        s.bar_down(s.a, 0);
        s.bar_drag(s.a, true);
        *s.dispatcher.state.hovered.borrow_mut() = [s.b].into();
        s.bar_up(s.a, 0);
        s.bar_drag(s.a, false);
        let (a, b) = (s.a, s.b);
        let got = s.take();
        let order: Vec<_> = [
            format!("mouseup@{a}"),
            format!("mouseout@{a}"),
            format!("mouseleave@{a}"),
            format!("mouseover@{b}"),
            format!("mouseenter@{b}"),
        ]
        .iter()
        .map(|event| got.iter().position(|e| e == event).unwrap())
        .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{got:?}");
    }

    #[gpui::test]
    fn the_end_of_another_containers_drag_leaves_the_freeze_in_place(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.bar_drag(s.b, true);
        s.bar_drag(s.a, false);
        assert_eq!(s.dispatcher.state.bar_drag.get(), Some(s.b));
    }

    #[gpui::test]
    fn the_release_that_ends_a_capture_fires_the_boundary_events_at_the_pointer(
        cx: &mut TestAppContext,
    ) {
        let mut s = scene(cx);
        s.capture_on_pointerdown(s.a);
        s.hover(s.a);
        s.bar_down(s.a, 0);
        *s.dispatcher.state.hovered.borrow_mut() = [s.b].into();
        s.take();
        s.bar_up(s.a, 0);
        let b = s.b;
        assert!(s.take().contains(&format!("mouseover@{b}")));
    }
}
