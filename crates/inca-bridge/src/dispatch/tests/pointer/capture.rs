// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

/// A scene where a press on `a` captures `capturer` from its `pointerdown` listener. The pointer
/// is hovering `a`.
fn captured(cx: &mut TestAppContext, capturer: &str) -> Scene {
    let mut s = scene(cx);
    let target = if capturer == "a" { s.a } else { s.b };
    s.listen(
        s.a,
        "pointerdown",
        &format!("() => __inca_native__.setPointerCapture({target}, 1)"),
    );
    s.hover(s.a);
    s.down(s.a, 0);
    s.take();
    s
}

fn only<'a>(events: &'a [String], prefixes: &[&str]) -> Vec<&'a String> {
    events
        .iter()
        .filter(|e| prefixes.iter().any(|p| e.starts_with(p)))
        .collect()
}

#[gpui::test]
fn pointer_capture_starts_before_the_next_pointer_event(cx: &mut TestAppContext) {
    let mut s = scene(cx);
    s.listen(
        s.a,
        "pointerdown",
        &format!("() => __inca_native__.setPointerCapture({}, 1)", s.b),
    );
    s.hover(s.a);
    s.take();
    s.down(s.a, 0);
    let (a, b) = (s.a, s.b);
    assert_eq!(
        s.take(),
        [format!("pointerdown@{a}"), format!("mousedown@{a}")]
    );
    s.moved(a, (2.0, 2.0));
    assert_eq!(
        s.take()[..3],
        [
            format!("gotpointercapture@{b}"),
            format!("pointerout@{a}"),
            format!("pointerleave@{a}"),
        ]
    );
}

#[gpui::test]
fn captured_pointer_motion_does_not_follow_physical_boundaries(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let b = s.b;
    s.moved(b, (5.0, 5.0));
    s.moved(b, (6.0, 6.0));
    let got = s.take();
    assert!(
        only(
            &got,
            &["pointerover", "pointerout", "mouseover", "mouseout"]
        )
        .is_empty()
    );
}

#[gpui::test]
fn pointermove_and_mousemove_keep_capture_target(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let (a, b) = (s.a, s.b);
    s.moved(b, (5.0, 5.0));
    assert_eq!(
        only(&s.take(), &["pointermove", "mousemove"]),
        [&format!("pointermove@{a}"), &format!("mousemove@{a}")]
    );
}

#[gpui::test]
fn pointer_capture_survives_nonfinal_button_release(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let (a, b) = (s.a, s.b);
    s.down(a, 2);
    s.up(b, a, 2);
    s.moved(b, (5.0, 5.0));
    let got = s.take();
    assert!(only(&got, &["lostpointercapture"]).is_empty());
    assert!(got.contains(&format!("pointermove@{a}")));
}

#[gpui::test]
fn pointer_capture_ends_after_final_pointerup(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let (a, b) = (s.a, s.b);
    s.moved(b, (5.0, 5.0));
    s.take();
    s.up(b, a, 0);
    assert_eq!(
        s.take(),
        [
            format!("pointerup@{a}"),
            format!("mouseup@{a}"),
            format!("lostpointercapture@{a}"),
            format!("click@{a}"),
            format!("pointerout@{a}"),
            format!("pointerleave@{a}"),
            format!("pointerover@{b}"),
            format!("pointerenter@{b}"),
            format!("mouseout@{a}"),
            format!("mouseleave@{a}"),
            format!("mouseover@{b}"),
            format!("mouseenter@{b}"),
        ]
    );
}

#[gpui_cases]
#[gpui::test]
#[case::click(0, "click")]
#[case::auxclick(1, "auxclick")]
fn captured_click_and_auxclick_keep_the_pointerup_target(
    cx: &mut TestAppContext,
    button: u8,
    click: &str,
) {
    let mut s = scene(cx);
    s.listen(
        s.a,
        "pointerdown",
        &format!("() => __inca_native__.setPointerCapture({}, 1)", s.b),
    );
    s.down(s.a, button);
    s.moved(s.a, (5.0, 5.0));
    s.take();
    s.up(s.a, s.a, button);
    let b = s.b;
    assert!(s.take().contains(&format!("{click}@{b}")));
}

#[gpui::test]
fn captured_dblclick_keeps_the_pointerup_target(cx: &mut TestAppContext) {
    let mut s = scene(cx);
    s.listen(s.a, "dblclick", "(e) => out.push(`${e.type}@${e.target}`)");
    s.listen(s.b, "dblclick", "(e) => out.push(`${e.type}@${e.target}`)");
    s.listen(
        s.a,
        "pointerdown",
        &format!("() => __inca_native__.setPointerCapture({}, 1)", s.b),
    );
    s.down(s.a, 0);
    s.moved(s.a, (5.0, 5.0));
    s.take();
    s.up_counted(s.a, s.a, 0, 2);
    let b = s.b;
    assert!(s.take().contains(&format!("dblclick@{b}")));
}

