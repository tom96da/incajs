// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The plain payloads of the document model read from `gpui` input events.

use gpui::{
    KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollDelta, ScrollEvent,
};

use inca_document::{
    EventPayload, KeyPayload, Modifiers, MousePayload, PointerSource, WheelPayload, buttons_mask,
};

use super::key::key_name;

/// The plain modifier set of `modifiers`. The Fn key sets no flag.
#[must_use]
pub const fn modifiers_of(modifiers: gpui::Modifiers) -> Modifiers {
    Modifiers::none()
        .set(Modifiers::CONTROL, modifiers.control)
        .set(Modifiers::ALT, modifiers.alt)
        .set(Modifiers::SHIFT, modifiers.shift)
        .set(Modifiers::PLATFORM, modifiers.platform)
}

/// The `button` index of a [`MouseButton`]: the position of that button's bit
/// in the `buttons` mask.
pub(crate) const fn button_index(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::Navigate(gpui::NavigationDirection::Back) => 3,
        MouseButton::Navigate(gpui::NavigationDirection::Forward) => 4,
    }
}

/// A mouse payload at `position` with the given `modifiers`, as the hover events carry.
#[must_use]
pub fn mouse_at(position: gpui::Point<gpui::Pixels>, modifiers: gpui::Modifiers) -> MousePayload {
    MousePayload::at(
        f32::from(position.x),
        f32::from(position.y),
        modifiers_of(modifiers),
    )
}

/// The payload of a `mousedown`.
#[must_use]
pub fn mouse_down_payload(event: &MouseDownEvent) -> EventPayload {
    let bit = button_index(event.button);
    EventPayload::Mouse(MousePayload {
        button: bit,
        buttons: buttons_mask(bit),
        detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
        ..mouse_at(event.position, event.modifiers)
    })
}

/// The payload of a `mouseup`.
#[must_use]
pub fn mouse_up_payload(event: &MouseUpEvent) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button: button_index(event.button),
        detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
        ..mouse_at(event.position, event.modifiers)
    })
}

/// The `click` or `auxclick` a release of `event`'s button produces:
/// `button` is the released button, `detail` the click count and `buttons`
/// 0, at the release point.
#[must_use]
pub fn click_payload(event: &MouseUpEvent) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button: button_index(event.button),
        detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
        pointer: Some(PointerSource::Mouse),
        ..mouse_at(event.position, event.modifiers)
    })
}

/// The `contextmenu` a press of `event`'s button produces: `detail` 0 and
/// `buttons` the pressed button.
#[must_use]
pub fn context_menu_payload(event: &MouseDownEvent) -> EventPayload {
    let bit = button_index(event.button);
    EventPayload::Mouse(MousePayload {
        button: bit,
        buttons: buttons_mask(bit),
        pointer: Some(PointerSource::Mouse),
        ..mouse_at(event.position, event.modifiers)
    })
}

/// The payload of a `mousemove`.
#[must_use]
pub fn mouse_move_payload(event: &MouseMoveEvent) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        buttons: event
            .pressed_button
            .map_or(0, |button| buttons_mask(button_index(button))),
        ..mouse_at(event.position, event.modifiers)
    })
}

/// The payload of a `wheel`.
#[must_use]
pub fn wheel_payload(event: &ScrollEvent) -> EventPayload {
    // GPUI reports a downward/rightward scroll as negative; the
    // payload reports it as positive. `0.0 - x` keeps a zero delta at +0.
    let (delta_x, delta_y, delta_mode) = match event.delta {
        ScrollDelta::Pixels(delta) => (0.0 - f32::from(delta.x), 0.0 - f32::from(delta.y), 0),
        ScrollDelta::Lines(delta) => (0.0 - delta.x, 0.0 - delta.y, 1),
    };
    EventPayload::Wheel(WheelPayload {
        mouse: mouse_at(event.position, event.modifiers),
        delta_x,
        delta_y,
        delta_z: 0.0,
        delta_mode,
    })
}

/// The payload of a `keydown`.
#[must_use]
pub fn key_down_payload(event: &KeyDownEvent) -> EventPayload {
    EventPayload::Key(KeyPayload {
        key: key_name(&event.keystroke),
        repeat: event.is_held,
        modifiers: modifiers_of(event.keystroke.modifiers),
        is_composing: false,
    })
}

