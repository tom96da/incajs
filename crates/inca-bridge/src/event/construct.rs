// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_gpui::{EventPayload, NodeId};
use rquickjs::atom::PredefinedAtom;
use rquickjs::convert::Coerced;
use rquickjs::function::{Constructor, IntoJsFunc, ParamRequirement, Params};
use rquickjs::object::Property;
use rquickjs::{Array, Ctx, Exception, Function, Null, Object, Result as JsResult, Value};

use super::payload::set_payload;
use super::walk::Walk;
use super::{
    EVENT, Flags, KEYBOARD, MOUSE, PHASES, REMEMBER, path_value, piece, target_at, targets_of,
    time_stamp,
};
use crate::bindings::convert::or_null;

/// A new idle event object and its record, filed in the realm's record
/// store. The event inherits from the prototype of kind `prototype`, and the
/// record keeps that kind as `kind`.
fn new_event<'js>(
    ctx: &Ctx<'js>,
    kind: &str,
    flags: Flags,
    trusted: bool,
    prototype: &'static str,
) -> JsResult<(Object<'js>, Object<'js>)> {
    let Flags {
        bubbles,
        cancelable,
        composed,
    } = flags;
    let proto: Object = piece(ctx, prototype)?;
    let remember: Function = piece(ctx, REMEMBER)?;

    let record = Object::new(ctx.clone())?;
    record.set_prototype(None)?;
    record.set("type", kind)?;
    record.set("stop", false)?;
    record.set("immediate", false)?;
    record.set("canceled", false)?;
    record.set("passive", false)?;
    record.set("bubbles", bubbles)?;
    record.set("cancelable", cancelable)?;
    record.set("composed", composed)?;
    record.set("dispatching", false)?;
    record.set("path", Vec::<NodeId>::new())?;
    record.set("kind", prototype)?;

    let event = Object::new(ctx.clone())?;
    event.set_prototype(Some(&proto))?;
    event.set("type", kind)?;
    event.set("target", Null)?;
    event.set("currentTarget", Null)?;
    event.set("bubbles", bubbles)?;
    event.set("cancelable", cancelable)?;
    event.set("composed", composed)?;
    event.set("eventPhase", 0)?;
    event.set("isTrusted", trusted)?;
    event.set("timeStamp", time_stamp())?;

    remember.call::<_, ()>((event.clone(), record.clone()))?;
    Ok((event, record))
}

/// A new event object for a walk, ready to deliver. `targets` holds what
/// stands for each node of the path.
pub(super) fn create<'js>(
    ctx: &Ctx<'js>,
    walk: &Walk<'_>,
    id: u64,
    targets: Option<&Array<'js>>,
) -> JsResult<(Object<'js>, Object<'js>)> {
    let (event, record) = new_event(
        ctx,
        walk.name,
        walk.flags,
        walk.trusted,
        prototype_kind(walk.payload),
    )?;
    record.set("dispatching", true)?;
    record.set("path", path_value(ctx, targets, walk.path)?)?;
    event.set("target", target_at(ctx, targets, 0, walk.target)?)?;
    set_payload(&event, walk.payload)?;
    if let Some(related) = related_of(walk.payload)
        && let Some(found) = targets_of(ctx, &[related])?
    {
        event.set("relatedTarget", found.get::<Value>(0)?)?;
    }
    event.set("eventId", id)?;
    Ok((event, record))
}

/// The kind of the prototype that an event carrying `payload` inherits from.
fn prototype_kind(payload: &EventPayload) -> &'static str {
    match payload {
        EventPayload::Mouse(_) | EventPayload::Wheel(_) => MOUSE,
        EventPayload::Key(_) => KEYBOARD,
        EventPayload::None | EventPayload::Focus { .. } => EVENT,
    }
}

/// The other node `payload` names, if any.
fn related_of(payload: &EventPayload) -> Option<NodeId> {
    match payload {
        EventPayload::Mouse(mouse) => mouse.related_target,
        EventPayload::Wheel(wheel) => wheel.mouse.related_target,
        EventPayload::Focus { related_target } => *related_target,
        EventPayload::None | EventPayload::Key(_) => None,
    }
}

/// Installs the global constructors `Event` and `CustomEvent`.
pub(super) fn constructors<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let event = Constructor::new_prototype(
        ctx,
        proto.clone(),
        Construct {
            name: "Event",
            custom: false,
        },
    )?;
    event.set_name("Event")?;
    event.set_length(1)?;
    let custom_proto = Object::new(ctx.clone())?;
    custom_proto.set_prototype(Some(proto))?;
    custom_proto.prop(
        PredefinedAtom::SymbolToStringTag,
        Property::from("CustomEvent").configurable(),
    )?;
    let custom = Constructor::new_prototype(
        ctx,
        custom_proto,
        Construct {
            name: "CustomEvent",
            custom: true,
        },
    )?;
    custom.set_name("CustomEvent")?;
    custom.set_length(1)?;
    custom.set_prototype(Some(&event))?;
    for (name, value) in PHASES {
        event.prop(name, Property::from(value).enumerable())?;
    }
    let globals = ctx.globals();
    globals.set("Event", event)?;
    globals.set("CustomEvent", custom)
}

/// The body of `Event` and `CustomEvent`.
struct Construct {
    name: &'static str,
    custom: bool,
}

impl<'js> IntoJsFunc<'js, ()> for Construct {
    fn param_requirements() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
        let ctx = params.ctx();
        let name = self.name;
        let type_error = |message: &str| {
            Exception::throw_type(ctx, &format!("cannot construct {name}: {message}"))
        };
        if !params.is_constructor() {
            return Err(type_error(&format!(
                "the event constructor needs new, so write new {name}(type)"
            )));
        }
        let Some(kind) = params.arg(0) else {
            return Err(type_error(concat!(
                "the event type is missing, so pass a type string such as \"click\" ",
                "as the first argument"
            )));
        };
        let kind = kind.get::<Coerced<String>>()?.0;
        let init = match params.arg(1) {
            Some(value) if !value.is_undefined() && !value.is_null() => {
                Some(value.into_object().ok_or_else(|| {
                    type_error(concat!(
                        "the event init is not an object, so pass an object such as ",
                        "{ bubbles: true } or leave it out"
                    ))
                })?)
            }
            _ => None,
        };
        let read = |key: &str| -> JsResult<bool> {
            init.as_ref().map_or(Ok(false), |init| {
                init.get::<_, Coerced<bool>>(key).map(|c| c.0)
            })
        };
        let flags = Flags {
            bubbles: read("bubbles")?,
            cancelable: read("cancelable")?,
            composed: read("composed")?,
        };
        let (event, _) = new_event(ctx, &kind, flags, false, EVENT)?;
        if self.custom {
            let detail = init
                .as_ref()
                .map(|init| init.get::<_, Value>("detail"))
                .transpose()?
                .filter(|detail| !detail.is_undefined());
            event.set("detail", or_null(ctx, detail)?)?;
        }
        Ok(event.into_value())
    }
}
