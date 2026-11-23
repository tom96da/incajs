// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::support::pointer::*;
use inca_test_macros::gpui_cases;

/// A `mousedown`'s payload carries coordinates, the button
/// and the buttons held (just the one just pressed).
#[gpui::test]
fn mousedown_carries_the_mouse_fields(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = wiring::engine_for(&host);
    install_click_counter(&engine, node, "mousedown");
    let dispatcher = host.dispatcher(&engine, inca_jsenv::stderr_reporter());

    let window = cx.add_window(|_, _| ClickableRoot {
        host: host.clone(),
        node,
        dispatcher: dispatcher.dispatcher().clone(),
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_mouse_down(
        point(px(10.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert_eq!(clicks(&engine), 1.0);
    let want: serde_json::Value = serde_json::from_str(&format!(
        concat!(
            r#"{{"type":"mousedown","target":{node},"currentTarget":null,"#,
            r#""bubbles":true,"cancelable":true,"composed":true,"#,
            r#""eventPhase":0,"isTrusted":true,"#,
            r#""clientX":10,"clientY":20,"screenX":10,"screenY":20,"offsetX":10,"offsetY":20,"x":10,"y":20,"relatedTarget":null,"pageX":10,"pageY":20,"#,
            r#""movementX":0,"movementY":0,"button":0,"buttons":1,"detail":1,"#,
            r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"#,
            r#""eventId":1}}"#
        ),
        node = node
    ))
    .unwrap();
    assert_eq!(event_fields(&engine, &want), want);
}

/// A `wheel`'s payload carries the delta fields plus
/// every field a `mouse` payload carries.
#[gpui::test]
fn wheel_carries_the_delta_fields(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = wiring::engine_for(&host);
    install_click_counter(&engine, node, "wheel");
    let dispatcher = host.dispatcher(&engine, inca_jsenv::stderr_reporter());

    let window = cx.add_window(|_, _| ClickableRoot {
        host: host.clone(),
        node,
        dispatcher: dispatcher.dispatcher().clone(),
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_event(ScrollEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-5.0))),
        ..Default::default()
    });

    assert_eq!(clicks(&engine), 1.0);
    let want: serde_json::Value = serde_json::from_str(&format!(
        concat!(
            r#"{{"type":"wheel","target":{node},"currentTarget":null,"#,
            r#""bubbles":true,"cancelable":true,"composed":true,"#,
            r#""eventPhase":0,"isTrusted":true,"#,
            r#""clientX":10,"clientY":10,"screenX":10,"screenY":10,"offsetX":10,"offsetY":10,"x":10,"y":10,"relatedTarget":null,"pageX":10,"pageY":10,"#,
            r#""movementX":0,"movementY":0,"button":0,"buttons":0,"detail":0,"#,
            r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"#,
            r#""deltaX":0,"deltaY":5,"deltaZ":0,"deltaMode":0,"#,
            r#""eventId":1}}"#
        ),
        node = node
    ))
    .unwrap();
    assert_eq!(event_fields(&engine, &want), want);
}

/// A mouse button held while scrolling still shows up in a `wheel`'s `buttons`.
#[gpui::test]
fn wheel_reports_a_button_held_from_an_earlier_mousedown(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = wiring::engine_for(&host);
    install_click_counter(&engine, node, "mousedown");
    install_click_counter(&engine, node, "wheel");
    let mut cx = mount_clickable(cx, &host, &engine, node);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Middle,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.simulate_event(ScrollEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });

    let buttons: u8 = engine.eval("globalThis.lastEvent.buttons").unwrap();
    assert_eq!(buttons, 0b0_0100);
}

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
