// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::{
    App, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ScrollDelta, ScrollWheelEvent, Window,
};

use crate::element::NodeLayout;
use crate::input::{InputLayout, InputPayload, InputRequest, InputState, InputType};
use crate::tree::NodeId;

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

/// Whether gpui's `key` names the Menu key: `"menu"`, or U+0010 from a Mac keyboard.
#[must_use]
pub fn is_menu_key(key: &str) -> bool {
    matches!(key, "menu" | "\u{10}")
}

/// GPUI's `Keystroke::key` to the key name, covering GPUI's
/// special key names, grown as real usage needs more of them. Falls
/// through to `key_char` (the character actually typed, when the platform
/// reports one) and then to `key` itself for anything not in the table.
fn key_name(keystroke: &Keystroke) -> String {
    if is_menu_key(&keystroke.key) {
        return "ContextMenu".to_string();
    }
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
/// event into JS. The trait keeps this crate free of a `QuickJS` dependency.
/// An implementor provides every method.
pub trait EventSink {
    /// Whether the containers report the pointer. The render path asks once
    /// per frame. With `true`, every container that is not inert becomes a
    /// pointer target, which gives it a hitbox and a hover state.
    fn tracks_pointer(&self) -> bool;

    /// Fires `event` at `start`, with `start` as its target. The event runs a
    /// capture pass over the ancestors of `start`, then a bubble pass that
    /// reaches the ancestors when its kind bubbles. `preventDefault()` calls
    /// from passive listeners are ignored. Returns whether a listener
    /// prevented the default action.
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

    /// Tracks this node's scroll offset. A scroll container (`hidden` or
    /// `scroll` on either axis) gets one. After the `wheel` listeners ran, the
    /// wheel step goes to the innermost container under the pointer that can
    /// move, and is not applied after `preventDefault()`.
    fn scroll_handle(&self, node_id: NodeId) -> Option<gpui::ScrollHandle>;

    /// The axes along which a wheel step can scroll `node_id` now, read from
    /// the live tree (see [`crate::wheel_axes`]). `None` when the node is unknown,
    /// detached, hidden or inert. A shown container without `scroll` overflow
    /// gives `false` on both axes.
    fn wheel_axes(&self, node_id: NodeId) -> Option<gpui::Point<bool>>;

    /// Forgets every recorded layout. Called before each frame's tree is
    /// built, so only nodes laid out in the new frame have a record.
    fn clear_layout(&self);

    /// Records the box `node_id` was laid out in. Called during prepaint for
    /// each displayed child of a container, as the frame is drawn.
    fn record_layout(&self, node_id: NodeId, layout: NodeLayout);

    /// Reports a pointer move to `position`, in window coordinates. Called
    /// for every pointer move in the window, buttons held included, before
    /// any container reports. `cx` lets the sink schedule work for the end
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
    /// `gpui` decides it. The sink fires the pointer and mouse `over`, `out`,
    /// `enter` and `leave` events for the net change once the current update
    /// ends.
    fn hover_changed(&self, node_id: NodeId, hovered: bool, window: &mut Window, cx: &mut App);

    /// Reports `node_id` as the deepest container under the pointer for the
    /// pointer move being handled, before the move itself is reported. The
    /// sink fires the hover events for it at once.
    fn pointer_over(&self, node_id: NodeId, window: &mut Window, cx: &mut App);

    /// Reports one `mousedown`, `mouseup`, `mousemove` or `wheel` (`event`)
    /// once every container has reported itself. `target` is the deepest
    /// container under the pointer, or the root when none is. The sink
    /// dispatches the event along the path of `target` and runs its default
    /// action. A `mousedown`, `mouseup` or `mousemove` is preceded by the
    /// pointer event of the input. Both go to the node holding a pointer
    /// capture, else to `target`. A `mousedown` focuses the nearest
    /// focusable node on the path and leaves the focus alone when a listener
    /// prevented the default or the `pointerdown` was canceled.
    fn pointer_input(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    );

    /// Reports a mouse button release right after its `mouseup`.
    /// `event.click_count` is the click count of the press of the same
    /// button, and the release's own count when `pressed` is `None`.
    /// `pressed` is the deepest container under the press of the same
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

    /// Reports a key press (`up` false) or release before any other key
    /// listener of the window runs. The sink dispatches `keydown` or `keyup`
    /// to the focused node, or to the root when none is focused, then runs
    /// the default action unless a listener prevented it. Tab and Shift+Tab
    /// move focus; Control, Alt or Meta cancel the move. On a focused enabled
    /// button, Enter clicks it on press and Space clicks it on release. A
    /// pending Space press ends at an Enter click, a Space key down, a Space
    /// key up or when the button loses focus. While a mouse press on the
    /// focused button is held, Space up also clicks it, once. A click, the
    /// mouse release or the button losing focus ends the hold. When a
    /// listener prevented the default, the sink calls `Window::prevent_default`
    /// and `App::stop_propagation`. At a focused `input` the default action
    /// edits it, and an edit key counts as handled.
    fn key_input(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    );

    /// The text state of the `input` node `node_id`, `None` for any other
    /// node.
    fn input(&self, node_id: NodeId) -> Option<InputState>;

    /// Stores where the frame being drawn put the text of `node_id`.
    fn record_input(&self, node_id: NodeId, layout: InputLayout);

    /// Carries a call of the platform's text input for the focused `input`
    /// `node_id`: an insertion, a composition, the end of a composition or a
    /// selection. A `disabled` input applies a selection only. A `readonly` one
    /// also reports typed text as `beforeinput` while no composition runs.
    fn input_request(
        &self,
        node_id: NodeId,
        request: InputRequest,
        window: &mut Window,
        cx: &mut App,
    );
}

/// Emits a body for each listed [`EventSink`] method, with the trait's
/// signature: empty, or `None` for `input`. A method missing from the trait
/// is a compile error.
macro_rules! noop_sink_methods {
    ($($name:ident),* $(,)?) => { $(noop_sink_methods!(@one $name);)* };
    (@one clear_layout) => { fn clear_layout(&self) {} };
    (@one record_layout) => {
        fn record_layout(&self, _: $crate::tree::NodeId, _: $crate::element::NodeLayout) {}
    };
    (@one pointer_moved) => {
        fn pointer_moved(&self, _: gpui::Point<gpui::Pixels>, _: &mut gpui::App) {}
    };
    (@one pointer_left) => { fn pointer_left(&self) {} };
    (@one modifiers_changed) => {
        fn modifiers_changed(&self, _: gpui::Modifiers, _: &mut gpui::Window, _: &mut gpui::App) {}
    };
    (@one hover_changed) => {
        fn hover_changed(
            &self,
            _: $crate::tree::NodeId,
            _: bool,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one pointer_over) => {
        fn pointer_over(&self, _: $crate::tree::NodeId, _: &mut gpui::Window, _: &mut gpui::App) {}
    };
    (@one pointer_input) => {
        fn pointer_input(
            &self,
            _: &str,
            _: $crate::tree::NodeId,
            _: &$crate::event_sink::EventPayload,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one pointer_released) => {
        fn pointer_released(
            &self,
            _: &gpui::MouseUpEvent,
            _: Option<$crate::tree::NodeId>,
            _: $crate::tree::NodeId,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one key_input) => {
        fn key_input(
            &self,
            _: &gpui::Keystroke,
            _: bool,
            _: &$crate::event_sink::EventPayload,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
    (@one input) => {
        fn input(&self, _: $crate::tree::NodeId) -> Option<$crate::input::InputState> {
            None
        }
    };
    (@one record_input) => {
        fn record_input(&self, _: $crate::tree::NodeId, _: $crate::input::InputLayout) {}
    };
    (@one input_request) => {
        fn input_request(
            &self,
            _: $crate::tree::NodeId,
            _: $crate::input::InputRequest,
            _: &mut gpui::Window,
            _: &mut gpui::App,
        ) {
        }
    };
}
pub(crate) use noop_sink_methods;

/// `bubbles`, `cancelable` and `composed` of one event.
#[derive(Clone, Copy)]
pub struct Flags {
    pub bubbles: bool,
    pub cancelable: bool,
    pub composed: bool,
}

impl Flags {
    pub const ALL: Self = Self {
        bubbles: true,
        cancelable: true,
        composed: true,
    };
    pub const NONE: Self = Self {
        bubbles: false,
        cancelable: false,
        composed: false,
    };
    /// Focus and capture events: composed and not cancelable.
    pub const FOCUS: Self = Self {
        composed: true,
        ..Self::NONE
    };
}

/// The rules of one event name.
#[derive(Clone, Copy)]
pub struct EventSpec {
    /// The event name.
    pub name: &'static str,
    pub flags: Flags,
    /// Takes `movementX`/`movementY` from the raw pointer move.
    pub movement: bool,
    /// Carries the `buttons` of its own press or release.
    pub own_buttons: bool,
}

const fn spec(name: &'static str, flags: Flags) -> EventSpec {
    EventSpec {
        name,
        flags,
        movement: false,
        own_buttons: false,
    }
}

const fn own_buttons(mut event: EventSpec) -> EventSpec {
    event.own_buttons = true;
    event
}

/// Every event the host dispatches.
pub const EVENTS: [EventSpec; 34] = [
    own_buttons(spec("click", Flags::ALL)),
    own_buttons(spec("dblclick", Flags::ALL)),
    own_buttons(spec("auxclick", Flags::ALL)),
    own_buttons(spec("contextmenu", Flags::ALL)),
    spec("mousedown", Flags::ALL),
    spec("mouseup", Flags::ALL),
    EventSpec {
        movement: true,
        ..spec("mousemove", Flags::ALL)
    },
    spec("mouseenter", Flags::NONE),
    spec("mouseleave", Flags::NONE),
    spec("mouseover", Flags::ALL),
    spec("mouseout", Flags::ALL),
    spec("pointerdown", Flags::ALL),
    spec("pointerup", Flags::ALL),
    EventSpec {
        movement: true,
        ..spec("pointermove", Flags::ALL)
    },
    spec("pointerover", Flags::ALL),
    spec("pointerout", Flags::ALL),
    spec("pointerenter", Flags::NONE),
    spec("pointerleave", Flags::NONE),
    spec(
        "gotpointercapture",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec(
        "lostpointercapture",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec("wheel", Flags::ALL),
    spec("scroll", Flags::NONE),
    spec("keydown", Flags::ALL),
    spec("keyup", Flags::ALL),
    spec("focus", Flags::FOCUS),
    spec("blur", Flags::FOCUS),
    spec(
        "focusin",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec(
        "focusout",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec("beforeinput", Flags::ALL),
    spec(
        "input",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec(
        "change",
        Flags {
            bubbles: true,
            ..Flags::NONE
        },
    ),
    spec("compositionstart", Flags::ALL),
    spec("compositionupdate", Flags::ALL),
    spec("compositionend", Flags::ALL),
];

/// The rules of `event`. Any other name takes all three flags as false.
#[must_use]
pub fn spec_of(event: &str) -> EventSpec {
    EVENTS
        .iter()
        .find(|spec| spec.name == event)
        .copied()
        .unwrap_or(spec("", Flags::NONE))
}

/// The flags of `event` carrying `payload`: those of [`spec_of`], with a
/// `beforeinput` of `insertCompositionText` not cancelable.
#[must_use]
pub fn flags_of(event: &str, payload: &EventPayload) -> Flags {
    let mut flags = spec_of(event).flags;
    if event == "beforeinput"
        && matches!(
            payload,
            EventPayload::Input(InputPayload {
                input_type: InputType::InsertCompositionText,
                ..
            })
        )
    {
        flags.cancelable = false;
    }
    flags
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
mod tests;
