// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::Modifiers;
use inca_gpui::{EventPayload, MousePayload, PointerSource, buttons_mask};
use rquickjs::convert::Coerced;
use rquickjs::function::This;
use rquickjs::{Ctx, Function, IntoJs, Object, Result as JsResult, Value};

use super::key_code::key_code;
use super::{
    EVENT, FOCUS, KEYBOARD, MOUSE, POINTER, WHEEL, child_of, constants, field, method, other_kind,
    with_event,
};
use crate::bindings::convert::or_null;

/// The payload fields of an event, as its getters read them.
pub(super) enum Payload {
    /// An event built with `new Event` or `new CustomEvent`, a scroll event and any event the
    /// host dispatches without data.
    None,
    /// An event built with `new UIEvent`.
    Ui(Ui),
    Focus(Ui),
    Mouse(Mouse),
    /// A mouse event with a pointer source: `click`, `auxclick` and `contextmenu`.
    Pointer(Mouse, Pointer),
    Wheel(Mouse, Wheel),
    Keyboard(Ui, Keyboard),
}

/// The fields every UI event carries.
#[derive(Clone, Copy, Default)]
pub(super) struct Ui {
    pub(super) detail: i32,
    pub(super) which: u32,
}

/// The fields of a mouse event.
#[derive(Clone, Copy)]
pub(super) struct Mouse {
    pub(super) ui: Ui,
    pub(super) screen_x: f64,
    pub(super) screen_y: f64,
    pub(super) client_x: f64,
    pub(super) client_y: f64,
    pub(super) offset_x: f64,
    pub(super) offset_y: f64,
    pub(super) movement_x: f64,
    pub(super) movement_y: f64,
    /// `-1` reads as 0.
    pub(super) button: i16,
    pub(super) buttons: u16,
    pub(super) modifiers: Modifiers,
}

/// The fields of a pointer event beyond the mouse ones.
pub(super) struct Pointer {
    pub(super) id: i32,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) pressure: f32,
    /// `pointerType`.
    pub(super) kind: String,
    pub(super) is_primary: bool,
}

/// The fields of a wheel event beyond the mouse ones.
pub(super) struct Wheel {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) z: f64,
    pub(super) mode: u32,
}

/// The fields of a key event.
pub(super) struct Keyboard {
    pub(super) key: String,
    pub(super) code: String,
    pub(super) location: u32,
    pub(super) repeat: bool,
    pub(super) is_composing: bool,
    pub(super) modifiers: Modifiers,
    pub(super) key_code: u32,
    pub(super) char_code: u32,
}

/// The mouse events about one button, whose `which` is that button + 1. A `wheel` reads 0 and
/// every other mouse event reads its lowest held button.
const BUTTON_EVENTS: [&str; 6] = [
    "mousedown",
    "mouseup",
    "click",
    "dblclick",
    "auxclick",
    "contextmenu",
];

/// The `deltaMode` constants on `WheelEvent` and its prototype.
pub(super) const DELTA_MODES: [(&str, i32); 3] = [
    ("DOM_DELTA_PIXEL", 0),
    ("DOM_DELTA_LINE", 1),
    ("DOM_DELTA_PAGE", 2),
];

/// The `location` constants on `KeyboardEvent` and its prototype.
pub(super) const KEY_LOCATIONS: [(&str, i32); 4] = [
    ("DOM_KEY_LOCATION_STANDARD", 0),
    ("DOM_KEY_LOCATION_LEFT", 1),
    ("DOM_KEY_LOCATION_RIGHT", 2),
    ("DOM_KEY_LOCATION_NUMPAD", 3),
];

impl Payload {
    /// The payload of a host event named `name` that carries `payload`.
    pub(super) fn from_host(name: &str, payload: &EventPayload) -> Self {
        match payload {
            EventPayload::None => Self::None,
            EventPayload::Focus { .. } => Self::Focus(Ui::default()),
            EventPayload::Mouse(mouse) => {
                let fields = Mouse::from_host(name, mouse);
                match mouse.pointer {
                    Some(source) => {
                        Self::Pointer(fields, Pointer::from_host(source, mouse.buttons))
                    }
                    None => Self::Mouse(fields),
                }
            }
            EventPayload::Wheel(wheel) => Self::Wheel(
                Mouse::from_host(name, &wheel.mouse),
                Wheel {
                    x: f64::from(wheel.delta_x),
                    y: f64::from(wheel.delta_y),
                    z: f64::from(wheel.delta_z),
                    mode: u32::from(wheel.delta_mode),
                },
            ),
            EventPayload::Key(key) => {
                let code = key_code(&key.key);
                Self::Keyboard(
                    Ui {
                        detail: 0,
                        which: code,
                    },
                    Keyboard {
                        key: key.key.clone(),
                        code: String::new(),
                        // gpui reports no key location or composition state.
                        location: 0,
                        repeat: key.repeat,
                        is_composing: false,
                        modifiers: key.modifiers,
                        key_code: code,
                        char_code: 0,
                    },
                )
            }
        }
    }