/// The payload of a `keyup`.
#[must_use]
pub fn key_up_payload(event: &KeyUpEvent) -> EventPayload {
    EventPayload::Key(KeyPayload {
        key: key_name(&event.keystroke),
        // A key release is never a repeat.
        repeat: false,
        modifiers: modifiers_of(event.keystroke.modifiers),
        is_composing: false,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use rstest::rstest;

    fn mouse_of(payload: EventPayload) -> MousePayload {
        match payload {
            EventPayload::Mouse(mouse) => mouse,
            other => panic!("expected a mouse payload, got {other:?}"),
        }
    }

    fn wheel_of(payload: EventPayload) -> WheelPayload {
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
        let mouse = mouse_of(mouse_down_payload(&event));
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
        let mouse = mouse_of(mouse_up_payload(&event));
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
        let mouse = mouse_of(mouse_move_payload(&event));
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
        let mouse = mouse_of(mouse_move_payload(&event));
        assert_eq!(mouse.buttons, 0b0_0100);
    }

    #[test]
    fn a_right_mouse_down_reports_the_right_buttons_bit() {
        let event = MouseDownEvent {
            button: MouseButton::Right,
            click_count: 1,
            ..Default::default()
        };
        let mouse = mouse_of(mouse_down_payload(&event));
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
        let mouse = mouse_of(mouse_down_payload(&event));
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
        let mouse = mouse_of(mouse_down_payload(&event));
        assert_eq!(mouse.modifiers, modifiers_of(modifiers));
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

    #[rstest]
    #[case::left_single(MouseButton::Left, 1, 0)]
    #[case::left_double(MouseButton::Left, 2, 0)]
    #[case::middle(MouseButton::Middle, 1, 1)]
    #[case::right(MouseButton::Right, 1, 2)]
    fn a_click_payload_carries_the_release_point_count_and_button(
        #[case] button: MouseButton,
        #[case] count: usize,
        #[case] want_button: u8,
    ) {
        let shift = gpui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let event = MouseUpEvent {
            button,
            position: gpui::point(gpui::px(3.0), gpui::px(4.0)),
            modifiers: shift,
            click_count: count,
        };
        let want = (
            3.0,
            4.0,
            want_button,
            0,
            u32::try_from(count).unwrap(),
            Some(PointerSource::Mouse),
        );
        let payload = mouse_of(click_payload(&event));
        assert_eq!(field_mismatches(&payload, want), Vec::<String>::new());
        assert_eq!(payload.modifiers, modifiers_of(shift));
    }

    #[test]
    fn a_context_menu_payload_is_the_pressed_button_with_detail_zero() {
        let event = MouseDownEvent {
            button: MouseButton::Right,
            position: gpui::point(gpui::px(8.0), gpui::px(9.0)),
            click_count: 1,
            ..Default::default()
        };
        let payload = mouse_of(context_menu_payload(&event));
        let want = (8.0, 9.0, 2, 0b0_0010, 0, Some(PointerSource::Mouse));
        assert_eq!(field_mismatches(&payload, want), Vec::<String>::new());
    }

    #[test]
    fn a_pixel_delta_reports_delta_mode_0() {
        let event = ScrollEvent {
            delta: ScrollDelta::Pixels(gpui::point(gpui::px(3.0), gpui::px(-4.0))),
            ..Default::default()
        };
        let wheel = wheel_of(wheel_payload(&event));
        assert_eq!(wheel.delta_x, -3.0);
        assert_eq!(wheel.delta_y, 4.0);
        assert_eq!(wheel.delta_z, 0.0);
        assert_eq!(wheel.delta_mode, 0);
    }

    #[test]
    fn a_line_delta_reports_delta_mode_1() {
        let event = ScrollEvent {
            delta: ScrollDelta::Lines(gpui::point(0.0, 2.0)),
            ..Default::default()
        };
        let wheel = wheel_of(wheel_payload(&event));
        assert_eq!(wheel.delta_x, 0.0);
        assert_eq!(wheel.delta_y, -2.0);
        assert_eq!(wheel.delta_mode, 1);
    }

    #[rstest]
    #[case::pixels(ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-0.0))))]
    #[case::lines(ScrollDelta::Lines(gpui::point(0.0, -0.0)))]
    fn a_zero_delta_stays_positive_zero(#[case] delta: ScrollDelta) {
        let event = ScrollEvent {
            delta,
            ..Default::default()
        };
        let wheel = wheel_of(wheel_payload(&event));
        assert!(wheel.delta_x.is_sign_positive() && wheel.delta_y.is_sign_positive());
    }

    #[rstest]
    #[case::control(gpui::Modifiers::control(), Modifiers::CONTROL)]
    #[case::alt(gpui::Modifiers::alt(), Modifiers::ALT)]
    #[case::shift(gpui::Modifiers::shift(), Modifiers::SHIFT)]
    #[case::command(gpui::Modifiers::command(), Modifiers::PLATFORM)]
    #[case::function_sets_no_flag(gpui::Modifiers::function(), Modifiers::none())]
    fn a_gpui_modifier_converts_to_its_plain_flag(
        #[case] held: gpui::Modifiers,
        #[case] want: Modifiers,
    ) {
        assert_eq!(modifiers_of(held), want);
    }

    #[test]
    fn mouse_at_carries_position_and_modifiers() {
        let modifiers = gpui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let mouse = mouse_at(gpui::point(gpui::px(12.0), gpui::px(34.0)), modifiers);
        assert_eq!(mouse.client_x, 12.0);
        assert_eq!(mouse.client_y, 34.0);
        assert_eq!(mouse.button, 0);
        assert_eq!(mouse.buttons, 0);
        assert_eq!(mouse.detail, 0);
        assert_eq!(mouse.modifiers, modifiers_of(modifiers));
    }
}
