// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::{
    App, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ScrollDelta, ScrollWheelEvent, Window,
};

use crate::element::NodeLayout;
use crate::input::{InputLayout, InputPayload, InputRequest, InputState, InputType};
use inca_node::tree::NodeId;

mod key;
mod sink;
mod spec;

pub use key::is_menu_key;
use key::key_name;
pub use sink::EventSink;
pub(crate) use sink::noop_sink_methods;
pub use spec::{EVENTS, EventSpec, Flags, flags_of, spec_of};

/// What a dispatch carries beyond the node it fired on.
#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    /// No data beyond the node and the event's name.
    None,
    /// A mouse position, button, and modifier state, as in a mouse event.
    Mouse(MousePayload),
    /// A scroll delta plus the same fields as [`EventPayload::Mouse`],
    /// as in a wheel event.
    Wheel(WheelPayload),
    /// A key and modifier state, as in a keyboard event.
    Key(KeyPayload),
    /// The other node of a focus change.
    /// `related_target` is the node gaining focus for `blur` and `focusout`,
    /// the node losing it for `focus` and `focusin`, and `None` when there
    /// is no such node.
    Focus { related_target: Option<NodeId> },
    /// The data, `inputType` and composition flag of `beforeinput` and
    /// `input`.
    Input(InputPayload),
    /// The text of a composition event.
    Composition { data: String },
}

/// [`EventPayload::Mouse`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MousePayload {
    pub client_x: f32,
    pub client_y: f32,
    /// The pointer's travel in pixels since the previous pointer move
    /// (`movementX`/`movementY`). 0 when built by this crate; the
    /// [`EventSink`] implementor supplies the real value.
    pub movement_x: f32,
    pub movement_y: f32,
    /// The window's position on the screen plus the client position
    /// (`screenX`/`screenY`). 0 when built by this crate; the [`EventSink`]
    /// implementor supplies the real value.
    pub screen_x: f32,
    pub screen_y: f32,
    /// The position relative to the padding edge of the event's `target`
    /// (`offsetX`/`offsetY`). 0 when built by this crate; the [`EventSink`]
    /// implementor supplies the real value.
    pub offset_x: f32,
    pub offset_y: f32,
    /// The button this event is about. 0 for a mouse move, which isn't about any
    /// one button. [`NO_BUTTON`] for a pointer event about no button.
    pub button: u8,
    /// Every button held during this event, as a bitmask.
    pub buttons: u8,
    /// How many clicks this is part of. 0 for a move.
    pub detail: u32,
    pub modifiers: gpui::Modifiers,
    /// Set for the pointer events, `click`, `auxclick` and `contextmenu`, which carry the pointer
    /// fields.
    pub pointer: Option<PointerSource>,
    /// `relatedTarget`: the node the pointer came from for `mouseover`
    /// and `mouseenter`, the node it went to for `mouseout` and `mouseleave`.
    pub related_target: Option<NodeId>,
}

/// The `button` of a pointer event that is about no button: it reads -1.
pub const NO_BUTTON: u8 = u8::MAX;

/// What produced a `PointerEvent`-typed event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerSource {
    /// The mouse: `pointerId` 1 and `pointerType` `"mouse"`. `isPrimary` is true on the pointer
    /// events and false on `click`, `auxclick` and `contextmenu`.
    Mouse,
    /// The keyboard: `pointerId` -1, `pointerType` `""`, `isPrimary` false.
    Keyboard,
}

impl MousePayload {
    /// A payload at `position` with the given `modifiers`, as the hover events
    /// carry. The movement fields are 0 and `related_target` is `None`.
    #[must_use]
    pub fn at(position: gpui::Point<gpui::Pixels>, modifiers: gpui::Modifiers) -> Self {
        Self {
            client_x: f32::from(position.x),
            client_y: f32::from(position.y),
            movement_x: 0.0,
            movement_y: 0.0,
            screen_x: 0.0,
            screen_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            button: 0,
            buttons: 0,
            detail: 0,
            modifiers,
            pointer: None,
            related_target: None,
        }
    }