#[gpui::test]
fn pointer_capture_replacement_releases_previous_target(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let (a, b) = (s.a, s.b);
    s.moved(a, (2.0, 2.0));
    s.take();
    s.eval(&format!("__inca_native__.setPointerCapture({b}, 1);"));
    s.moved(a, (3.0, 3.0));
    assert_eq!(
        only(&s.take(), &["lost", "got"]),
        [
            &format!("lostpointercapture@{a}"),
            &format!("gotpointercapture@{b}")
        ]
    );
}

#[gpui::test]
fn setting_the_same_pointer_capture_is_a_noop(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let a = s.a;
    s.eval(&format!("__inca_native__.setPointerCapture({a}, 1);"));
    s.moved(a, (2.0, 2.0));
    s.eval(&format!("__inca_native__.setPointerCapture({a}, 1);"));
    s.moved(a, (3.0, 3.0));
    let got = s.take();
    assert_eq!(
        only(&got, &["got", "lost"]),
        [&format!("gotpointercapture@{a}")]
    );
}

#[gpui::test]
fn pointer_capture_release_is_pending_until_next_pointer_event(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let (a, b) = (s.a, s.b);
    s.moved(b, (5.0, 5.0));
    s.take();
    s.eval(&format!("__inca_native__.releasePointerCapture({a}, 1);"));
    assert!(s.take().is_empty());
    s.moved(b, (6.0, 6.0));
    let got = s.take();
    assert_eq!(
        got[..4],
        [
            format!("lostpointercapture@{a}"),
            format!("pointerout@{a}"),
            format!("pointerleave@{a}"),
            format!("pointerover@{b}"),
        ]
    );
    assert_eq!(only(&got, &["pointermove"]), [&format!("pointermove@{b}")]);
}

#[gpui::test]
fn pointer_capture_follows_a_node_that_is_moved(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let (a, b) = (s.a, s.b);
    s.moved(a, (2.0, 2.0));
    s.host.borrow_mut().tree.append_child(b, a).unwrap();
    s.take();
    s.moved(b, (5.0, 5.0));
    let got = s.take();
    assert!(only(&got, &["lostpointercapture"]).is_empty());
    assert!(got.contains(&format!("pointermove@{a}")));
}

#[gpui_cases]
#[gpui::test]
#[case::move_cause(false, "-1,1,0,0.5")]
#[case::pointerup_cause(true, "0,0,1,0")]
fn gotpointercapture_copies_the_causing_event(cx: &mut TestAppContext, release: bool, want: &str) {
    let mut s = scene(cx);
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

#[gpui::test]
fn the_contextmenu_of_a_captured_press_goes_to_the_pointerdown_target(cx: &mut TestAppContext) {
    let mut s = scene(cx);
    s.listen(
        s.a,
        "contextmenu",
        "(e) => out.push(`${e.type}@${e.target}`)",
    );
    s.listen(
        s.b,
        "contextmenu",
        "(e) => out.push(`${e.type}@${e.target}`)",
    );
    s.listen(
        s.a,
        "pointerdown",
        &format!("() => __inca_native__.setPointerCapture({}, 1)", s.a),
    );
    s.down(s.a, 2);
    s.moved(s.b, (5.0, 5.0));
    s.down(s.b, 0);
    s.take();
    let sink = s.dispatcher.clone();
    let payload = EventPayload::Mouse(MousePayload {
        pointer: Some(PointerSource::Mouse),
        ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
    });
    s.cx.update(|window, cx| EventSink::fire(&sink, s.b, "contextmenu", &payload, window, cx));
    assert_eq!(s.take(), [format!("contextmenu@{}", s.a)]);
}

#[gpui::test]
fn removed_capture_reports_loss_at_document(cx: &mut TestAppContext) {
    let mut s = captured(cx, "a");
    let (a, b) = (s.a, s.b);
    s.moved(a, (2.0, 2.0));
    listen_global(
        &s.dispatcher,
        "$document",
        "lostpointercapture",
        false,
        "(e) => out.push(`${e.type}@${e.target}`)",
    );
    s.take();
    s.host.borrow_mut().tree.destroy_node(a);
    s.moved(b, (5.0, 5.0));
    let got = s.take();
    assert_eq!(
        only(&got, &["lost"]),
        [&"lostpointercapture@$document".to_owned()]
    );
    assert!(!got.iter().any(|e| e.ends_with(&format!("@{a}"))));
}

#[gpui::test]
fn deactivating_the_window_fires_lostpointercapture_at_the_captured_target(
    cx: &mut TestAppContext,
) {
    let mut s = captured(cx, "a");
    let a = s.a;
    s.moved(a, (2.0, 2.0));
    s.take();
    let sink = s.dispatcher.clone();
    s.cx.update(|window, cx| sink.window_activated(false, window, cx));
    assert_eq!(s.take(), [format!("lostpointercapture@{a}")]);
}