    /// The kind of a host event that carries `payload`: the store key of its prototype.
    pub(super) fn host_kind(payload: &EventPayload) -> &'static str {
        match payload {
            EventPayload::None => EVENT,
            EventPayload::Focus { .. } => FOCUS,
            EventPayload::Mouse(mouse) if mouse.pointer.is_some() => POINTER,
            EventPayload::Mouse(_) => MOUSE,
            EventPayload::Wheel(_) => WHEEL,
            EventPayload::Key(_) => KEYBOARD,
        }
    }
}

impl Mouse {
    /// The fields of a host mouse event named `name`.
    fn from_host(name: &str, mouse: &MousePayload) -> Self {
        let which = if BUTTON_EVENTS.contains(&name) {
            u32::from(mouse.button) + 1
        } else if name == "wheel" {
            0
        } else {
            held_button(mouse.buttons).map_or(0, |index| u32::from(index) + 1)
        };
        Self {
            ui: Ui {
                detail: i32::try_from(mouse.detail).unwrap_or(i32::MAX),
                which,
            },
            screen_x: f64::from(mouse.screen_x),
            screen_y: f64::from(mouse.screen_y),
            client_x: f64::from(mouse.client_x),
            client_y: f64::from(mouse.client_y),
            offset_x: f64::from(mouse.offset_x),
            offset_y: f64::from(mouse.offset_y),
            movement_x: f64::from(mouse.movement_x),
            movement_y: f64::from(mouse.movement_y),
            button: i16::from(mouse.button),
            buttons: u16::from(mouse.buttons),
            modifiers: mouse.modifiers,
        }
    }
}

impl Pointer {
    /// The pointer fields of a host event from `source` while `buttons` are held. The host has
    /// one mouse pointer, and the click of a key has no pointer.
    fn from_host(source: PointerSource, buttons: u8) -> Self {
        let mouse = source == PointerSource::Mouse;
        Self {
            id: if mouse { 1 } else { -1 },
            width: 1.0,
            height: 1.0,
            // Pointer events report 0.5 while a button is held.
            pressure: if buttons == 0 { 0.0 } else { 0.5 },
            kind: if mouse { "mouse" } else { "" }.to_owned(),
            is_primary: mouse,
        }
    }
}

/// The lowest button index whose bit `buttons` holds.
fn held_button(buttons: u8) -> Option<u8> {
    (0..5).find(|&index| buttons & buttons_mask(index) != 0)
}

/// A payload field as a listener reads it.
#[derive(Clone, Copy)]
enum Field<'a> {
    Number(f64),
    Bool(bool),
    Text(&'a str),
    Null,
}

impl<'js> IntoJs<'js> for Field<'_> {
    fn into_js(self, ctx: &Ctx<'js>) -> JsResult<Value<'js>> {
        match self {
            Field::Number(number) => number.into_js(ctx),
            Field::Bool(flag) => flag.into_js(ctx),
            Field::Text(text) => text.into_js(ctx),
            Field::Null => Ok(Value::new_null(ctx.clone())),
        }
    }
}

/// Reads one field of a payload of type `P`.
type Read<P> = for<'a> fn(&'a P) -> Field<'a>;

/// The fields of every UI event.
const UI_FIELDS: &[(&str, Read<Ui>)] = &[
    // The host has no `Window` object for an event to name.
    ("view", |_| Field::Null),
    ("detail", |ui| Field::Number(ui.detail.into())),
    ("which", |ui| Field::Number(ui.which.into())),
];

