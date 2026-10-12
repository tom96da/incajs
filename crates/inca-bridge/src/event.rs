// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event object a listener receives and the walk of one event over its
//! path.
//!
//! One event is one object. The walk creates it, runs the capture listeners
//! from the root down to the target and then the bubble listeners from the
//! target up, all with that same object, then resets its `currentTarget` and
//! `eventPhase`. The object's methods and accessors live on a prototype built
//! once per runtime. What they read and write sits in a record that a
//! `WeakMap` keeps, so no script can reach it. The global constructors `Event`
//! and `CustomEvent` file their events in the same store, so a script can
//! build an event and dispatch it.

use std::sync::OnceLock;
use std::time::Instant;

use inca_gpui::NodeId;
use rquickjs::convert::Coerced;
use rquickjs::function::This;
use rquickjs::object::{Accessor, Property};
use rquickjs::{Ctx, Exception, Function, Object, Result as JsResult, Value};

mod construct;
mod payload;
mod walk;

use construct::constructors;
pub(crate) use walk::{Walk, click, dispatch_event, listened, path_from, walk};

/// `bubbles`, `cancelable` and `composed` of one event.
#[derive(Clone, Copy)]
pub(crate) struct Flags {
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
    /// Focus events: composed and not cancelable.
    pub const FOCUS: Self = Self {
        composed: true,
        ..Self::NONE
    };
}

/// What the event machinery keeps in the runtime's userdata, in the order of
/// [`PROTOTYPE`], [`REMEMBER`] and [`RECALL`].
// A `Vec<Value>` keeps clear of console's `Vec<Function>` userdata; the values belong to one context.
type Slots<'js> = Vec<Value<'js>>;

/// The prototype of every event object.
const PROTOTYPE: usize = 0;
/// `(event, record)`: files the record of a new event.
const REMEMBER: usize = 1;
/// `(event)`: the record of an event, `undefined` for any other value.
const RECALL: usize = 2;

/// Creates the prototype and the record store of `ctx`'s runtime. The first
/// call wins, and later calls on the same runtime leave it as it is.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or the runtime's
/// userdata is borrowed.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    if ctx.userdata::<Slots>().is_some() {
        return Ok(());
    }
    let [remember, recall]: [Function; 2] = ctx
        .eval::<Vec<Function>, _>(
            "(() => {
                const records = new WeakMap();
                const { get, set } = WeakMap.prototype;
                const { apply } = Reflect;
                return [
                    (event, record) => { apply(set, records, [event, record]); },
                    (event) => apply(get, records, [event]),
                ];
            })()",
        )?
        .try_into()
        .map_err(|_| rquickjs::Error::Unknown)?;
    let proto = prototype(ctx)?;
    let slots: Slots = vec![
        proto.clone().into_value(),
        remember.into_value(),
        recall.into_value(),
    ];
    ctx.store_userdata(slots).map_err(|_| {
        rquickjs::Error::new_from_js_message(
            "events",
            "userdata",
            "the runtime's userdata is borrowed",
        )
    })?;
    constructors(ctx, &proto)
}

/// Milliseconds since the first call. The dispatcher makes the first call
/// when it starts, so the origin is the host start.
pub(crate) fn time_stamp() -> f64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

fn slot<'js>(ctx: &Ctx<'js>, index: usize) -> JsResult<Value<'js>> {
    install(ctx)?;
    ctx.userdata::<Slots>()
        .map(|slots| slots[index].clone())
        .ok_or_else(|| Exception::throw_type(ctx, "events are not installed"))
}

fn function_slot<'js>(ctx: &Ctx<'js>, index: usize) -> JsResult<Function<'js>> {
    slot(ctx, index)?
        .into_function()
        .ok_or_else(|| Exception::throw_type(ctx, "events are not installed"))
}

/// The record of `event`, `None` for any object the host did not file.
fn find_record<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Option<Object<'js>>> {
    let recall = function_slot(ctx, RECALL)?;
    let record: Value = recall.call((event.clone(),))?;
    Ok(record.into_object())
}

