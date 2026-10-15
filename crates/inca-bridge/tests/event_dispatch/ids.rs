// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

fn mouse_down(cx: &mut VisualTestContext) {
    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
}

#[gpui::test]
fn every_node_on_a_bubble_path_sees_the_same_event_id(cx: &mut TestAppContext) {
    let (mut cx, engine, parent, child) = mount_recording_pair(cx, &["mousedown"]);

    mouse_down(&mut cx);

    assert_eq!(
        take_ids(&engine),
        [
            ("mousedown".to_owned(), child, 1),
            ("mousedown".to_owned(), parent, 1)
        ]
    );
}

#[gpui::test]
fn each_input_event_gets_the_next_event_id(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["mousedown"]);

    mouse_down(&mut cx);
    mouse_down(&mut cx);
    mouse_down(&mut cx);

    let ids: Vec<u64> = take_ids(&engine).iter().map(|seen| seen.2).collect();
    assert_eq!(ids, [1, 1, 2, 2, 3, 3]);
}

#[gpui::test]
fn a_click_has_its_own_event_id_after_its_mousedown(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, child) = mount_recording_pair(cx, &["mousedown", "click"]);

    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());

    let seen = take_ids(&engine);
    let of = |name: &str, node: NodeId| {
        seen.iter()
            .find(|entry| entry.0 == name && entry.1 == node)
            .unwrap()
            .2
    };
    assert!(of("click", child) > of("mousedown", child));
}

#[gpui::test]
fn a_wheel_takes_a_larger_event_id_than_the_mouseenter_before_it(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["wheel", "mouseenter"]);
    let entered = take_ids(&engine);
    assert!(!entered.is_empty() && entered.iter().all(|seen| seen.2 >= 1));
    let before = entered.iter().map(|seen| seen.2).max().unwrap();

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-5.0))),
        ..Default::default()
    });

    let wheel = take_ids(&engine);
    assert_eq!(wheel.len(), 2);
    assert!(
        wheel
            .iter()
            .all(|seen| seen.0 == "wheel" && seen.2 > before)
    );
    assert_eq!(wheel[0].2, wheel[1].2);
}

#[gpui::test]
fn each_mouseleave_of_one_move_takes_its_own_event_id(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["mouseleave"]);
    take_ids(&engine);

    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    let seen = take_ids(&engine);
    assert_eq!(seen.len(), 2, "the child and the parent are left");
    assert!(seen.iter().all(|entry| entry.0 == "mouseleave"));
    assert!(seen[0].2 >= 1 && seen[1].2 > seen[0].2, "{seen:?}");
}

#[gpui::test]
fn a_moves_mousemove_gets_a_later_id_than_its_mouseenter(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["mousemove", "mouseenter"]);
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    take_ids(&engine);

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    let seen = take_ids(&engine);
    let first_of = |name: &str| seen.iter().find(|entry| entry.0 == name).unwrap().2;
    assert!(first_of("mousemove") > first_of("mouseenter"));
}
