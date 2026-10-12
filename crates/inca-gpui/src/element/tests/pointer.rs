// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

use gpui::{MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point};

const LEAF: (f32, f32) = (10., 10.);
const MID: (f32, f32) = (80., 80.);
const ROOT_ONLY: (f32, f32) = (150., 150.);
const OUTSIDE: (f32, f32) = (300., 300.);

fn at((x, y): (f32, f32)) -> Point<Pixels> {
    point(px(x), px(y))
}

/// Runs one of each mouse event at `pos`.
fn mouse_events(cx: &mut VisualTestContext, pos: (f32, f32)) {
    cx.simulate_mouse_move(at(pos), None, gpui::Modifiers::none());
    cx.simulate_event(ScrollWheelEvent {
        position: at(pos),
        delta: ScrollDelta::Pixels(point(px(0.), px(-5.))),
        ..Default::default()
    });
    cx.simulate_click(at(pos), gpui::Modifiers::none());
}

/// `(event, target)` of each report, in order.
fn reports(calls: &Calls) -> Vec<(String, NodeId)> {
    calls.borrow().iter().map(|c| (c.0.clone(), c.1)).collect()
}

fn report(event: &str, target: NodeId) -> (String, NodeId) {
    (event.to_owned(), target)
}

#[gpui::test]
fn every_input_is_reported_once_with_the_deepest_container_as_target(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, true);
    mouse_events(&mut scene.cx, LEAF);
    assert_eq!(
        reports(&scene.calls),
        [
            report("mousemove", 2),
            report("wheel", 2),
            report("mousedown", 2),
            report("mouseup", 2),
            report("release", 2),
        ]
    );
    assert!(
        scene.calls.borrow()[..4].iter().all(|c| c.2 == Some(2)),
        "every container below the root reported before the sink"
    );
}

#[gpui::test]
fn the_target_is_cleared_for_each_event(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, true);
    mouse_events(&mut scene.cx, LEAF);
    scene.calls.borrow_mut().clear();
    mouse_events(&mut scene.cx, MID);
    let targets: Vec<_> = scene.calls.borrow().iter().map(|c| c.1).collect();
    assert_eq!(targets, [1; 5]);
}

#[gpui::test]
fn the_root_is_the_target_when_only_the_root_is_under_the_pointer(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, true);
    mouse_events(&mut scene.cx, ROOT_ONLY);
    let calls = scene.calls.borrow();
    assert!(calls.iter().all(|c| c.1 == 0), "{calls:?}");
    assert!(
        calls[..4].iter().all(|c| c.2.is_none()),
        "the root marks itself after the tracker reports"
    );
}

#[gpui::test]
fn a_sink_that_tracks_no_pointer_gets_the_root_as_every_target(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, false);
    mouse_events(&mut scene.cx, LEAF);
    let calls = scene.calls.borrow();
    assert_eq!(calls.len(), 5);
    assert!(
        calls[..4].iter().all(|c| c.1 == 0 && c.2.is_none()),
        "{calls:?}"
    );
    assert_eq!(calls[4].1, 0);
}

#[gpui::test]
fn a_release_outside_every_container_reports_the_root(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, true);
    scene
        .cx
        .simulate_mouse_down(at(LEAF), MouseButton::Left, gpui::Modifiers::none());
    scene
        .cx
        .simulate_mouse_up(at(OUTSIDE), MouseButton::Left, gpui::Modifiers::none());
    assert_eq!(
        reports(&scene.calls)[1..],
        [report("mouseup", 0), report("release", 0)]
    );
}

/// Every `(event, target, pressed)` after the last `mousedown`.
fn since_press(calls: &Calls) -> Vec<(String, NodeId, Option<NodeId>)> {
    let calls = calls.borrow();
    let last = calls.iter().rposition(|c| c.0 == "mousedown").unwrap();
    calls[last..]
        .iter()
        .map(|c| (c.0.clone(), c.1, c.3))
        .collect()
}

