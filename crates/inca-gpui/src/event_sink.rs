// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::{
    App, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent,
    Window,
};

use crate::tree::NodeId;

/// Defines [`EventKind`] and its `ALL` slice from one variant/name list, so
/// a variant can't be added to the enum without also landing in `ALL`.
macro_rules! event_kinds {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// A native event kind `inca-gpui` can wire a node for.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum EventKind {
            $($variant),+
        }

        impl EventKind {
            /// Every kind, in no particular order.
            pub const ALL: &[Self] = &[$(Self::$variant),+];

            /// The name `addEventListener`/`EventSink::dispatch` use for this kind.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }
        }
    };
}

event_kinds! {
    Click => "click",
    MouseDown => "mousedown",
    MouseUp => "mouseup",
    MouseMove => "mousemove",
    Wheel => "wheel",
}

impl EventKind {
    /// The bit [`EventMask`] uses for this kind.
    #[must_use]
    pub const fn mask(self) -> EventMask {
        match self {
            Self::Click => EventMask::CLICK,
            Self::MouseDown => EventMask::MOUSE_DOWN,
            Self::MouseUp => EventMask::MOUSE_UP,
            Self::MouseMove => EventMask::MOUSE_MOVE,
            Self::Wheel => EventMask::WHEEL,
        }
    }

    /// GPUI's own dispatch requires a `gpui` `ElementId` (`.id()`) to keep
    /// state for this kind across frames.
    #[must_use]
    pub const fn needs_element_id(self) -> bool {
        match self {
            Self::Click => true,
            Self::MouseDown | Self::MouseUp | Self::MouseMove | Self::Wheel => false,
        }
    }
}

/// Which of [`EventKind::ALL`] a node is wired for, as one bit per kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventMask(u16);

impl EventMask {
    /// Wired for nothing.
    pub const NONE: Self = Self(0);
    /// Wired for [`EventKind::Click`].
    pub const CLICK: Self = Self(1 << 0);
    /// Wired for [`EventKind::MouseDown`].
    pub const MOUSE_DOWN: Self = Self(1 << 1);
    /// Wired for [`EventKind::MouseUp`].
    pub const MOUSE_UP: Self = Self(1 << 2);
    /// Wired for [`EventKind::MouseMove`].
    pub const MOUSE_MOVE: Self = Self(1 << 3);
    /// Wired for [`EventKind::Wheel`].
    pub const WHEEL: Self = Self(1 << 4);

    /// Whether every bit set in `other` is also set in `self`.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether `self` and `other` share at least one bit.
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Every [`EventKind::needs_element_id`] kind's bit, combined.
    #[must_use]
    pub fn needing_element_id() -> Self {
        EventKind::ALL
            .iter()
            .filter(|kind| kind.needs_element_id())
            .fold(Self::NONE, |mask, kind| mask | kind.mask())
    }

    /// Whether `self` includes a kind that requires a `gpui` `ElementId`.
    #[must_use]
    pub fn needs_element_id(self) -> bool {
        self.intersects(Self::needing_element_id())
    }

    /// The bit `name` occupies, or [`EventMask::NONE`] if `name` names no
    /// [`EventKind`].
    #[must_use]
    pub fn for_name(name: &str) -> Self {
        EventKind::ALL
            .iter()
            .find(|kind| kind.name() == name)
            .map_or(Self::NONE, |kind| kind.mask())
    }
}

impl std::ops::BitOr for EventMask {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// What a dispatch carries beyond the node it fired on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventPayload {
    /// No data beyond the node and the event's name.
    None,
    /// A mouse position, button, and modifier state, DOM-`MouseEvent`-shaped.
    Mouse(MousePayload),
    /// A scroll delta plus the same fields as [`EventPayload::Mouse`],
    /// DOM-`WheelEvent`-shaped (`WheelEvent` extends `MouseEvent`).
    Wheel(WheelPayload),
}

