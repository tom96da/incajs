// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// What the callbacks recorded since the last [`take_seen`], in order.
fn take_seen(engine: &Engine) -> Vec<(String, f32, f32)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

fn seen_event(name: &str, x: f32, y: f32) -> (String, f32, f32) {
    (name.to_owned(), x, y)
}

fn move_to(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_mouse_move(point(px(x), px(y)), None::<MouseButton>, Modifiers::none());
}

/// A move over a node nothing listens on still advances the baseline.
#[gpui::test]
fn moves_over_an_unlistened_node_advance_the_movement_baseline(cx: &mut TestAppContext) {
    let panes = build_panes(&["mousemove"], &[]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    move_to(&mut cx, 150.0, 10.0);
    move_to(&mut cx, 160.0, 10.0);
    move_to(&mut cx, 10.0, 10.0);

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mousemove", -150.0, 0.0)]
    );
}

/// A move that leaves a pane gives its `mouseleave` 0 and its `mousemove` the
/// delta, with a `mousedown` between the two moves.
#[gpui::test]
fn mouseleave_reports_zero_where_mousemove_reports_the_delta(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseleave"], &["mousemove", "mousedown"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    cx.simulate_mouse_down(
        point(px(60.0), px(60.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    take_seen(&panes.engine);

    move_to(&mut cx, 200.0, 10.0);

    let mut seen = take_seen(&panes.engine);
    seen.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        seen,
        [
            seen_event("mouseleave", 0.0, 0.0),
            seen_event("mousemove", 190.0, 0.0)
        ]
    );
}

fn wheel_at(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });
}

/// A move with a button held reports the delta like any other move.
#[gpui::test]
fn a_drag_move_reports_the_delta(cx: &mut TestAppContext) {
    let panes = build_panes(&[], &["mousemove"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    cx.simulate_mouse_move(
        point(px(150.0), px(10.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mousemove", 140.0, 0.0)]
    );
}

/// A window exit's `mouseleave` reports 0.
#[gpui::test]
fn a_window_exit_gives_mouseleave_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseleave"], &["mousemove"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    cx.simulate_event(MouseExitEvent {
        position: point(px(10.0), px(10.0)),
        ..Default::default()
    });
    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseleave", 0.0, 0.0)]
    );
}

/// The first move after the pointer comes back reports 0.
#[gpui::test]
fn the_first_move_after_a_window_exit_reports_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseleave"], &["mousemove"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    cx.simulate_event(MouseExitEvent {
        position: point(px(10.0), px(10.0)),
        ..Default::default()
    });
    take_seen(&panes.engine);

    move_to(&mut cx, 50.0, 80.0);
    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mousemove", 0.0, 0.0)]
    );
}

/// A button or wheel event reports 0 movement.
#[gpui::test]
fn button_and_wheel_events_report_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&[], &["mousedown", "mouseup", "wheel", "mousemove"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);
    let at = point(px(30.0), px(40.0));

    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    wheel_at(&mut cx, 30.0, 40.0);
    move_to(&mut cx, 30.0, 40.0);

    let seen = take_seen(&panes.engine);
    assert_eq!(
        seen[..3],
        [
            seen_event("mousedown", 0.0, 0.0),
            seen_event("mouseup", 0.0, 0.0),
            seen_event("wheel", 0.0, 0.0),
        ]
    );
}

/// A button or wheel event keeps the movement baseline, so the move that
/// follows reports the full distance.
#[gpui::test]
fn button_and_wheel_events_keep_the_movement_baseline(cx: &mut TestAppContext) {
    let panes = build_panes(&[], &["mousedown", "mouseup", "wheel", "mousemove"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);
    let at = point(px(30.0), px(40.0));

    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    wheel_at(&mut cx, 30.0, 40.0);
    move_to(&mut cx, 30.0, 40.0);

    let seen = take_seen(&panes.engine);
    assert_eq!(seen[3], seen_event("mousemove", 20.0, 30.0));
}

/// The pointer tracker is a zero-size child: the root and its panes keep
/// their bounds.
#[gpui::test]
fn the_pointer_tracker_leaves_the_layout_unchanged(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &[]);
    let mut cx = mount_panes(cx, &panes);

    let root = cx.debug_bounds("node-1").unwrap();
    assert_eq!(root.origin, point(px(0.0), px(0.0)));
    assert_eq!(root.size, gpui::size(px(300.0), px(100.0)));
    let a = cx.debug_bounds("node-2").unwrap();
    let b = cx.debug_bounds("node-3").unwrap();
    assert_eq!(a.origin, root.origin);
    assert_eq!(b.origin, point(px(100.0), px(0.0)));
    assert_eq!((panes.root, panes.a, panes.b), (1, 2, 3));
}