/// The fields of mouse events, wheel events included.
const MOUSE_FIELDS: &[(&str, Read<Mouse>)] = &[
    ("screenX", |mouse| Field::Number(mouse.screen_x)),
    ("screenY", |mouse| Field::Number(mouse.screen_y)),
    ("clientX", |mouse| Field::Number(mouse.client_x)),
    ("clientY", |mouse| Field::Number(mouse.client_y)),
    ("ctrlKey", |mouse| Field::Bool(mouse.modifiers.control)),
    ("shiftKey", |mouse| Field::Bool(mouse.modifiers.shift)),
    ("altKey", |mouse| Field::Bool(mouse.modifiers.alt)),
    ("metaKey", |mouse| Field::Bool(mouse.modifiers.platform)),
    ("button", |mouse| {
        Field::Number(if mouse.button == -1 {
            0.0
        } else {
            mouse.button.into()
        })
    }),
    ("buttons", |mouse| Field::Number(mouse.buttons.into())),
    // Equal to clientX/clientY: no host scrolling moves the page itself.
    ("pageX", |mouse| Field::Number(mouse.client_x)),
    ("pageY", |mouse| Field::Number(mouse.client_y)),
    ("x", |mouse| Field::Number(mouse.client_x)),
    ("y", |mouse| Field::Number(mouse.client_y)),
    ("offsetX", |mouse| Field::Number(mouse.offset_x)),
    ("offsetY", |mouse| Field::Number(mouse.offset_y)),
    ("movementX", |mouse| Field::Number(mouse.movement_x)),
    ("movementY", |mouse| Field::Number(mouse.movement_y)),
];

/// The fields of mouse events with a pointer source.
const POINTER_FIELDS: &[(&str, Read<Pointer>)] = &[
    ("pointerId", |pointer| Field::Number(pointer.id.into())),
    ("width", |pointer| Field::Number(pointer.width)),
    ("height", |pointer| Field::Number(pointer.height)),
    ("pressure", |pointer| Field::Number(pointer.pressure.into())),
    ("pointerType", |pointer| Field::Text(&pointer.kind)),
    ("isPrimary", |pointer| Field::Bool(pointer.is_primary)),
];

/// The fields of wheel events beyond the mouse ones.
const WHEEL_FIELDS: &[(&str, Read<Wheel>)] = &[
    ("deltaX", |wheel| Field::Number(wheel.x)),
    ("deltaY", |wheel| Field::Number(wheel.y)),
    ("deltaZ", |wheel| Field::Number(wheel.z)),
    ("deltaMode", |wheel| Field::Number(wheel.mode.into())),
];

/// The fields of key events.
const KEY_FIELDS: &[(&str, Read<Keyboard>)] = &[
    ("key", |key| Field::Text(&key.key)),
    ("code", |key| Field::Text(&key.code)),
    ("location", |key| Field::Number(key.location.into())),
    ("ctrlKey", |key| Field::Bool(key.modifiers.control)),
    ("shiftKey", |key| Field::Bool(key.modifiers.shift)),
    ("altKey", |key| Field::Bool(key.modifiers.alt)),
    ("metaKey", |key| Field::Bool(key.modifiers.platform)),
    ("repeat", |key| Field::Bool(key.repeat)),
    ("isComposing", |key| Field::Bool(key.is_composing)),
    ("charCode", |key| Field::Number(key.char_code.into())),
    ("keyCode", |key| Field::Number(key.key_code.into())),
];

/// The UI fields of `payload`.
fn ui_of(payload: &Payload) -> Option<&Ui> {
    match payload {
        Payload::Ui(ui) | Payload::Focus(ui) | Payload::Keyboard(ui, _) => Some(ui),
        Payload::Mouse(mouse) | Payload::Pointer(mouse, _) | Payload::Wheel(mouse, _) => {
            Some(&mouse.ui)
        }
        Payload::None => None,
    }
}

/// The mouse fields of `payload`. A pointer or wheel payload carries them too.
fn mouse_of(payload: &Payload) -> Option<&Mouse> {
    match payload {
        Payload::Mouse(mouse) | Payload::Pointer(mouse, _) | Payload::Wheel(mouse, _) => {
            Some(mouse)
        }
        _ => None,
    }
}

fn pointer_of(payload: &Payload) -> Option<&Pointer> {
    match payload {
        Payload::Pointer(_, pointer) => Some(pointer),
        _ => None,
    }
}

fn wheel_of(payload: &Payload) -> Option<&Wheel> {
    match payload {
        Payload::Wheel(_, wheel) => Some(wheel),
        _ => None,
    }
}

