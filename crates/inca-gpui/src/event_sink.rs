// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::{
    App, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ScrollDelta, ScrollWheelEvent, Window,
};

use crate::tree::NodeId;

/// Defines [`EventKind`], its `ALL` slice and the [`EventMask`] bit constants
/// from one list.
macro_rules! event_kinds {
    ($($variant:ident => $name:literal, $bit:ident),+ $(,)?) => {
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

            /// The bit [`EventMask`] uses for this kind.
            #[must_use]
            pub const fn mask(self) -> EventMask {
                EventMask(1 << self as u16)
            }
        }

        impl EventMask {
            $(
                #[doc = concat!("Wired for [`EventKind::", stringify!($variant), "`].")]
                pub const $bit: Self = EventKind::$variant.mask();
            )+
        }
    };
}

event_kinds! {
    Click => "click", CLICK,
    DblClick => "dblclick", DBL_CLICK,
    AuxClick => "auxclick", AUX_CLICK,
    ContextMenu => "contextmenu", CONTEXT_MENU,
    MouseDown => "mousedown", MOUSE_DOWN,
    MouseUp => "mouseup", MOUSE_UP,
    MouseMove => "mousemove", MOUSE_MOVE,
    Wheel => "wheel", WHEEL,
    MouseEnter => "mouseenter", MOUSE_ENTER,
    MouseLeave => "mouseleave", MOUSE_LEAVE,
    MouseOver => "mouseover", MOUSE_OVER,
    MouseOut => "mouseout", MOUSE_OUT,
    Focus => "focus", FOCUS,
    Blur => "blur", BLUR,
    KeyDown => "keydown", KEY_DOWN,
    KeyUp => "keyup", KEY_UP,
}

/// Which of [`EventKind::ALL`] a node is wired for, as one bit per kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EventMask(u16);

impl EventMask {
    /// Wired for nothing.
    pub const NONE: Self = Self(0);

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

impl std::ops::Not for EventMask {
    type Output = Self;

    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl std::ops::BitOr for EventMask {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

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
}

/// [`EventPayload::Mouse`]'s fields, named and shaped after DOM's
/// `MouseEvent`.
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
    /// The button this event is about. 0 for a move, which isn't about any
    /// one button.
    pub button: u8,
    /// Every button held during this event, as a bitmask (DOM's `buttons`).
    pub buttons: u8,
    /// How many clicks this is part of (DOM's `detail`). 0 for a move.
    pub detail: u32,
    pub modifiers: gpui::Modifiers,
    /// Set for `click`, `auxclick` and `contextmenu`, which carry the pointer fields.
    pub pointer: Option<PointerSource>,
    /// `relatedTarget`: the node the pointer came from for `mouseover`
    /// and `mouseenter`, the node it went to for `mouseout` and `mouseleave`.
    pub related_target: Option<NodeId>,
}

/// What produced a `PointerEvent`-typed event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerSource {
    /// The mouse: `pointerId` 1, `pointerType` `"mouse"`, `isPrimary` true.
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
    /// DOM's `deltaMode`: 0 (`DOM_DELTA_PIXEL`) or 1 (`DOM_DELTA_LINE`).
    /// GPUI has no equivalent of `DOM_DELTA_PAGE` (2).
    pub delta_mode: u8,
}

/// [`EventPayload::Key`]'s fields, named and shaped after DOM's
/// `KeyboardEvent`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyPayload {
    /// DOM's `key`, from [`dom_key`].
    pub key: String,
    /// DOM's `repeat`.
    pub repeat: bool,
    pub modifiers: gpui::Modifiers,
}

/// DOM's `button` numbering for a [`MouseButton`]: the index of that
/// button's bit in `buttons`, not the bit itself.
pub(crate) const fn dom_button_bit(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::Navigate(gpui::NavigationDirection::Back) => 3,
        MouseButton::Navigate(gpui::NavigationDirection::Forward) => 4,
    }
}

/// Maps a [`dom_button_bit`] index (equivalently, `MousePayload::button`) to
/// its DOM `buttons` bitmask value, ordered left/right/middle/back/forward.
/// An index outside `0..=4` maps to 0.
#[must_use]
pub const fn dom_buttons_bit(index: u8) -> u8 {
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
        let bit = dom_button_bit(event.button);
        Self::Mouse(MousePayload {
            client_x: f32::from(event.position.x),
            client_y: f32::from(event.position.y),
            movement_x: 0.0,
            movement_y: 0.0,
            screen_x: 0.0,
            screen_y: 0.0,
            button: bit,
            buttons: dom_buttons_bit(bit),
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            modifiers: event.modifiers,
            pointer: None,
            related_target: None,
        })
    }
}