    /// The `click` a key press produces: position, `button`, `buttons` and
    /// `detail` at 0, with the key event's `modifiers`.
    #[must_use]
    pub fn keyboard_click(modifiers: gpui::Modifiers) -> Self {
        Self {
            pointer: Some(PointerSource::Keyboard),
            ..Self::at(gpui::Point::default(), modifiers)
        }
    }
}

/// [`EventPayload::Wheel`]'s fields beyond [`MousePayload`]'s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelPayload {
    /// `clientX`/`clientY`/`button`/`buttons`/the modifier keys.
    pub mouse: MousePayload,
    pub delta_x: f32,
    pub delta_y: f32,
    /// GPUI carries no Z-axis scroll; always 0.
    pub delta_z: f32,
    /// Delta mode: 0 for pixels or 1 for lines. GPUI has no page mode.
    pub delta_mode: u8,
}

/// [`EventPayload::Key`]'s fields.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyPayload {
    /// The key's name, such as `Enter`, `F5` or the typed character.
    pub key: String,
    /// Whether the key press is an auto-repeat.
    pub repeat: bool,
    pub modifiers: gpui::Modifiers,
    /// An input method composes in the focused `input`.
    pub is_composing: bool,
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

/// Maps a button index (the value of `MousePayload::button`) to its
/// `buttons` bitmask value, ordered left/right/middle/back/forward.
/// An index outside `0..=4` maps to 0.
#[must_use]
pub const fn buttons_mask(index: u8) -> u8 {
    match index {
        0 => 0b0_0001, // left
        1 => 0b0_0100, // middle
        2 => 0b0_0010, // right
        3 => 0b0_1000, // back
        4 => 0b1_0000, // forward
        _ => 0,
    }
}

impl From<&MouseDownEvent> for EventPayload {
    fn from(event: &MouseDownEvent) -> Self {
        let bit = button_index(event.button);
        Self::Mouse(MousePayload {
            button: bit,
            buttons: buttons_mask(bit),
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            ..MousePayload::at(event.position, event.modifiers)
        })
    }
}

impl From<&MouseUpEvent> for EventPayload {
    fn from(event: &MouseUpEvent) -> Self {
        let bit = button_index(event.button);
        Self::Mouse(MousePayload {
            button: bit,
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            ..MousePayload::at(event.position, event.modifiers)
        })
    }
}

impl EventPayload {
    /// The mouse fields of a `Mouse` or `Wheel` payload.
    pub fn mouse_mut(&mut self) -> Option<&mut MousePayload> {
        match self {
            Self::Mouse(mouse) => Some(mouse),
            Self::Wheel(wheel) => Some(&mut wheel.mouse),
            _ => None,
        }
    }

    /// The `click` or `auxclick` a release of `event`'s button produces:
    /// `button` is the released button, `detail` the click count and `buttons`
    /// 0, at the release point.
    #[must_use]
    pub fn click(event: &MouseUpEvent) -> Self {
        Self::Mouse(MousePayload {
            button: button_index(event.button),
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            pointer: Some(PointerSource::Mouse),
            ..MousePayload::at(event.position, event.modifiers)
        })
    }

    /// The `contextmenu` a press of `event`'s button produces: `detail` 0 and
    /// `buttons` the pressed button.
    #[must_use]
    pub fn context_menu(event: &MouseDownEvent) -> Self {
        let bit = button_index(event.button);
        Self::Mouse(MousePayload {
            button: bit,
            buttons: buttons_mask(bit),
            pointer: Some(PointerSource::Mouse),
            ..MousePayload::at(event.position, event.modifiers)
        })
    }
}

impl From<&MouseMoveEvent> for EventPayload {
    fn from(event: &MouseMoveEvent) -> Self {
        Self::Mouse(MousePayload {
            buttons: event
                .pressed_button
                .map_or(0, |button| buttons_mask(button_index(button))),
            ..MousePayload::at(event.position, event.modifiers)
        })
    }
}

