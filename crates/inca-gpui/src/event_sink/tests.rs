// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn key_name_prefers_a_named_key_over_its_key_char() {
    let enter = Keystroke {
        key: "enter".to_string(),
        key_char: Some("\n".to_string()),
        modifiers: gpui::Modifiers::default(),
    };
    assert_eq!(key_name(&enter), "Enter");
}

#[test]
fn key_name_names_every_function_key() {
    for n in 1..=35u8 {
        let keystroke = Keystroke {
            key: format!("f{n}"),
            key_char: None,
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(key_name(&keystroke), format!("F{n}"));
    }
}

#[test]
fn key_name_falls_back_to_key_char_for_an_unnamed_key() {
    let shifted_s = Keystroke {
        key: "s".to_string(),
        key_char: Some("S".to_string()),
        modifiers: gpui::Modifiers::default(),
    };
    assert_eq!(key_name(&shifted_s), "S");
}

#[test]
fn key_name_falls_back_to_key_when_key_char_is_none() {
    let cmd_s = Keystroke {
        key: "s".to_string(),
        key_char: None,
        modifiers: gpui::Modifiers::default(),
    };
    assert_eq!(key_name(&cmd_s), "s");
}

#[test]
fn key_name_covers_the_unidentified_and_shifted_fallbacks() {
    let shift = gpui::Modifiers {
        shift: true,
        ..gpui::Modifiers::default()
    };
    let none = gpui::Modifiers::default();
    // (key, key_char, modifiers, expected)
    let cases = [
        ("", None, none, "Unidentified"),
        ("", Some(""), none, "Unidentified"),
        ("s", Some(""), none, "s"),
        ("s", None, shift, "S"),
        ("1", None, shift, "1"),
        ("shift", None, shift, "shift"),
        ("s", Some("S"), shift, "S"),
    ];
    let mismatches: Vec<_> = cases
        .iter()
        .filter_map(|&(key, key_char, modifiers, expected)| {
            let keystroke = Keystroke {
                key: key.to_string(),
                key_char: key_char.map(str::to_string),
                modifiers,
            };
            let got = key_name(&keystroke);
            (got != expected).then(|| format!("{key:?}/{key_char:?}: {got} != {expected}"))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

fn mouse_payload(payload: EventPayload) -> MousePayload {
    match payload {
        EventPayload::Mouse(mouse) => mouse,
        other => panic!("expected a mouse payload, got {other:?}"),
    }
}

fn wheel_payload(payload: EventPayload) -> WheelPayload {
    match payload {
        EventPayload::Wheel(wheel) => wheel,
        other => panic!("expected a wheel payload, got {other:?}"),
    }
}

#[test]
fn a_left_mouse_down_reports_button_zero_held() {
    let event = MouseDownEvent {
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    };
    let mouse = mouse_payload(EventPayload::from(&event));
    assert_eq!(mouse.button, 0);
    assert_eq!(mouse.buttons, 0b0_0001);
    assert_eq!(mouse.detail, 1);
}

#[test]
fn a_right_mouse_up_holds_no_button() {
    let event = MouseUpEvent {
        button: MouseButton::Right,
        click_count: 2,
        ..Default::default()
    };
    let mouse = mouse_payload(EventPayload::from(&event));
    assert_eq!(mouse.button, 2);
    assert_eq!(mouse.buttons, 0);
    assert_eq!(mouse.detail, 2);
}

#[test]
fn a_move_with_no_button_pressed_reports_none_held() {
    let event = MouseMoveEvent {
        pressed_button: None,
        ..Default::default()
    };
    let mouse = mouse_payload(EventPayload::from(&event));
    assert_eq!(mouse.button, 0);
    assert_eq!(mouse.buttons, 0);
    assert_eq!(mouse.detail, 0);
}

#[test]
fn a_move_while_dragging_reports_the_dragged_buttons_bit() {
    let event = MouseMoveEvent {
        pressed_button: Some(MouseButton::Middle),
        ..Default::default()
    };
    let mouse = mouse_payload(EventPayload::from(&event));
    assert_eq!(mouse.buttons, 0b0_0100);
}

#[test]
fn a_right_mouse_down_reports_the_right_buttons_bit() {
    let event = MouseDownEvent {
        button: MouseButton::Right,
        click_count: 1,
        ..Default::default()
    };
    let mouse = mouse_payload(EventPayload::from(&event));
    assert_eq!(mouse.button, 2);
    assert_eq!(mouse.buttons, 0b0_0010);
}

#[test]
fn a_middle_mouse_down_reports_the_middle_buttons_bit() {
    let event = MouseDownEvent {
        button: MouseButton::Middle,
        click_count: 1,
        ..Default::default()
    };
    let mouse = mouse_payload(EventPayload::from(&event));
    assert_eq!(mouse.button, 1);
    assert_eq!(mouse.buttons, 0b0_0100);
}

#[test]
fn modifiers_carry_through_unchanged() {
    let modifiers = gpui::Modifiers {
        control: true,
        platform: true,
        ..Default::default()
    };
    let event = MouseDownEvent {
        modifiers,
        ..Default::default()
    };
    let mouse = mouse_payload(EventPayload::from(&event));
    assert_eq!(mouse.modifiers, modifiers);
}

/// (name, got, want) for each field of `mouse` that differs from `want`.
fn field_mismatches(
    mouse: &MousePayload,
    want: (f32, f32, u8, u8, u32, Option<PointerSource>),
) -> Vec<String> {
    let got = (
        mouse.client_x,
        mouse.client_y,
        mouse.button,
        mouse.buttons,
        mouse.detail,
        mouse.pointer,
    );
    if got == want {
        Vec::new()
    } else {
        vec![format!("{got:?} != {want:?}")]
    }
}

#[test]
fn a_click_payload_carries_the_release_point_count_and_button() {
    let shift = gpui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let release = |button, count| MouseUpEvent {
        button,
        position: gpui::point(gpui::px(3.0), gpui::px(4.0)),
        modifiers: shift,
        click_count: count,
    };
    let mouse = Some(PointerSource::Mouse);
    // (event, expected fields)
    let cases = [
        (release(MouseButton::Left, 1), (3.0, 4.0, 0, 0, 1, mouse)),
        (release(MouseButton::Left, 2), (3.0, 4.0, 0, 0, 2, mouse)),
        (release(MouseButton::Middle, 1), (3.0, 4.0, 1, 0, 1, mouse)),
        (release(MouseButton::Right, 1), (3.0, 4.0, 2, 0, 1, mouse)),
    ];
    let mut mismatches = Vec::new();
    for (event, want) in cases {
        let payload = mouse_payload(EventPayload::click(&event));
        mismatches.extend(field_mismatches(&payload, want));
        if payload.modifiers != shift {
            mismatches.push(format!("modifiers {:?}", payload.modifiers));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

#[test]
fn a_context_menu_payload_is_the_pressed_button_with_detail_zero() {
    let event = MouseDownEvent {
        button: MouseButton::Right,
        position: gpui::point(gpui::px(8.0), gpui::px(9.0)),
        click_count: 1,
        ..Default::default()
    };
    let payload = mouse_payload(EventPayload::context_menu(&event));
    let want = (8.0, 9.0, 2, 0b0_0010, 0, Some(PointerSource::Mouse));
    assert_eq!(field_mismatches(&payload, want), Vec::<String>::new());
}

#[test]
fn a_pixel_delta_reports_delta_mode_0() {
    let event = ScrollWheelEvent {
        delta: ScrollDelta::Pixels(gpui::point(gpui::px(3.0), gpui::px(-4.0))),
        ..Default::default()
    };
    let wheel = wheel_payload(EventPayload::from(&event));
    assert_eq!(wheel.delta_x, -3.0);
    assert_eq!(wheel.delta_y, 4.0);
    assert_eq!(wheel.delta_z, 0.0);
    assert_eq!(wheel.delta_mode, 0);
}

#[test]
fn a_line_delta_reports_delta_mode_1() {
    let event = ScrollWheelEvent {
        delta: ScrollDelta::Lines(gpui::point(0.0, 2.0)),
        ..Default::default()
    };
    let wheel = wheel_payload(EventPayload::from(&event));
    assert_eq!(wheel.delta_x, 0.0);
    assert_eq!(wheel.delta_y, -2.0);
    assert_eq!(wheel.delta_mode, 1);
}

#[test]
fn scrolling_down_or_right_reports_positive_deltas() {
    // GPUI: down and right are negative.
    for delta in [
        ScrollDelta::Pixels(gpui::point(gpui::px(-5.0), gpui::px(-6.0))),
        ScrollDelta::Lines(gpui::point(-5.0, -6.0)),
    ] {
        let event = ScrollWheelEvent {
            delta,
            ..Default::default()
        };
        let wheel = wheel_payload(EventPayload::from(&event));
        assert_eq!((wheel.delta_x, wheel.delta_y), (5.0, 6.0));
    }
}

#[test]
fn a_zero_delta_stays_positive_zero() {
    for delta in [
        ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-0.0))),
        ScrollDelta::Lines(gpui::point(0.0, -0.0)),
    ] {
        let event = ScrollWheelEvent {
            delta,
            ..Default::default()
        };
        let wheel = wheel_payload(EventPayload::from(&event));
        assert!(wheel.delta_x.is_sign_positive() && wheel.delta_y.is_sign_positive());
    }
}

#[test]
fn a_zero_delta_is_still_reported() {
    let event = ScrollWheelEvent {
        delta: ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(0.0))),
        ..Default::default()
    };
    let wheel = wheel_payload(EventPayload::from(&event));
    assert_eq!(wheel.delta_x, 0.0);
    assert_eq!(wheel.delta_y, 0.0);
    assert_eq!(wheel.delta_mode, 0);
}

#[test]
fn mouse_payload_at_carries_position_and_modifiers() {
    let modifiers = gpui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let mouse = MousePayload::at(gpui::point(gpui::px(12.0), gpui::px(34.0)), modifiers);
    assert_eq!(mouse.client_x, 12.0);
    assert_eq!(mouse.client_y, 34.0);
    assert_eq!(mouse.button, 0);
    assert_eq!(mouse.buttons, 0);
    assert_eq!(mouse.detail, 0);
    assert_eq!(mouse.modifiers, modifiers);
}

#[test]
fn the_hover_path_difference_lists_the_containers_left_and_entered() {
    // Paths run nearest first: 3 sits in 2, which sits in 1, then 0.
    let nested = [3, 2, 1, 0];
    let cases: [[&[NodeId]; 4]; 5] = [
        // into a nested container
        [&[0], &nested, &[], &[1, 2, 3]],
        // to a sibling inside the same parent
        [&nested, &[4, 2, 1, 0], &[3], &[4]],
        // out of everything
        [&nested, &[], &[3, 2, 1, 0], &[]],
        // within the same container
        [&nested, &nested, &[], &[]],
        // the first move after entering the window
        [&[], &nested, &[], &[0, 1, 2, 3]],
    ];
    for [old, new, leave, enter] in cases {
        assert_eq!(
            hover_path_difference(old, new),
            (leave.to_vec(), enter.to_vec()),
            "{old:?} -> {new:?}"
        );
    }
}
