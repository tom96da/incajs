// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::Modifiers;
use inca_gpui::{EventPayload, KeyPayload, MousePayload, PointerSource, WheelPayload};
use rquickjs::convert::Coerced;
use rquickjs::function::This;
use rquickjs::{Ctx, Function, IntoJs, Object, Result as JsResult, Value};

use super::{child_of, field, method, other_kind, with_event};
use crate::bindings::convert::or_null;

/// A payload field as a listener reads it.
#[derive(Clone, Copy)]
enum Field<'a> {
    Number(f64),
    Bool(bool),
    Text(&'a str),
}

impl<'js> IntoJs<'js> for Field<'_> {
    fn into_js(self, ctx: &Ctx<'js>) -> JsResult<Value<'js>> {
        match self {
            Field::Number(number) => number.into_js(ctx),
            Field::Bool(flag) => flag.into_js(ctx),
            Field::Text(text) => text.into_js(ctx),
        }
    }
}

/// Reads one field of a payload of type `P`.
type Read<P> = for<'a> fn(&'a P) -> Field<'a>;

/// The fields of mouse events, wheel events included.
const MOUSE_FIELDS: &[(&str, Read<MousePayload>)] = &[
    ("screenX", |mouse| Field::Number(mouse.screen_x.into())),
    ("screenY", |mouse| Field::Number(mouse.screen_y.into())),
    ("clientX", |mouse| Field::Number(mouse.client_x.into())),
    ("clientY", |mouse| Field::Number(mouse.client_y.into())),
    ("ctrlKey", |mouse| Field::Bool(mouse.modifiers.control)),
    ("shiftKey", |mouse| Field::Bool(mouse.modifiers.shift)),
    ("altKey", |mouse| Field::Bool(mouse.modifiers.alt)),
    ("metaKey", |mouse| Field::Bool(mouse.modifiers.platform)),
    ("button", |mouse| Field::Number(mouse.button.into())),
    ("buttons", |mouse| Field::Number(mouse.buttons.into())),
    // Equal to clientX/clientY: no host scrolling moves the page itself.
    ("pageX", |mouse| Field::Number(mouse.client_x.into())),
    ("pageY", |mouse| Field::Number(mouse.client_y.into())),
    ("x", |mouse| Field::Number(mouse.client_x.into())),
    ("y", |mouse| Field::Number(mouse.client_y.into())),
    ("offsetX", |mouse| Field::Number(mouse.offset_x.into())),
    ("offsetY", |mouse| Field::Number(mouse.offset_y.into())),
    ("movementX", |mouse| Field::Number(mouse.movement_x.into())),
    ("movementY", |mouse| Field::Number(mouse.movement_y.into())),
    ("detail", |mouse| Field::Number(mouse.detail.into())),
];

/// The fields of mouse events with a pointer source. The host has one mouse
/// pointer, and the click of a key has no pointer.
const POINTER_FIELDS: &[(&str, Read<MousePayload>)] = &[
    ("pointerId", |mouse| {
        Field::Number(if from_mouse(mouse) { 1.0 } else { -1.0 })
    }),
    ("width", |_| Field::Number(1.0)),
    ("height", |_| Field::Number(1.0)),
    // Pointer events report 0.5 while a button is held.
    ("pressure", |mouse| {
        Field::Number(if mouse.buttons == 0 { 0.0 } else { 0.5 })
    }),
    ("pointerType", |mouse| {
        Field::Text(if from_mouse(mouse) { "mouse" } else { "" })
    }),
    ("isPrimary", |mouse| Field::Bool(from_mouse(mouse))),
];

/// The fields of wheel events beyond the mouse ones.
const WHEEL_FIELDS: &[(&str, Read<WheelPayload>)] = &[
    ("deltaX", |wheel| Field::Number(wheel.delta_x.into())),
    ("deltaY", |wheel| Field::Number(wheel.delta_y.into())),
    ("deltaZ", |wheel| Field::Number(wheel.delta_z.into())),
    ("deltaMode", |wheel| Field::Number(wheel.delta_mode.into())),
];

/// The fields of key events.
const KEY_FIELDS: &[(&str, Read<KeyPayload>)] = &[
    ("key", |key| Field::Text(&key.key)),
    // gpui reports no key location or composition state.
    ("location", |_| Field::Number(0.0)),
    ("ctrlKey", |key| Field::Bool(key.modifiers.control)),
    ("shiftKey", |key| Field::Bool(key.modifiers.shift)),
    ("altKey", |key| Field::Bool(key.modifiers.alt)),
    ("metaKey", |key| Field::Bool(key.modifiers.platform)),
    ("repeat", |key| Field::Bool(key.repeat)),
    ("isComposing", |_| Field::Bool(false)),
];