impl From<&MouseUpEvent> for EventPayload {
    fn from(event: &MouseUpEvent) -> Self {
        let bit = dom_button_bit(event.button);
        Self::Mouse(MousePayload {
            client_x: f32::from(event.position.x),
            client_y: f32::from(event.position.y),
            movement_x: 0.0,
            movement_y: 0.0,
            screen_x: 0.0,
            screen_y: 0.0,
            button: bit,
            buttons: 0,
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            modifiers: event.modifiers,
            pointer: None,
            related_target: None,
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
            button: dom_button_bit(event.button),
            detail: u32::try_from(event.click_count).unwrap_or(u32::MAX),
            pointer: Some(PointerSource::Mouse),
            ..MousePayload::at(event.position, event.modifiers)
        })
    }

    /// The `contextmenu` a press of `event`'s button produces: `detail` 0 and
    /// `buttons` the pressed button.
    #[must_use]
    pub fn context_menu(event: &MouseDownEvent) -> Self {
        let bit = dom_button_bit(event.button);
        Self::Mouse(MousePayload {
            button: bit,
            buttons: dom_buttons_bit(bit),
            pointer: Some(PointerSource::Mouse),
            ..MousePayload::at(event.position, event.modifiers)
        })
    }
}

impl From<&MouseMoveEvent> for EventPayload {
    fn from(event: &MouseMoveEvent) -> Self {
        Self::Mouse(MousePayload {
            client_x: f32::from(event.position.x),
            client_y: f32::from(event.position.y),
            movement_x: 0.0,
            movement_y: 0.0,
            screen_x: 0.0,
            screen_y: 0.0,
            button: 0,
            buttons: event
                .pressed_button
                .map_or(0, |button| dom_buttons_bit(dom_button_bit(button))),
            detail: 0,
            modifiers: event.modifiers,
            pointer: None,
            related_target: None,
        })
    }
}

impl From<&ScrollWheelEvent> for EventPayload {
    fn from(event: &ScrollWheelEvent) -> Self {
        // GPUI reports a downward/rightward scroll as negative; the DOM
        // reports it as positive. `0.0 - x` keeps a zero delta at +0.
        let (delta_x, delta_y, delta_mode) = match event.delta {
            ScrollDelta::Pixels(delta) => (0.0 - f32::from(delta.x), 0.0 - f32::from(delta.y), 0),
            ScrollDelta::Lines(delta) => (0.0 - delta.x, 0.0 - delta.y, 1),
        };
        Self::Wheel(WheelPayload {
            mouse: MousePayload {
                client_x: f32::from(event.position.x),
                client_y: f32::from(event.position.y),
                movement_x: 0.0,
                movement_y: 0.0,
                screen_x: 0.0,
                screen_y: 0.0,
                // Not about any one button; `inca-bridge` fills in the
                // buttons actually held.
                button: 0,
                buttons: 0,
                detail: 0,
                modifiers: event.modifiers,
                pointer: None,
                related_target: None,
            },
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
            key: dom_key(&event.keystroke),
            repeat: event.is_held,
            modifiers: event.keystroke.modifiers,
        })
    }
}

impl From<&KeyUpEvent> for EventPayload {
    fn from(event: &KeyUpEvent) -> Self {
        Self::Key(KeyPayload {
            key: dom_key(&event.keystroke),
            // DOM's `keyup` is never a repeat.
            repeat: false,
            modifiers: event.keystroke.modifiers,
        })
    }
}

