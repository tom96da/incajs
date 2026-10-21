// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_jsenv::FunctionAccessor;
use rquickjs::convert::Coerced;
use rquickjs::function::{Opt, This};
use rquickjs::{Ctx, Function, IntoJs, Object, Result as JsResult, Value};

use super::{
    IncaEvent, action, array_of, constants, field, getter, given, initialize, method, with_event,
};
use crate::bindings::convert::or_null;

/// The phase constants on `Event` and its prototype.
pub(super) const PHASES: [(&str, i32); 4] = [
    ("NONE", 0),
    ("CAPTURING_PHASE", 1),
    ("AT_TARGET", 2),
    ("BUBBLING_PHASE", 3),
];

/// `Event.prototype`: the phase constants, the propagation methods,
/// `composedPath`, `initEvent` and the getters of the base fields.
pub(super) fn event_prototype<'js>(ctx: &Ctx<'js>) -> JsResult<Object<'js>> {
    let proto = Object::new(ctx.clone())?;
    constants(&proto, &PHASES)?;
    define_methods(ctx, &proto)?;
    define_fields(ctx, &proto)?;
    Ok(proto)
}

/// The getter every event owns as `isTrusted`.
pub(super) fn is_trusted<'js>(ctx: &Ctx<'js>) -> JsResult<Function<'js>> {
    getter(ctx, "isTrusted", |ctx, event| {
        event.state.trusted.get().into_js(ctx)
    })
}

fn define_methods<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    action(ctx, proto, "stopPropagation", |event| {
        event.state.stop.set(true);
    })?;
    action(ctx, proto, "stopImmediatePropagation", |event| {
        event.state.stop.set(true);
        event.state.immediate.set(true);
    })?;
    action(ctx, proto, "preventDefault", |event| {
        if event.cancels() {
            event.state.canceled.set(true);
        }
    })?;
    let composed_path = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
            with_event(&ctx, &this, |event| {
                let path = if event.state.dispatching.get() {
                    event.held.path.borrow().clone()
                } else {
                    Vec::new()
                };
                array_of(&ctx, path)
            })
        },
    )?;
    let init_event = Function::new(
        ctx.clone(),
        |ctx: Ctx<'js>,
         This(this): This<Value<'js>>,
         event_type: Coerced<String>,
         bubbles: Opt<Coerced<bool>>,
         cancelable: Opt<Coerced<bool>>| {
            with_event(&ctx, &this, |event| {
                initialize(event, event_type.0, given(&bubbles), given(&cancelable));
                Ok(())
            })
        },
    )?;
    method(proto, "initEvent", init_event)?;
    method(proto, "composedPath", composed_path)
}

fn define_fields<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    field(ctx, proto, "type", |ctx, event| {
        event.state.name.borrow().as_str().into_js(ctx)
    })?;
    field(ctx, proto, "target", |ctx, event| {
        let target = event.held.target.borrow().clone();
        or_null(ctx, target)
    })?;
    field(ctx, proto, "currentTarget", |ctx, event| {
        or_null(ctx, event.current_target())
    })?;
    field(ctx, proto, "eventPhase", |ctx, event| {
        event.state.phase.get().into_js(ctx)
    })?;
    field(ctx, proto, "bubbles", |ctx, event| {
        event.state.flags.get().bubbles.into_js(ctx)
    })?;
    field(ctx, proto, "cancelable", |ctx, event| {
        event.state.flags.get().cancelable.into_js(ctx)
    })?;
    field(ctx, proto, "defaultPrevented", |ctx, event| {
        event.state.canceled.get().into_js(ctx)
    })?;
    field(ctx, proto, "composed", |ctx, event| {
        event.state.flags.get().composed.into_js(ctx)
    })?;
    field(ctx, proto, "timeStamp", |ctx, event| {
        event.state.time_stamp.into_js(ctx)
    })?;
    field(ctx, proto, "srcElement", |ctx, event| {
        let target = event.held.target.borrow().clone();
        or_null(ctx, target)
    })?;
    let return_value = getter(ctx, "returnValue", |ctx, event| {
        (!event.state.canceled.get()).into_js(ctx)
    })?;
    let set_return_value = setter(ctx, "returnValue", |event, value| {
        if !value && event.cancels() {
            event.state.canceled.set(true);
        }
    })?;
    proto.prop(
        "returnValue",
        FunctionAccessor::new_get(return_value)
            .set(set_return_value)
            .enumerable()
            .configurable(),
    )?;
    let cancel_bubble = getter(ctx, "cancelBubble", |ctx, event| {
        event.state.stop.get().into_js(ctx)
    })?;
    let set_cancel_bubble = setter(ctx, "cancelBubble", |event, value| {
        if value {
            event.state.stop.set(true);
        }
    })?;
    proto.prop(
        "cancelBubble",
        FunctionAccessor::new_get(cancel_bubble)
            .set(set_cancel_bubble)
            .enumerable()
            .configurable(),
    )?;
    // The host's own field, so it stays out of `for...in`.
    let event_id = getter(ctx, "eventId", |ctx, event| {
        event.state.event_id.get().into_js(ctx)
    })?;
    proto.prop(
        "eventId",
        FunctionAccessor::new_get(event_id).configurable(),
    )
}

/// The setter `set <key>`, which runs `write` with the value converted to a
/// boolean on the event `this` names.
fn setter<'js>(
    ctx: &Ctx<'js>,
    key: &str,
    write: fn(&IncaEvent<'_>, bool),
) -> JsResult<Function<'js>> {
    Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>, value: Coerced<bool>| {
            with_event(&ctx, &this, |event| {
                write(event, value.0);
                Ok(())
            })
        },
    )?
    .with_name(format!("set {key}"))
}
