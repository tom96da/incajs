// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

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