impl From<&ScrollWheelEvent> for EventPayload {
    fn from(event: &ScrollWheelEvent) -> Self {
        // GPUI reports a downward/rightward scroll as negative; the
        // payload reports it as positive. `0.0 - x` keeps a zero delta at +0.
        let (delta_x, delta_y, delta_mode) = match event.delta {
            ScrollDelta::Pixels(delta) => (0.0 - f32::from(delta.x), 0.0 - f32::from(delta.y), 0),
            ScrollDelta::Lines(delta) => (0.0 - delta.x, 0.0 - delta.y, 1),
        };
        Self::Wheel(WheelPayload {
            mouse: MousePayload::at(event.position, event.modifiers),
            delta_x,
            delta_y,
            delta_z: 0.0,
            delta_mode,
        })
    }
}

impl From<&KeyDownEvent> for EventPayload {
    fn from(event: &KeyDownEvent) -> Self {
        Self::Key(KeyPayload {
            key: key_name(&event.keystroke),
            repeat: event.is_held,
            modifiers: event.keystroke.modifiers,
            is_composing: false,
        })
    }
}

impl From<&KeyUpEvent> for EventPayload {
    fn from(event: &KeyUpEvent) -> Self {
        Self::Key(KeyPayload {
            key: key_name(&event.keystroke),
            // A key release is never a repeat.
            repeat: false,
            modifiers: event.keystroke.modifiers,
            is_composing: false,
        })
    }
}

/// The containers a pointer leaves and enters when its deepest container
/// changes from the end of `old_path` to the end of `new_path`. Each path
/// lists a container and its ancestors, nearest first. The first list holds
/// the containers only `old_path` has, innermost first. The second holds the
/// ones only `new_path` has, outermost first.
#[must_use]
pub fn hover_path_difference(
    old_path: &[NodeId],
    new_path: &[NodeId],
) -> (Vec<NodeId>, Vec<NodeId>) {
    let leave = old_path
        .iter()
        .copied()
        .filter(|id| !new_path.contains(id))
        .collect();
    let enter = new_path
        .iter()
        .rev()
        .copied()
        .filter(|id| !old_path.contains(id))
        .collect();
    (leave, enter)
}

/// Returns `payload` with `screenX`/`screenY` set to the window's position
/// on the screen plus the client position.
#[must_use]
pub fn with_screen(mut payload: EventPayload, window: &Window) -> EventPayload {
    let origin = window.bounds().origin;
    if let Some(mouse) = payload.mouse_mut() {
        mouse.screen_x = f32::from(origin.x) + mouse.client_x;
        mouse.screen_y = f32::from(origin.y) + mouse.client_y;
    }
    payload
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use gpui::AppContext as _;
    use rstest::rstest;

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
        let payload = mouse_payload(EventPayload::click(&event));
        assert_eq!(field_mismatches(&payload, want), Vec::<String>::new());
        assert_eq!(payload.modifiers, shift);
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

    #[rstest]
    #[case::pixels(ScrollDelta::Pixels(gpui::point(gpui::px(0.0), gpui::px(-0.0))))]
    #[case::lines(ScrollDelta::Lines(gpui::point(0.0, -0.0)))]
    fn a_zero_delta_stays_positive_zero(#[case] delta: ScrollDelta) {
        let event = ScrollWheelEvent {
            delta,
            ..Default::default()
        };
        let wheel = wheel_payload(EventPayload::from(&event));
        assert!(wheel.delta_x.is_sign_positive() && wheel.delta_y.is_sign_positive());
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

    #[gpui::test]
    fn screen_coordinates_are_the_window_origin_plus_the_client_position(
        cx: &mut gpui::TestAppContext,
    ) {
        let bounds = gpui::Bounds::new(
            gpui::point(gpui::px(30.0), gpui::px(40.0)),
            gpui::size(gpui::px(100.0), gpui::px(100.0)),
        );
        let window = cx.update(|cx| {
            cx.open_window(
                gpui::WindowOptions {
                    window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(|_| gpui::Empty),
            )
            .unwrap()
        });
        let position = gpui::point(gpui::px(12.0), gpui::px(34.0));
        let payload = EventPayload::Mouse(MousePayload::at(position, gpui::Modifiers::default()));

        let screen = cx
            .update_window(window.into(), |_, window, _| {
                mouse_payload(with_screen(payload, window))
            })
            .unwrap();

        assert_eq!(screen.screen_x, 42.0);
        assert_eq!(screen.screen_y, 74.0);
    }
}
