// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;

use inca_gpui::{EventPayload, NodeId};
use rquickjs::atom::PredefinedAtom;
use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, ParamRequirement, Params};
use rquickjs::object::Property;
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, Value};

use super::members::PHASES;
use super::walk::Walk;
use super::{
    CUSTOM, EVENT, FOCUS, Flags, Held, IncaEvent, KEYBOARD, MOUSE, POINTER, State, WHEEL, child_of,
    field, new_event, other_kind, targets_of,
};
use crate::bindings::convert::or_null;

/// A new event for a walk, ready to deliver. `targets` holds what stands for
/// each node of the path, target first.
pub(super) fn create<'js>(
    ctx: &Ctx<'js>,
    walk: &Walk<'_>,
    id: u64,
    targets: Vec<Value<'js>>,
) -> JsResult<Class<'js, IncaEvent<'js>>> {
    let related = match related_of(walk.payload) {
        Some(node) => targets_of(ctx, &[node])?.pop(),
        None => None,
    };
    let state = State::new(
        walk.name.to_owned(),
        walk.flags,
        walk.payload.clone(),
        walk.trusted,
    );
    state.event_id.set(Some(id));
    state.dispatching.set(true);
    let event = IncaEvent {
        state,
        held: Held {
            target: RefCell::new(targets.first().cloned()),
            path: RefCell::new(targets),
            related,
            detail: None,
        },
    };
    new_event(ctx, kind_of(walk.payload), event)
}

/// The kind of a host event that carries `payload`.
fn kind_of(payload: &EventPayload) -> &'static str {
    match payload {
        EventPayload::None => EVENT,
        EventPayload::Focus { .. } => FOCUS,
        EventPayload::Mouse(mouse) if mouse.pointer.is_some() => POINTER,
        EventPayload::Mouse(_) => MOUSE,
        EventPayload::Wheel(_) => WHEEL,
        EventPayload::Key(_) => KEYBOARD,
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

/// Installs the global constructors `Event` and `CustomEvent`, and returns
/// `CustomEvent.prototype`, which inherits from `proto`.
pub(super) fn constructors<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<Object<'js>> {
    let event = constructor(ctx, proto, "Event", false)?;
    let custom_proto = child_of(ctx, proto)?;
    field(ctx, &custom_proto, "detail", |ctx, event| {
        event
            .held
            .detail
            .clone()
            .ok_or_else(|| other_kind(ctx, "detail"))
    })?;
    custom_proto.prop(
        PredefinedAtom::SymbolToStringTag,
        Property::from("CustomEvent").configurable(),
    )?;
    let custom = constructor(ctx, &custom_proto, "CustomEvent", true)?;
    custom.set_prototype(Some(&event))?;
    for (name, value) in PHASES {
        event.prop(name, Property::from(value).enumerable())?;
    }
    let globals = ctx.globals();
    globals.set("Event", event)?;
    globals.set("CustomEvent", custom)?;
    Ok(custom_proto)
}

/// The constructor `name` of length 1, linked with `proto` as
/// `JS_SetConstructor` links a built-in: `name.prototype` is read-only and
/// `proto.constructor` is writable and configurable.
fn constructor<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &'static str,
    custom: bool,
) -> JsResult<Function<'js>> {
    let function = Function::new(ctx.clone(), Construct { name, custom })?
        .with_name(name)?
        .with_constructor(true);
    function.set_length(1)?;
    function.prop("prototype", Property::from(proto.clone()))?;
    proto.prop(
        "constructor",
        Property::from(function.clone()).writable().configurable(),
    )?;
    Ok(function)
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
        let Some(event_type) = params.arg(0) else {
            return Err(type_error(concat!(
                "the event type is missing, so pass a type string such as \"click\" ",
                "as the first argument"
            )));
        };
        let event_type = event_type.get::<Coerced<String>>()?.0;
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
        let state = State::new(event_type, flags, EventPayload::None, false);
        let (kind, detail) = if self.custom {
            let detail = init
                .as_ref()
                .map(|init| init.get::<_, Value>("detail"))
                .transpose()?
                .filter(|detail| !detail.is_undefined());
            (CUSTOM, Some(or_null(ctx, detail)?))
        } else {
            (EVENT, None)
        };
        let event = IncaEvent {
            state,
            held: Held {
                target: RefCell::new(None),
                path: RefCell::new(Vec::new()),
                related: None,
                detail,
            },
        };
        let event = new_event(ctx, kind, event)?;
        // `new.target`'s prototype when it is an object, as for a subclass;
        // the prototype of the kind otherwise.
        let target_proto = params
            .this()
            .as_object()
            .map(|target| target.get::<_, Value>("prototype"))
            .transpose()?
            .and_then(Value::into_object);
        if let Some(proto) = target_proto {
            event.set_prototype(Some(&proto))?;
        }
        Ok(event.into_value())
    }
}