#[gpui::test]
fn the_pressed_target_is_the_press_container_while_the_release_runs(cx: &mut TestAppContext) {
    let none = gpui::Modifiers::none();
    let mut scene = probe_tree(cx, true);
    scene
        .cx
        .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
    scene.cx.simulate_mouse_up(at(MID), MouseButton::Left, none);
    let ev = |name: &str, target, pressed| (name.to_owned(), target, pressed);
    assert_eq!(
        since_press(&scene.calls)[1..],
        [ev("mouseup", 1, Some(2)), ev("release", 1, Some(2))]
    );
    assert_eq!(scene.cx.update(|_, cx| pressed_target(cx, 0)), None);
    scene
        .cx
        .simulate_mouse_down(at(MID), MouseButton::Left, none);
    let down = scene
        .calls
        .borrow()
        .iter()
        .rfind(|c| c.0 == "mousedown")
        .cloned();
    assert_eq!(down.map(|c| c.3), Some(Some(1)));
}

#[gpui::test]
fn a_mouseup_is_followed_by_one_release_report_naming_its_press(cx: &mut TestAppContext) {
    let none = gpui::Modifiers::none();
    let mut scene = probe_tree(cx, true);
    let ev = |name: &str, target, pressed| (name.to_owned(), target, pressed);

    // A release after a completed click has no press to report.
    scene.cx.simulate_click(at(LEAF), none);
    scene
        .cx
        .simulate_mouse_up(at(LEAF), MouseButton::Left, none);
    assert_eq!(scene.calls.borrow().last().unwrap().3, None);

    // A press by another button is not the release's press.
    scene
        .cx
        .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
    scene
        .cx
        .simulate_mouse_up(at(LEAF), MouseButton::Right, none);
    assert_eq!(
        since_press(&scene.calls)[1..],
        [ev("mouseup", 2, None), ev("release", 2, None)]
    );

    // Every release reports, whichever button.
    let releases = |calls: &Calls| calls.borrow().iter().filter(|c| c.0 == "release").count();
    let before = releases(&scene.calls);
    for button in [MouseButton::Right, MouseButton::Middle] {
        scene.cx.simulate_mouse_down(at(LEAF), button, none);
        scene.cx.simulate_mouse_up(at(LEAF), button, none);
    }
    assert_eq!(releases(&scene.calls) - before, 2);
}

#[gpui::test]
fn each_button_keeps_its_own_press_until_its_release(cx: &mut TestAppContext) {
    let none = gpui::Modifiers::none();
    let mut scene = probe_tree(cx, true);
    let release = |calls: &Calls| {
        let calls = calls.borrow();
        let last = calls.iter().rfind(|c| c.0 == "release").unwrap();
        (last.1, last.3)
    };

    scene
        .cx
        .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
    scene
        .cx
        .simulate_mouse_down(at(MID), MouseButton::Right, none);
    scene
        .cx
        .simulate_mouse_up(at(LEAF), MouseButton::Left, none);
    assert_eq!(release(&scene.calls), (2, Some(2)));
    scene
        .cx
        .simulate_mouse_up(at(LEAF), MouseButton::Right, none);
    assert_eq!(release(&scene.calls), (2, Some(1)));
}

#[gpui::test]
fn after_clearing_the_presses_a_release_reports_no_press(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, true);
    let none = gpui::Modifiers::none();

    scene
        .cx
        .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
    scene.cx.update(|_, cx| clear_presses(cx));
    scene
        .cx
        .simulate_mouse_up(at(LEAF), MouseButton::Left, none);

    assert_eq!(since_press(&scene.calls).last().unwrap().2, None);
}

#[gpui::test]
fn only_a_press_prevents_the_default_before_it_is_reported(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, true);
    mouse_events(&mut scene.cx, LEAF);
    let prevented: Vec<_> = scene.prevented.borrow().clone();
    let want = |event: &str, prevented| (event.to_owned(), prevented);
    assert_eq!(
        prevented,
        [
            want("mousemove", false),
            want("wheel", false),
            want("mousedown", true),
            want("mouseup", false),
        ]
    );
}

#[gpui::test]
fn a_right_press_fires_contextmenu_at_its_target_after_the_mousedown(cx: &mut TestAppContext) {
    let mut scene = probe_tree(cx, true);
    let none = gpui::Modifiers::none();
    scene
        .cx
        .simulate_mouse_down(at(LEAF), MouseButton::Right, none);
    scene
        .cx
        .simulate_mouse_down(at(LEAF), MouseButton::Left, none);
    assert_eq!(
        reports(&scene.calls),
        [
            report("mousedown", 2),
            report("fire contextmenu", 2),
            report("mousedown", 2)
        ]
    );
}
