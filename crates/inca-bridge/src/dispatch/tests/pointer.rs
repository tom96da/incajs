// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use gpui::{MouseButton, MouseUpEvent, VisualTestContext};
use inca_test_macros::gpui_cases;

mod capture;

const POINTER_AND_MOUSE: [&str; 17] = [
    "pointerdown",
    "pointerup",
    "pointermove",
    "pointerover",
    "pointerout",
    "pointerenter",
    "pointerleave",
    "gotpointercapture",
    "lostpointercapture",
    "mousedown",
    "mouseup",
    "mousemove",
    "mouseover",
    "mouseout",
    "mouseenter",
    "mouseleave",
    "click",
];

/// Two nodes `a` and `b` under the host root, each listening for every pointer and mouse event.
/// A listener records `type@target` into `globalThis.out` where the event is at its target.
struct Scene {
    dispatcher: EventDispatcher,
    host: Rc<RefCell<Host>>,
    cx: VisualTestContext,
    a: NodeId,
    b: NodeId,
}

fn scene(cx: &mut TestAppContext) -> Scene {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let (a, b) = (node_under_root(&host), node_under_root(&host));
    let record = r"
        (e) => {
            if (e.eventPhase === 2) {
                out.push(`${e.type}@${e.target}`);
            }
        }
        ";
    for node in [a, b] {
        for event in POINTER_AND_MOUSE.into_iter().chain(["auxclick"]) {
            listen(&dispatcher, node, event, record);
        }
    }
    Scene {
        dispatcher,
        host,
        cx: cx.add_empty_window().clone(),
        a,
        b,
    }
}

impl Scene {
    /// The events recorded since the last call.
    fn take(&self) -> Vec<String> {
        let out = eval_json(&self.dispatcher, "out.splice(0)");
        serde_json::from_value(out).unwrap()
    }

    fn listen(&self, node: NodeId, event: &str, f: &str) {
        listen(&self.dispatcher, node, event, f);
    }

    /// Reports the pointer over `node` as the hover events do.
    fn hover(&mut self, node: NodeId) {
        let sink = self.dispatcher.clone();
        self.cx
            .update(|window, cx| sink.report_hover(Some(node), window, cx));
        *self.dispatcher.state.hovered.borrow_mut() = [node].into();
    }

    /// A raw move over `node` by `(x, y)` from the previous position.
    fn moved(&mut self, node: NodeId, (x, y): (f32, f32)) {
        let sink = self.dispatcher.clone();
        self.cx.update(|window, cx| {
            sink.pointer_moved(at(x, y), cx);
            sink.pointer_over(node, window, cx);
            sink.pointer_input("mousemove", node, &mouse_at(x, y), window, cx);
        });
        *self.dispatcher.state.hovered.borrow_mut() = [node].into();
    }

    fn down(&mut self, node: NodeId, button: u8) {
        let sink = self.dispatcher.clone();
        self.cx.update(|window, cx| {
            sink.pointer_input("mousedown", node, &button_payload(button), window, cx);
        });
    }

    /// A release of `button` over `node`, pressed over `pressed`.
    fn up(&mut self, node: NodeId, pressed: NodeId, button: u8) {
        self.up_counted(node, pressed, button, 1);
    }

    /// [`Self::up`] for the `count`-th click of a series.
    fn up_counted(&mut self, node: NodeId, pressed: NodeId, button: u8, count: usize) {
        let sink = self.dispatcher.clone();
        let mouse_button =
            [MouseButton::Left, MouseButton::Middle, MouseButton::Right][button as usize];
        let event = MouseUpEvent {
            button: mouse_button,
            position: at(1.0, 1.0),
            modifiers: gpui::Modifiers::none(),
            click_count: count,
        };
        self.cx.update(|window, cx| {
            sink.pointer_input("mouseup", node, &button_payload(button), window, cx);
            sink.pointer_released(&event, Some(pressed), node, window, cx);
        });
    }

    fn eval(&self, js: &str) {
        self.dispatcher.engine.eval::<()>(js).unwrap();
    }
}

fn button_payload(button: u8) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button,
        ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
    })
}

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
fn pointerdown_and_pointerup_carry_their_button_and_pressure(cx: &mut TestAppContext, button: u8) {
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
