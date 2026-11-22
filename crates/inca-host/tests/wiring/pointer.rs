// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of pointer events.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_document::{EventPayload, EventTargetId};
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::support::*;
use inca_document::{EVENTS, MousePayload, PointerSource};
use inca_gpui::{EventSink, InputLayout};
use inca_node::NodeId;

// The pointer fields, the button, the buttons, `which` and the pressure are what JS reads of
// the payload the dispatch builds, so these three run on an engine.
#[gpui::test]
fn pointer_events_carry_the_mouse_pointer_fields(cx: &mut TestAppContext) {
    let mut s = js_pointer_scene(cx);
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
    let mut s = js_pointer_scene(cx);
    let note = r"
            (e) => out.push([e.type, e.button, e.buttons, e.which, e.pressure].join())
            ";
    s.listen(s.a, "pointerdown", note);
    s.listen(s.a, "pointerup", note);
    s.down(s.a, button);
    s.up(s.a, s.a, button);
    let got: Vec<_> = s.take().into_iter().filter(|e| !e.contains('@')).collect();
    let mask = inca_document::buttons_mask(button);
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
fn pointer_boundary_events_report_no_button(cx: &mut TestAppContext) {
    let mut s = js_pointer_scene(cx);
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

#[gpui_cases]
#[gpui::test]
#[case::move_cause(false, "-1,1,0,0.5")]
#[case::pointerup_cause(true, "0,0,1,0")]
fn gotpointercapture_copies_the_causing_event(cx: &mut TestAppContext, release: bool, want: &str) {
    let mut s = js_pointer_scene(cx);
    s.listen(
        s.a,
        "pointerdown",
        &format!("() => __inca_native__.setPointerCapture({}, 1)", s.a),
    );
    s.listen(
        s.a,
        "gotpointercapture",
        "(e) => out.push([e.button, e.buttons, e.which, e.pressure].join())",
    );
    s.down(s.a, 0);
    s.take();
    if release {
        s.up(s.a, s.a, 0);
    } else {
        s.moved(s.a, (2.0, 2.0));
    }
    let a = s.a;
    let got: Vec<_> = s.take().into_iter().filter(|e| !e.contains('@')).collect();
    assert_eq!(got, [want], "capture of {a}");
}