/// The record of `event`.
///
/// # Errors
///
/// Throws a `TypeError` when `event` is not an event object of this host.
fn record_of<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Object<'js>> {
    find_record(ctx, event)?.ok_or_else(|| Exception::throw_type(ctx, "Illegal invocation"))
}

fn flag(record: &Object<'_>, key: &str) -> bool {
    record.get::<_, bool>(key).unwrap_or(false)
}

/// Whether a cancel request takes effect now: the event is cancelable and the
/// running listener is active.
fn cancels(record: &Object<'_>) -> bool {
    flag(record, "cancelable") && !flag(record, "passive")
}

/// The prototype of event objects: the phase constants, the propagation
/// methods, `composedPath` and the accessors.
fn prototype<'js>(ctx: &Ctx<'js>) -> JsResult<Object<'js>> {
    let proto = Object::new(ctx.clone())?;
    for (name, value) in [
        ("NONE", 0),
        ("CAPTURING_PHASE", 1),
        ("AT_TARGET", 2),
        ("BUBBLING_PHASE", 3),
    ] {
        proto.prop(name, Property::from(value))?;
    }
    define_methods(ctx, &proto)?;
    define_accessors(&proto)?;
    Ok(proto)
}

fn define_methods<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let method = |name: &str, function: Function<'js>| {
        proto.prop(name, Property::from(function).writable().configurable())
    };
    method(
        "stopPropagation",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<()> {
                let record = record_of(&ctx, &event)?;
                if flag(&record, "dispatching") {
                    record.set("stop", true)?;
                }
                Ok(())
            },
        )?,
    )?;
    method(
        "stopImmediatePropagation",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<()> {
                let record = record_of(&ctx, &event)?;
                if flag(&record, "dispatching") {
                    record.set("stop", true)?;
                    record.set("immediate", true)?;
                }
                Ok(())
            },
        )?,
    )?;
    method(
        "preventDefault",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<()> {
                let record = record_of(&ctx, &event)?;
                if cancels(&record) {
                    record.set("canceled", true)?;
                }
                Ok(())
            },
        )?,
    )?;
    method(
        "composedPath",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<Vec<NodeId>> {
                let record = record_of(&ctx, &event)?;
                if flag(&record, "dispatching") {
                    record.get("path")
                } else {
                    Ok(Vec::new())
                }
            },
        )?,
    )
}

fn define_accessors<'js>(proto: &Object<'js>) -> JsResult<()> {
    proto.prop(
        "srcElement",
        Accessor::new_get(
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<Value<'js>> {
                record_of(&ctx, &event)?;
                event.get("target")
            },
        ),
    )?;
    proto.prop(
        "defaultPrevented",
        Accessor::new_get(
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<bool> {
                Ok(flag(&record_of(&ctx, &event)?, "canceled"))
            },
        )
        .enumerable(),
    )?;
    proto.prop(
        "returnValue",
        Accessor::new_get(
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<bool> {
                Ok(!flag(&record_of(&ctx, &event)?, "canceled"))
            },
        )
        .set(
            |ctx: Ctx<'js>, This(event): This<Object<'js>>, value: Coerced<bool>| -> JsResult<()> {
                let record = record_of(&ctx, &event)?;
                if !value.0 && cancels(&record) {
                    record.set("canceled", true)?;
                }
                Ok(())
            },
        ),
    )?;
    proto.prop(
        "cancelBubble",
        Accessor::new_get(
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<bool> {
                Ok(flag(&record_of(&ctx, &event)?, "stop"))
            },
        )
        .set(
            |ctx: Ctx<'js>, This(event): This<Object<'js>>, value: Coerced<bool>| -> JsResult<()> {
                let record = record_of(&ctx, &event)?;
                if value.0 && flag(&record, "dispatching") {
                    record.set("stop", true)?;
                }
                Ok(())
            },
        ),
    )
}
