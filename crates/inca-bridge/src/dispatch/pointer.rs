// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer events of the mouse and the capture of the pointer.

use gpui::{App, Window};
use inca_gpui::event_sink::spec_of;
use inca_gpui::{EventPayload, NO_BUTTON, NodeId, PointerSource, buttons_mask};

use super::EventDispatcher;

mod capture;

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

    /// Returns `payload` with its `buttons` bitmask set to
    /// [`Host::held_buttons`](crate::Host::held_buttons) unless the event carries the buttons of
    /// its own press or release.
    pub(super) fn with_held_buttons(&self, event: &str, mut payload: EventPayload) -> EventPayload {
        let held = self.host.borrow().held_buttons;
        if let Some(mouse) = payload.mouse_mut().filter(|_| !spec_of(event).own_buttons) {
            mouse.buttons = held;
        }
        payload
    }

    /// Updates [`Host::held_buttons`](crate::Host::held_buttons) for a raw `"mousedown"` or
    /// `"mouseup"` of `button` and returns the new value. `button` is `MousePayload`'s own field, a
    /// public part of `inca-gpui`'s API, so an out-of-range value (only 0..=4 are ever produced by
    /// this crate) is handled: [`buttons_mask`] maps it to 0.
    pub(crate) fn update_held_buttons(&self, event: &str, button: u8) -> u8 {
        let bit = buttons_mask(button);
        let mut host = self.host.borrow_mut();
        match event {
            "mousedown" => host.held_buttons |= bit,
            "mouseup" => host.held_buttons &= !bit,
            _ => {}
        }
        host.held_buttons
    }

    /// Returns `payload` with `movementX`/`movementY` set to the delta of the
    /// raw pointer move being dispatched, for `mousemove`. Every other event
    /// keeps 0.
    pub(super) fn with_movement(&self, mut payload: EventPayload, event: &str) -> EventPayload {
        if spec_of(event).movement
            && let EventPayload::Mouse(mouse) = &mut payload
        {
            let (movement_x, movement_y) = self.state.current_movement.get().unwrap_or((0.0, 0.0));
            mouse.movement_x = movement_x;
            mouse.movement_y = movement_y;
        }
        payload
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::pointer::scene;
    use crate::tests::{at, dispatcher_with_engine, mouse_at, wheel_at};
    use gpui::TestAppContext;
    use inca_gpui::{EventPayload, EventSink as _};
    use inca_test_macros::gpui_cases;

    #[gpui::test]
    fn pointerdown_precedes_mousedown(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.down(s.a, 0);
        assert_eq!(
            s.take(),
            [format!("pointerdown@{}", s.a), format!("mousedown@{}", s.a)]
        );
    }

    #[gpui::test]
    fn pointerdown_fires_only_when_first_button_is_pressed(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.down(s.a, 0);
        s.down(s.a, 2);
        let got = s.take();
        let count = |name: &str| got.iter().filter(|e| e.starts_with(name)).count();
        assert_eq!((count("pointerdown"), count("mousedown")), (1, 2));
    }

    #[gpui::test]
    fn pointerup_fires_only_when_last_button_is_released(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.down(s.a, 0);
        s.down(s.a, 2);
        s.take();
        s.up(s.a, s.a, 2);
        let after_first = s
            .take()
            .iter()
            .filter(|e| e.starts_with("pointerup"))
            .count();
        s.up(s.a, s.a, 0);
        let after_last = s
            .take()
            .iter()
            .filter(|e| e.starts_with("pointerup"))
            .count();
        assert_eq!((after_first, after_last), (0, 1));
    }

    #[gpui::test]
    fn pointerup_precedes_mouseup(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.down(s.a, 0);
        s.take();
        s.up(s.a, s.a, 0);
        let a = s.a;
        assert_eq!(
            s.take(),
            [
                format!("pointerup@{a}"),
                format!("mouseup@{a}"),
                format!("click@{a}")
            ]
        );
    }

    #[gpui::test]
    fn extra_button_changes_fire_pointermove(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.listen(
            s.a,
            "pointermove",
            "(e) => out.push([e.button, e.buttons, e.which].join())",
        );
        s.down(s.a, 0);
        s.take();
        s.down(s.a, 2);
        s.up(s.a, s.a, 0);
        let got: Vec<_> = s.take().into_iter().filter(|e| !e.contains('@')).collect();
        assert_eq!(got, ["2,3,3", "0,2,1"]);
    }

    #[gpui::test]
    fn pointermove_shares_movement_with_mousemove(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        let note = "(e) => out.push([e.type, e.movementX, e.movementY].join())";
        s.listen(s.a, "pointermove", note);
        s.listen(s.a, "mousemove", note);
        s.moved(s.a, (10.0, 10.0));
        s.take();
        s.moved(s.a, (30.0, 5.0));
        let got: Vec<_> = s.take().into_iter().filter(|e| !e.contains('@')).collect();
        assert_eq!(got, ["pointermove,20,-5", "mousemove,20,-5"]);
    }

    #[gpui::test]
    fn pointer_boundary_events_report_related_targets(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        let note = "(e) => out.push(`${e.type}<${e.relatedTarget}:${e.bubbles}`)";
        for event in ["pointerover", "pointerout", "pointerenter", "pointerleave"] {
            s.listen(s.a, event, note);
            s.listen(s.b, event, note);
        }
        s.hover(s.a);
        s.take();
        s.hover(s.b);
        let (a, b) = (s.a, s.b);
        let got: Vec<_> = s.take().into_iter().filter(|e| e.contains('<')).collect();
        assert_eq!(
            got,
            [
                format!("pointerout<{b}:true"),
                format!("pointerleave<{b}:false"),
                format!("pointerover<{a}:true"),
                format!("pointerenter<{a}:false"),
            ]
        );
    }

    #[gpui::test]
    fn pointer_boundary_events_come_before_mouse_boundary_events(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.hover(s.a);
        s.take();
        s.moved(s.b, (5.0, 5.0));
        let (a, b) = (s.a, s.b);
        assert_eq!(
            s.take(),
            [
                format!("pointerout@{a}"),
                format!("pointerleave@{a}"),
                format!("pointerover@{b}"),
                format!("pointerenter@{b}"),
                format!("mouseout@{a}"),
                format!("mouseleave@{a}"),
                format!("mouseover@{b}"),
                format!("mouseenter@{b}"),
                format!("pointermove@{b}"),
                format!("mousemove@{b}"),
            ]
        );
    }

    #[gpui::test]
    fn a_canceled_pointerdown_suppresses_mouse_events_until_pointerup(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.eval("globalThis.cancel = true;");
        s.listen(
            s.a,
            "pointerdown",
            r"
            (e) => {
                if (cancel) {
                    e.preventDefault();
                }
            }
            ",
        );
        let root = s.host.borrow().root();
        s.listen(
            root,
            "click",
            "(e) => e.eventPhase === 2 && out.push(`${e.type}@${e.target}`)",
        );
        s.hover(s.a);
        s.take();
        s.down(s.a, 0);
        s.moved(s.b, (5.0, 5.0));
        s.up(s.b, s.a, 0);
        let (a, b) = (s.a, s.b);
        let held = s.take();
        s.eval("globalThis.cancel = false;");
        s.moved(s.b, (6.0, 6.0));
        let after_up = s.take();
        s.down(s.a, 0);
        let next = s.take();
        assert_eq!(
            held,
            [
                format!("pointerdown@{a}"),
                format!("pointerout@{a}"),
                format!("pointerleave@{a}"),
                format!("pointerover@{b}"),
                format!("pointerenter@{b}"),
                format!("mouseout@{a}"),
                format!("mouseleave@{a}"),
                format!("mouseover@{b}"),
                format!("mouseenter@{b}"),
                format!("pointermove@{b}"),
                format!("pointerup@{b}"),
                format!("click@{root}"),
            ]
        );
        assert_eq!(
            after_up,
            [format!("pointermove@{b}"), format!("mousemove@{b}")]
        );
        assert_eq!(next, [format!("pointerdown@{a}"), format!("mousedown@{a}")]);
    }

    #[gpui::test]
    fn pointer_events_carry_the_mouse_pointer_fields(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        let note = r"
            (e) => out.push([
                e.type, e.pointerId, e.pointerType, e.isPrimary, e.width, e.height, e.detail
            ].join())
            ";
        s.listen(s.a, "pointerdown", note);
        s.listen(s.a, "pointerup", note);
        s.down(s.a, 0);
        s.up(s.a, s.a, 0);
        let got: Vec<_> = s.take().into_iter().filter(|e| !e.contains('@')).collect();
        assert_eq!(
            got,
            [
                "pointerdown,1,mouse,true,1,1,0",
                "pointerup,1,mouse,true,1,1,0"
            ]
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::primary(0)]
    #[case::secondary(2)]
    fn pointerdown_and_pointerup_carry_their_button_and_pressure(
        cx: &mut TestAppContext,
        button: u8,
    ) {
        let mut s = scene(cx);
        let note = r"
            (e) => out.push([e.type, e.button, e.buttons, e.which, e.pressure].join())
            ";
        s.listen(s.a, "pointerdown", note);
        s.listen(s.a, "pointerup", note);
        s.down(s.a, button);
        s.up(s.a, s.a, button);
        let got: Vec<_> = s.take().into_iter().filter(|e| !e.contains('@')).collect();
        let mask = inca_gpui::buttons_mask(button);
        let which = u32::from(button) + 1;
        assert_eq!(
            got,
            [
                format!("pointerdown,{button},{mask},{which},0.5"),
                format!("pointerup,{button},0,{which},0"),
            ]
        );
    }

    #[gpui::test]
    fn deactivating_the_window_resets_the_pointer_state(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.listen(
            s.a,
            "pointerdown",
            &format!(
                r"
                (e) => {{
                    e.preventDefault();
                    __inca_native__.setPointerCapture({}, 1);
                }}
                ",
                s.a
            ),
        );
        s.moved(s.b, (5.0, 5.0));
        s.down(s.a, 0);
        s.moved(s.b, (6.0, 6.0));
        s.take();
        let sink = s.dispatcher.clone();
        s.cx.update(|window, cx| sink.window_activated(false, window, cx));
        s.take();
        s.moved(s.b, (7.0, 7.0));
        let b = s.b;
        let got = s.take();
        assert_eq!(
            got.iter()
                .filter(|e| e.starts_with("pointermove") || e.starts_with("mousemove"))
                .collect::<Vec<_>>(),
            [&format!("pointermove@{b}"), &format!("mousemove@{b}")]
        );
        assert!(!got.iter().any(|e| e.starts_with("lostpointercapture")));
        let host = s.host.borrow();
        assert_eq!(
            (host.held_buttons, host.capture_pending, host.capture_active),
            (0, None, None)
        );
    }

    #[gpui::test]
    fn pointer_boundary_events_report_no_button(cx: &mut TestAppContext) {
        let mut s = scene(cx);
        s.listen(
            s.a,
            "pointerover",
            "(e) => out.push([e.button, e.buttons, e.which, e.pressure].join())",
        );
        s.down(s.b, 0);
        s.hover(s.a);
        let got: Vec<_> = s.take().into_iter().filter(|e| !e.contains('@')).collect();
        assert_eq!(got, ["-1,1,0,0.5"]);
    }

    fn movement_of(payload: &EventPayload) -> (f32, f32) {
        match payload {
            EventPayload::Mouse(mouse) => (mouse.movement_x, mouse.movement_y),
            EventPayload::Wheel(wheel) => (wheel.mouse.movement_x, wheel.mouse.movement_y),
            other => panic!("expected a mouse or wheel payload, got {other:?}"),
        }
    }

    #[gpui::test]
    fn a_second_pointer_moved_in_one_cycle_keeps_the_first_delta(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));

        let seen = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(30.0, 5.0), cx);
            dispatcher.pointer_moved(at(100.0, 100.0), cx);
            dispatcher.state.current_movement.get()
        });

        assert_eq!(seen, Some((20.0, -5.0)));
        assert_eq!(dispatcher.state.last_position.get(), Some((30.0, 5.0)));
    }

    #[gpui::test]
    fn pointer_left_makes_the_next_move_report_zero(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));

        dispatcher.pointer_left();
        assert_eq!(dispatcher.state.last_position.get(), None);

        let seen = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(50.0, 60.0), cx);
            dispatcher.state.current_movement.get()
        });
        assert_eq!(seen, Some((0.0, 0.0)));
    }

    #[gpui::test]
    fn only_mousemove_takes_the_movement_of_the_raw_move(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));

        let (moved, others) = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(30.0, 5.0), cx);
            let moved = dispatcher.with_movement(mouse_at(30.0, 5.0), "mousemove");
            let others = [
                "mouseenter",
                "mouseleave",
                "mouseover",
                "mouseout",
                "mousedown",
                "mouseup",
            ]
            .map(|event| movement_of(&dispatcher.with_movement(mouse_at(30.0, 5.0), event)));
            let wheel = dispatcher.with_movement(wheel_at(30.0, 5.0), "wheel");
            (movement_of(&moved), (others, movement_of(&wheel)))
        });

        assert_eq!(moved, (20.0, -5.0));
        assert_eq!(others, ([(0.0, 0.0); 6], (0.0, 0.0)));
    }

    #[test]
    fn a_mousemove_with_no_raw_move_reports_zero() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let payload = dispatcher.with_movement(mouse_at(90.0, 90.0), "mousemove");

        assert_eq!(movement_of(&payload), (0.0, 0.0));
    }

    #[test]
    fn a_click_or_key_payload_is_left_alone() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        dispatcher.state.last_position.set(Some((10.0, 10.0)));

        let payload = dispatcher.with_movement(EventPayload::None, "click");
        assert_eq!(payload, EventPayload::None);
    }
}