/// [`EventPayload::Mouse`]'s fields, named and shaped after DOM's
/// `MouseEvent`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MousePayload {
    pub client_x: f32,
    pub client_y: f32,
    /// The button this event is about. 0 for a move, which isn't about any
    /// one button.
    pub button: u8,
    /// Every button held during this event, as a bitmask (DOM's `buttons`).
    pub buttons: u8,
    /// How many clicks this is part of (DOM's `detail`). 0 for a move.
    pub detail: u32,
    pub modifiers: gpui::Modifiers,
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
    /// DOM's `deltaMode`: 0 (`DOM_DELTA_PIXEL`) or 1 (`DOM_DELTA_LINE`).
    /// GPUI has no equivalent of `DOM_DELTA_PAGE` (2).
    pub delta_mode: u8,
}

/// DOM's `button`/`buttons` numbering for a [`MouseButton`]. `buttons` is a
/// bit; `button` is that bit's index.
const fn dom_button_bit(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::Navigate(gpui::NavigationDirection::Back) => 3,
        MouseButton::Navigate(gpui::NavigationDirection::Forward) => 4,
    }
}

impl From<&MouseDownEvent> for EventPayload {
    fn from(event: &MouseDownEvent) -> Self {
        let bit = dom_button_bit(event.button);
        Self::Mouse(MousePayload {
            client_x: f32::from(event.position.x),
            client_y: f32::from(event.position.y),
            button: bit,
            buttons: 1 << bit,
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            modifiers: event.modifiers,
        })
    }
}

impl From<&MouseUpEvent> for EventPayload {
    fn from(event: &MouseUpEvent) -> Self {
        let bit = dom_button_bit(event.button);
        Self::Mouse(MousePayload {
            client_x: f32::from(event.position.x),
            client_y: f32::from(event.position.y),
            button: bit,
            buttons: 0,
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            modifiers: event.modifiers,
        })
    }
}

impl From<&MouseMoveEvent> for EventPayload {
    fn from(event: &MouseMoveEvent) -> Self {
        Self::Mouse(MousePayload {
            client_x: f32::from(event.position.x),
            client_y: f32::from(event.position.y),
            button: 0,
            buttons: event
                .pressed_button
                .map_or(0, |button| 1 << dom_button_bit(button)),
            detail: 0,
            modifiers: event.modifiers,
        })
    }
}

impl From<&ScrollWheelEvent> for EventPayload {
    fn from(event: &ScrollWheelEvent) -> Self {
        let (delta_x, delta_y, delta_mode) = match event.delta {
            ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y), 0),
            ScrollDelta::Lines(delta) => (delta.x, delta.y, 1),
        };
        Self::Wheel(WheelPayload {
            mouse: MousePayload {
                client_x: f32::from(event.position.x),
                client_y: f32::from(event.position.y),
                // Not about any one button; `inca-bridge` fills in the
                // buttons actually held.
                button: 0,
                buttons: 0,
                detail: 0,
                modifiers: event.modifiers,
            },
            delta_x,
            delta_y,
            delta_z: 0.0,
            delta_mode,
        })
    }
}

/// What [`crate::element`] needs from something that can dispatch a native
/// event into JS — `inca-bridge`'s `EventDispatcher` implements this, kept
/// as a trait here rather than a direct dependency so this crate never has
/// to depend on `QuickJS`.
pub trait EventSink {
    /// Every kind registered on `node_id`. The render path asks before
    /// wiring an element for input.
    fn listens(&self, node_id: NodeId) -> EventMask;

    /// Calls whatever is registered for `(node_id, event)`, passing
    /// `payload`. `cx` lets a callback's `stopPropagation`/`preventDefault`
    /// reach GPUI's own dispatch.
    fn dispatch(
        &self,
        node_id: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    );
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn none_contains_only_none() {
        assert!(EventMask::NONE.contains(EventMask::NONE));
        assert!(!EventMask::NONE.contains(EventMask::CLICK));
    }

    #[test]
    fn a_mask_contains_itself_and_none() {
        assert!(EventMask::CLICK.contains(EventMask::CLICK));
        assert!(EventMask::CLICK.contains(EventMask::NONE));
    }

    #[test]
    fn union_contains_both_operands() {
        let union = EventMask::CLICK | EventMask::NONE;
        assert!(union.contains(EventMask::CLICK));
    }