/// GPUI's `Keystroke::key` to DOM's `KeyboardEvent.key`, covering GPUI's
/// special key names, grown as real usage needs more of them. Falls
/// through to `key_char` (the character actually typed, when the platform
/// reports one) and then to `key` itself for anything not in the table.
fn dom_key(keystroke: &Keystroke) -> String {
    let named = match keystroke.key.as_str() {
        "space" => Some(" "),
        "tab" => Some("Tab"),
        "enter" => Some("Enter"),
        "backspace" => Some("Backspace"),
        "delete" => Some("Delete"),
        "left" => Some("ArrowLeft"),
        "right" => Some("ArrowRight"),
        "up" => Some("ArrowUp"),
        "down" => Some("ArrowDown"),
        "pageup" => Some("PageUp"),
        "pagedown" => Some("PageDown"),
        "insert" => Some("Insert"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "back" => Some("BrowserBack"),
        "forward" => Some("BrowserForward"),
        "escape" => Some("Escape"),
        _ => None,
    };
    if let Some(named) = named {
        return named.to_string();
    }
    if let Some(function) = keystroke
        .key
        .strip_prefix('f')
        .filter(|n| n.parse::<u8>().is_ok_and(|n| (1..=35).contains(&n)))
    {
        return format!("F{function}");
    }
    if let Some(typed) = keystroke.key_char.as_ref().filter(|c| !c.is_empty()) {
        return typed.clone();
    }
    // A shifted letter with no `key_char` (a shortcut) keeps its case.
    let mut letter = keystroke.key.chars();
    match (letter.next(), letter.next()) {
        (Some(c), None) if c.is_ascii_lowercase() && keystroke.modifiers.shift => {
            c.to_ascii_uppercase().to_string()
        }
        (None, _) => "Unidentified".to_string(),
        _ => keystroke.key.clone(),
    }
}

/// What [`crate::element`] needs from something that can dispatch a native
/// event into JS — `inca-bridge`'s `EventDispatcher` implements this, kept
/// as a trait here rather than a direct dependency so this crate never has
/// to depend on `QuickJS`. An implementor provides every method, including
/// [`EventSink::pointer_moved`] and [`EventSink::pointer_left`].
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

    /// Fires `event` at `start`, with `start` as its target. The event
    /// bubbles to the ancestors of `start` when its kind bubbles. Returns
    /// whether a listener prevented the default action.
    fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool;

    /// The handle to track this node's focus with, if it's focusable.
    fn focus_handle(&self, node_id: NodeId) -> Option<gpui::FocusHandle>;

    /// Tracks this node's scroll offset. A scrolling container that gets one
    /// scrolls for the innermost container under the wheel that can still
    /// move, and stays put after `preventDefault()`.
    fn scroll_handle(&self, node_id: NodeId) -> Option<gpui::ScrollHandle>;

    /// Reports a pointer move to `position`, in window coordinates. Called
    /// for every pointer move in the window, buttons held included, before
    /// any node's listener runs. `cx` lets the sink schedule work for the end
    /// of the current update.
    fn pointer_moved(&self, position: gpui::Point<gpui::Pixels>, cx: &mut App);

    /// Reports that the pointer left the window. The next
    /// [`EventSink::pointer_moved`] starts a new measurement.
    fn pointer_left(&self);

    /// Reports the window's modifier keys after a change. The sink fires
    /// `keydown` for each modifier pressed and `keyup` for each released, at
    /// the focused node or the root.
    fn modifiers_changed(&self, modifiers: gpui::Modifiers, window: &mut Window, cx: &mut App);

    /// Reports that `node_id` became hovered or stopped being hovered, as
    /// `gpui` decides it. The sink fires `mouseover`, `mouseout`,
    /// `mouseenter` and `mouseleave` for the net change once the current
    /// update ends.
    fn hover_changed(&self, node_id: NodeId, hovered: bool, window: &mut Window, cx: &mut App);

    /// Reports `node_id` as the deepest container under the pointer for the
    /// pointer move being handled, before any listener of that move runs. The
    /// sink fires the hover events for it at once.
    fn pointer_over(&self, node_id: NodeId, window: &mut Window, cx: &mut App);

    /// Reports that a mouse button was pressed. Ends a pending Space press.
    fn pointer_pressed(&self);

    /// Reports a mouse button release once every listener of the release has
    /// run. `pressed` is the deepest container under the press of the same
    /// button, and `None` when no such press is recorded. `released` is the
    /// deepest container under the release, or the root when none is. The
    /// sink fires `click` for the left button, `auxclick` for the others and
    /// `dblclick` after the second `click`, at the nearest common ancestor of
    /// the two containers. With `pressed` `None`, nothing fires.
    fn pointer_released(
        &self,
        event: &MouseUpEvent,
        pressed: Option<NodeId>,
        released: NodeId,
        window: &mut Window,
        cx: &mut App,
    );

    /// Runs the default action of a key press (`up` false) or release once the
    /// key's dispatch has ended, unless a listener called `preventDefault()`.
    /// Tab and Shift+Tab move focus; Control, Alt or Meta cancel the move. On
    /// a focused enabled button, Enter clicks it on press and Space clicks it
    /// on release. A pending Space press ends at the next key press or mouse
    /// press.
    fn key_default(&self, keystroke: &gpui::Keystroke, up: bool, window: &mut Window, cx: &mut App);
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

    #[test]
    fn dom_key_prefers_a_named_key_over_its_key_char() {
        let enter = Keystroke {
            key: "enter".to_string(),
            key_char: Some("\n".to_string()),
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(dom_key(&enter), "Enter");
    }

    #[test]
    fn dom_key_names_every_function_key() {
        for n in 1..=35u8 {
            let keystroke = Keystroke {
                key: format!("f{n}"),
                key_char: None,
                modifiers: gpui::Modifiers::default(),
            };
            assert_eq!(dom_key(&keystroke), format!("F{n}"));
        }
    }

    #[test]
    fn dom_key_falls_back_to_key_char_for_an_unnamed_key() {
        let shifted_s = Keystroke {
            key: "s".to_string(),
            key_char: Some("S".to_string()),
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(dom_key(&shifted_s), "S");
    }

    #[test]
    fn dom_key_falls_back_to_key_when_key_char_is_none() {
        let cmd_s = Keystroke {
            key: "s".to_string(),
            key_char: None,
            modifiers: gpui::Modifiers::default(),
        };
        assert_eq!(dom_key(&cmd_s), "s");
    }

    #[test]
    fn dom_key_covers_the_unidentified_and_shifted_fallbacks() {
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
                let got = dom_key(&keystroke);
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
    fn a_pixel_delta_reports_dom_delta_pixel() {
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
    fn a_line_delta_reports_dom_delta_line() {
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
    fn scrolling_down_or_right_reports_positive_dom_deltas() {
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
}
