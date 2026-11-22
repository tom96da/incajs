// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The data an event carries beyond its target and name.

use inca_node::tree::NodeId;

use crate::text::InputPayload;

/// The modifier keys held during an event, as a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const CONTROL: Self = Self(1);
    pub const ALT: Self = Self(2);
    pub const SHIFT: Self = Self(4);
    /// The Command key on macOS and the Super key elsewhere: `metaKey`.
    pub const PLATFORM: Self = Self(8);

    /// No modifier held.
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }

    /// This set with `key` held when `held` is true and released otherwise.
    #[must_use]
    pub const fn set(self, key: Self, held: bool) -> Self {
        if held {
            Self(self.0 | key.0)
        } else {
            Self(self.0 & !key.0)
        }
    }

    #[must_use]
    pub const fn control(self) -> bool {
        self.0 & Self::CONTROL.0 != 0
    }

    #[must_use]
    pub const fn alt(self) -> bool {
        self.0 & Self::ALT.0 != 0
    }

    #[must_use]
    pub const fn shift(self) -> bool {
        self.0 & Self::SHIFT.0 != 0
    }

    #[must_use]
    pub const fn platform(self) -> bool {
        self.0 & Self::PLATFORM.0 != 0
    }
}

/// What a dispatch carries beyond the node it fired on.
#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    /// No data beyond the node and the event's name.
    None,
    /// A mouse position, button, and modifier state.
    Mouse(MousePayload),
    /// A scroll delta plus the same fields as [`EventPayload::Mouse`].
    Wheel(WheelPayload),
    /// A key and modifier state.
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
    /// (`movementX`/`movementY`). 0 until the source of the pointer
    /// event sets it.
    pub movement_x: f32,
    pub movement_y: f32,
    /// The window's position on the screen plus the client position
    /// (`screenX`/`screenY`). 0 until the source of the pointer event
    /// sets it.
    pub screen_x: f32,
    pub screen_y: f32,
    /// The position relative to the padding edge of the event's `target`
    /// (`offsetX`/`offsetY`). 0 until the source of the pointer event
    /// sets it.
    pub offset_x: f32,
    pub offset_y: f32,
    /// The button this event is about. 0 for a mouse move, which isn't about any
    /// one button. [`NO_BUTTON`] for a pointer event about no button.
    pub button: u8,
    /// Every button held during this event, as a bitmask.
    pub buttons: u8,
    /// How many clicks this is part of. 0 for a move.
    pub detail: u32,
    pub modifiers: Modifiers,
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
    /// A payload at (`client_x`, `client_y`) with the given `modifiers`. The movement fields are 0
    /// and `related_target` is `None`.
    #[must_use]
    pub fn at(client_x: f32, client_y: f32, modifiers: Modifiers) -> Self {
        Self {
            client_x,
            client_y,
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
    pub fn keyboard_click(modifiers: Modifiers) -> Self {
        Self {
            pointer: Some(PointerSource::Keyboard),
            ..Self::at(0.0, 0.0, modifiers)
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
    /// Always 0.
    pub delta_z: f32,
    /// Delta mode: 0 for pixels or 1 for lines.
    pub delta_mode: u8,
}

/// [`EventPayload::Key`]'s fields.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyPayload {
    /// The key's name, such as `Enter`, `F5` or the typed character.
    pub key: String,
    /// Whether the key press is an auto-repeat.
    pub repeat: bool,
    pub modifiers: Modifiers,
    /// An input method composes in the focused `input`.
    pub is_composing: bool,
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

impl EventPayload {
    /// The mouse fields of a `Mouse` or `Wheel` payload.
    pub fn mouse_mut(&mut self) -> Option<&mut MousePayload> {
        match self {
            Self::Mouse(mouse) => Some(mouse),
            Self::Wheel(wheel) => Some(&mut wheel.mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use rstest::rstest;

    #[test]
    fn a_modifier_set_holds_each_key_apart() {
        let held = Modifiers::none()
            .set(Modifiers::ALT, true)
            .set(Modifiers::PLATFORM, true);
        assert_eq!(
            (held.control(), held.alt(), held.shift(), held.platform()),
            (false, true, false, true)
        );
        let released = held.set(Modifiers::ALT, false);
        assert_eq!(
            (released.alt(), released.platform()),
            (false, true),
            "releasing one key keeps the others"
        );
    }

    #[test]
    fn a_mouse_payload_at_a_point_holds_the_position_and_the_modifiers() {
        let mouse = MousePayload::at(12.0, 34.0, Modifiers::SHIFT);
        assert_eq!((mouse.client_x, mouse.client_y), (12.0, 34.0));
        assert_eq!((mouse.button, mouse.buttons, mouse.detail), (0, 0, 0));
        assert_eq!(mouse.modifiers, Modifiers::SHIFT);
        assert_eq!((mouse.pointer, mouse.related_target), (None, None));
    }

    #[test]
    fn a_keyboard_click_comes_from_the_keyboard_at_the_origin() {
        let mouse = MousePayload::keyboard_click(Modifiers::CONTROL);
        assert_eq!(mouse.pointer, Some(PointerSource::Keyboard));
        assert_eq!((mouse.client_x, mouse.client_y), (0.0, 0.0));
        assert_eq!(mouse.modifiers, Modifiers::CONTROL);
    }

    #[test]
    fn the_mouse_fields_of_a_wheel_payload_are_reachable() {
        let mut wheel = EventPayload::Wheel(WheelPayload {
            mouse: MousePayload::at(1.0, 2.0, Modifiers::none()),
            delta_x: 0.0,
            delta_y: 3.0,
            delta_z: 0.0,
            delta_mode: 0,
        });
        wheel.mouse_mut().unwrap_or_else(|| unreachable!()).offset_x = 9.0;
        let EventPayload::Wheel(wheel) = wheel else {
            unreachable!()
        };
        assert_eq!(wheel.mouse.offset_x, 9.0);
        assert!(EventPayload::None.mouse_mut().is_none());
    }

    #[rstest]
    #[case::left(0, 0b0_0001)]
    #[case::middle(1, 0b0_0100)]
    #[case::right(2, 0b0_0010)]
    #[case::back(3, 0b0_1000)]
    #[case::forward(4, 0b1_0000)]
    #[case::outside(5, 0)]
    fn a_button_index_maps_to_its_bit(#[case] index: u8, #[case] want: u8) {
        assert_eq!(buttons_mask(index), want);
    }
}