    /// Every kind carries its own bit; no two share one.
    #[test]
    fn every_kind_has_its_own_bit() {
        let mut seen = EventMask::NONE;
        for kind in EventKind::ALL {
            let bit = kind.mask();
            assert_ne!(bit, EventMask::NONE, "{kind:?} has no bit");
            assert!(
                !seen.contains(bit),
                "{kind:?}'s bit collides with an earlier kind's"
            );
            seen = seen | bit;
        }
    }

    #[test]
    fn for_name_matches_every_kind() {
        for kind in EventKind::ALL {
            assert_eq!(EventMask::for_name(kind.name()), kind.mask());
        }
        assert_eq!(EventMask::for_name("not-a-real-event"), EventMask::NONE);
    }

    #[test]
    fn no_two_kinds_share_a_name() {
        let mut names = std::collections::HashSet::new();
        for kind in EventKind::ALL {
            assert!(
                names.insert(kind.name()),
                "{kind:?} repeats an earlier name"
            );
        }
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
        assert_eq!(mouse.buttons, 0b001);
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
        assert_eq!(mouse.buttons, 0b010);
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

    #[test]
    fn a_pixel_delta_reports_dom_delta_pixel() {
        let event = ScrollWheelEvent {
            delta: ScrollDelta::Pixels(gpui::point(gpui::px(3.0), gpui::px(-4.0))),
            ..Default::default()
        };
        let wheel = wheel_payload(EventPayload::from(&event));
        assert_eq!(wheel.delta_x, 3.0);
        assert_eq!(wheel.delta_y, -4.0);
        assert_eq!(wheel.delta_z, 0.0);
        assert_eq!(wheel.delta_mode, 0);
    }

    #[test]
    fn a_line_delta_reports_dom_delta_line() {
        let event = ScrollWheelEvent {
            delta: ScrollDelta::Lines(gpui::point(0.0, 2.0)),
            ..Default::default()
        };
        let wheel = wheel_payload(EventPayload::from(&event));
        assert_eq!(wheel.delta_x, 0.0);
        assert_eq!(wheel.delta_y, 2.0);
        assert_eq!(wheel.delta_mode, 1);
    }

    #[test]
    fn a_negative_line_delta_carries_its_sign() {
        let event = ScrollWheelEvent {
            delta: ScrollDelta::Lines(gpui::point(-1.5, 0.0)),
            ..Default::default()
        };
        let wheel = wheel_payload(EventPayload::from(&event));
        assert_eq!(wheel.delta_x, -1.5);
        assert_eq!(wheel.delta_mode, 1);
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
    fn only_click_needs_an_element_id_today() {
        assert_eq!(EventMask::needing_element_id(), EventMask::CLICK);
        assert!(EventMask::CLICK.needs_element_id());
        assert!(!EventMask::MOUSE_DOWN.needs_element_id());
        assert!(!EventMask::MOUSE_UP.needs_element_id());
        assert!(!EventMask::MOUSE_MOVE.needs_element_id());
        assert!(!EventMask::WHEEL.needs_element_id());
    }

    /// A mask still needs an id if `.needs_element_id()`-requiring kind is
    /// only one bit among several — union with an unrelated kind can't
    /// hide it.
    #[test]
    fn needing_an_element_id_survives_a_union_with_other_kinds() {
        let mixed = EventMask::CLICK | EventMask::MOUSE_MOVE;
        assert!(mixed.needs_element_id());
    }

    #[test]
    fn disjoint_masks_do_not_intersect() {
        assert!(!EventMask::MOUSE_DOWN.intersects(EventMask::MOUSE_UP));
    }

    #[test]
    fn a_mask_intersects_a_union_it_is_part_of() {
        let union = EventMask::CLICK | EventMask::WHEEL;
        assert!(union.intersects(EventMask::CLICK));
        assert!(union.intersects(EventMask::WHEEL));
    }

    #[test]
    fn none_intersects_nothing() {
        assert!(!EventMask::NONE.intersects(EventMask::NONE));
        assert!(!EventMask::NONE.intersects(EventMask::CLICK));
    }
}