fn key_of(payload: &Payload) -> Option<&Keyboard> {
    match payload {
        Payload::Keyboard(_, key) => Some(key),
        _ => None,
    }
}

/// The prototype of UI events: the UI fields. It inherits from `event`.
pub(super) fn ui_prototype<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, event)?;
    define_fields(ctx, &proto, ui_of, UI_FIELDS)?;
    Ok(proto)
}

/// The prototype of mouse events: the mouse fields, `relatedTarget` and `getModifierState`. It
/// inherits from `ui`.
pub(super) fn mouse_prototype<'js>(ctx: &Ctx<'js>, ui: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, ui)?;
    define_fields(ctx, &proto, mouse_of, MOUSE_FIELDS)?;
    related_target(ctx, &proto, |payload| mouse_of(payload).is_some())?;
    modifier_state(ctx, &proto, |payload| {
        mouse_of(payload).map(|mouse| mouse.modifiers)
    })?;
    Ok(proto)
}

/// The prototype of mouse events with a pointer source: the pointer fields. It inherits from
/// `mouse`.
pub(super) fn pointer_prototype<'js>(ctx: &Ctx<'js>, mouse: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, mouse)?;
    define_fields(ctx, &proto, pointer_of, POINTER_FIELDS)?;
    Ok(proto)
}

/// The prototype of wheel events: the wheel fields. It inherits from `mouse`.
pub(super) fn wheel_prototype<'js>(ctx: &Ctx<'js>, mouse: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, mouse)?;
    define_fields(ctx, &proto, wheel_of, WHEEL_FIELDS)?;
    constants(&proto, &DELTA_MODES)?;
    Ok(proto)
}

/// The prototype of key events: the key fields and `getModifierState`. It inherits from `ui`.
pub(super) fn keyboard_prototype<'js>(ctx: &Ctx<'js>, ui: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, ui)?;
    define_fields(ctx, &proto, key_of, KEY_FIELDS)?;
    constants(&proto, &KEY_LOCATIONS)?;
    modifier_state(ctx, &proto, |payload| {
        key_of(payload).map(|key| key.modifiers)
    })?;
    Ok(proto)
}

/// The prototype of focus events: `relatedTarget`. It inherits from `ui`.
pub(super) fn focus_prototype<'js>(ctx: &Ctx<'js>, ui: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, ui)?;
    related_target(ctx, &proto, |payload| matches!(payload, Payload::Focus(_)))?;
    Ok(proto)
}

/// Defines on `proto` a getter per entry of `fields`. A getter reads the payload `of` finds in
/// the event, and throws a `TypeError` for an event without one.
fn define_fields<'js, P: 'static>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    of: fn(&Payload) -> Option<&P>,
    fields: &'static [(&'static str, Read<P>)],
) -> JsResult<()> {
    for &(key, read) in fields {
        field(ctx, proto, key, move |ctx, event| {
            let payload = of(&event.state.payload).ok_or_else(|| other_kind(ctx, key))?;
            read(payload).into_js(ctx)
        })?;
    }
    Ok(())
}

/// Defines on `proto` the getter `relatedTarget`. It throws a `TypeError` for an event whose
/// payload `has` rejects.
fn related_target<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    has: fn(&Payload) -> bool,
) -> JsResult<()> {
    field(ctx, proto, "relatedTarget", move |ctx, event| {
        if !has(&event.state.payload) {
            return Err(other_kind(ctx, "relatedTarget"));
        }
        or_null(ctx, event.held.related.clone())
    })
}

/// Defines `getModifierState` on `proto`. It reads the modifiers `of` finds in the event, and
/// throws a `TypeError` for an event without them. Accel reads Meta on macOS and Control
/// elsewhere.
fn modifier_state<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    of: fn(&Payload) -> Option<Modifiers>,
) -> JsResult<()> {
    let function = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>, name: Coerced<String>| {
            with_event(&ctx, &this, |event| {
                let modifiers =
                    of(&event.state.payload).ok_or_else(|| other_kind(&ctx, "getModifierState"))?;
                Ok(match name.0.as_str() {
                    "Accel" if cfg!(target_os = "macos") => modifiers.platform,
                    "Accel" | "Control" => modifiers.control,
                    "Shift" => modifiers.shift,
                    "Alt" => modifiers.alt,
                    "Meta" => modifiers.platform,
                    _ => false,
                })
            })
        },
    )?;
    method(proto, "getModifierState", function)
}