/// Whether `mouse` comes from the mouse pointer.
fn from_mouse(mouse: &MousePayload) -> bool {
    mouse.pointer == Some(PointerSource::Mouse)
}

/// The mouse fields of `payload`. A wheel payload carries them too.
fn mouse_of(payload: &EventPayload) -> Option<&MousePayload> {
    match payload {
        EventPayload::Mouse(mouse) => Some(mouse),
        EventPayload::Wheel(wheel) => Some(&wheel.mouse),
        _ => None,
    }
}

/// The mouse fields of `payload` when it has a pointer source.
fn pointer_of(payload: &EventPayload) -> Option<&MousePayload> {
    match payload {
        EventPayload::Mouse(mouse) if mouse.pointer.is_some() => Some(mouse),
        _ => None,
    }
}

fn wheel_of(payload: &EventPayload) -> Option<&WheelPayload> {
    match payload {
        EventPayload::Wheel(wheel) => Some(wheel),
        _ => None,
    }
}

fn key_of(payload: &EventPayload) -> Option<&KeyPayload> {
    match payload {
        EventPayload::Key(key) => Some(key),
        _ => None,
    }
}

/// The prototype of mouse events: the mouse fields, `relatedTarget` and
/// `getModifierState`. It inherits from `event`.
pub(super) fn mouse_prototype<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, event)?;
    define_fields(ctx, &proto, mouse_of, MOUSE_FIELDS)?;
    related_target(ctx, &proto, |payload| mouse_of(payload).is_some())?;
    modifier_state(ctx, &proto, |payload| {
        mouse_of(payload).map(|mouse| mouse.modifiers)
    })?;
    Ok(proto)
}

/// The prototype of mouse events with a pointer source: the pointer fields.
/// It inherits from `mouse`.
pub(super) fn pointer_prototype<'js>(ctx: &Ctx<'js>, mouse: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, mouse)?;
    define_fields(ctx, &proto, pointer_of, POINTER_FIELDS)?;
    Ok(proto)
}

/// The prototype of wheel events: the wheel fields. It inherits from `mouse`.
pub(super) fn wheel_prototype<'js>(ctx: &Ctx<'js>, mouse: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, mouse)?;
    define_fields(ctx, &proto, wheel_of, WHEEL_FIELDS)?;
    Ok(proto)
}

/// The prototype of key events: the key fields and `getModifierState`. It
/// inherits from `event`.
pub(super) fn keyboard_prototype<'js>(
    ctx: &Ctx<'js>,
    event: &Object<'js>,
) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, event)?;
    define_fields(ctx, &proto, key_of, KEY_FIELDS)?;
    modifier_state(ctx, &proto, |payload| {
        key_of(payload).map(|key| key.modifiers)
    })?;
    Ok(proto)
}

/// The prototype of focus events: `relatedTarget`. It inherits from `event`.
pub(super) fn focus_prototype<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Object<'js>> {
    let proto = child_of(ctx, event)?;
    related_target(ctx, &proto, |payload| {
        matches!(payload, EventPayload::Focus { .. })
    })?;
    Ok(proto)
}

/// Defines on `proto` a getter per entry of `fields`. A getter reads the
/// payload `of` finds in the event, and throws a `TypeError` for an event
/// without one.
fn define_fields<'js, P: 'static>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    of: fn(&EventPayload) -> Option<&P>,
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

/// Defines on `proto` the getter `relatedTarget`. It throws a `TypeError` for
/// an event whose payload `has` rejects.
fn related_target<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    has: fn(&EventPayload) -> bool,
) -> JsResult<()> {
    field(ctx, proto, "relatedTarget", move |ctx, event| {
        if !has(&event.state.payload) {
            return Err(other_kind(ctx, "relatedTarget"));
        }
        or_null(ctx, event.held.related.clone())
    })
}

/// Defines `getModifierState` on `proto`. It reads the modifiers `of` finds
/// in the event, and throws a `TypeError` for an event without them. Accel
/// reads Meta on macOS and Control elsewhere.
fn modifier_state<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    of: fn(&EventPayload) -> Option<Modifiers>,
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
