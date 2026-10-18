// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event object a listener receives and the walk of one event over its
//! path.
//!
//! One event is one object. The walk creates it, runs the capture listeners
//! from the root down to the target and then the bubble listeners from the
//! target up, all with that same object, then resets its `currentTarget` and
//! `eventPhase`. The object's methods and accessors live on a prototype built
//! once per realm. What they read and write sits in a record that a `WeakMap`
//! keeps, so no script can reach it. The prototypes and the two record
//! functions sit in the realm's vault, and Rust keeps only their keys. Mouse
//! and key events inherit from a prototype of their kind, which adds
//! `getModifierState`. The global constructors `Event` and `CustomEvent` file
//! their events in the same store, so a script can build an event and
//! dispatch it.
//!
//! The global `__inca_targets__` gives the objects that stand for the nodes of
//! a walk.

use std::sync::OnceLock;
use std::time::Instant;

use inca_gpui::NodeId;
pub(crate) use inca_gpui::event_sink::Flags;
use inca_jsenv::vault;
use rquickjs::atom::PredefinedAtom;
use rquickjs::convert::Coerced;
use rquickjs::function::This;
use rquickjs::object::Property;
use rquickjs::{
    Array, Ctx, Exception, FromJs, Function, IntoJs, Object, Result as JsResult, Value,
};

mod construct;
mod payload;
mod walk;

use construct::constructors;
pub(crate) use walk::{Walk, click, dispatch_event, listened, path_from, walk};

/// The vault key of the event store: the three prototypes under the kind of
/// event they serve, and the two record functions.
const STORE: &str = "events";
/// The kind of events that inherit from `Event.prototype` directly. A kind is
/// the store key of its prototype and the `kind` of its records.
const EVENT: &str = "event";
/// The kind of mouse and wheel events. Their prototype inherits from the
/// [`EVENT`] one.
const MOUSE: &str = "mouse";
/// The kind of key events. Their prototype inherits from the [`EVENT`] one.
const KEYBOARD: &str = "keyboard";
/// `(event, record)`: files the record of a new event.
const REMEMBER: &str = "remember";
/// `(event)`: the record of an event, `undefined` for any other value.
const RECALL: &str = "recall";

/// Creates the prototypes and the record store of `ctx`'s realm in its vault.
/// The first call wins, and later calls in the same realm leave it as it is.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let vault = vault(ctx)?;
    if vault.contains_key(STORE)? {
        return Ok(());
    }
    let store: Object = ctx.eval(
        r"
        (() => {
            const records = new WeakMap();
            const { get, set } = WeakMap.prototype;
            const { apply } = Reflect;
            return {
                __proto__: null,
                remember: (event, record) => {
                    apply(set, records, [event, record]);
                },
                recall: (event) => apply(get, records, [event]),
            };
        })()
        ",
    )?;
    let proto = prototype(ctx)?;
    store.set(MOUSE, modifier_prototype(ctx, &proto, MOUSE)?)?;
    store.set(KEYBOARD, modifier_prototype(ctx, &proto, KEYBOARD)?)?;
    store.set(EVENT, proto.clone())?;
    vault.set(STORE, store)?;
    constructors(ctx, &proto)
}

/// Milliseconds since the first call. The dispatcher makes the first call
/// when it starts, so the origin is the host start.
pub(crate) fn time_stamp() -> f64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// The objects that the global `__inca_targets__` gives for `ids`, entry `i`
/// standing for `ids[i]`. `None` when the global is not a function, and the
/// event fields then carry the node ids.
fn targets_of<'js>(ctx: &Ctx<'js>, ids: &[NodeId]) -> JsResult<Option<Array<'js>>> {
    let Ok(hook) = ctx.globals().get::<_, Function>("__inca_targets__") else {
        return Ok(None);
    };
    hook.call((ids,)).map(Some)
}

/// What stands for `path[index]`: the entry of `targets`, or `node` itself.
fn target_at<'js>(
    ctx: &Ctx<'js>,
    targets: Option<&Array<'js>>,
    index: usize,
    node: NodeId,
) -> JsResult<Value<'js>> {
    match targets {
        Some(targets) => targets.get(index),
        None => node.into_js(ctx),
    }
}

/// What stands for the whole of `path`: `targets`, or the node ids.
fn path_value<'js>(
    ctx: &Ctx<'js>,
    targets: Option<&Array<'js>>,
    path: &[NodeId],
) -> JsResult<Value<'js>> {
    match targets {
        Some(targets) => Ok(targets.clone().into_value()),
        None => path.into_js(ctx),
    }
}

/// The entry `key` of the event store.
fn piece<'js, T: FromJs<'js>>(ctx: &Ctx<'js>, key: &str) -> JsResult<T> {
    vault(ctx)?.get::<_, Object>(STORE)?.get(key)
}

/// The record of `event`, `None` for any object the host did not file.
fn find_record<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Option<Object<'js>>> {
    let recall: Function = piece(ctx, RECALL)?;
    let record: Value = recall.call((event.clone(),))?;
    Ok(record.into_object())
}

/// The record of `event`.
///
/// # Errors
///
/// Throws a `TypeError` when `event` is not an event object of this host.
fn record_of<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Object<'js>> {
    find_record(ctx, event)?.ok_or_else(|| {
        Exception::throw_type(
            ctx,
            concat!(
                "not an event object, so call the method on an Event ",
                "such as the one a listener receives"
            ),
        )
    })
}

fn flag(record: &Object<'_>, key: &str) -> bool {
    record.get::<_, bool>(key).unwrap_or(false)
}

/// Whether a cancel request takes effect now: the event is cancelable and the
/// running listener is active.
fn cancels(record: &Object<'_>) -> bool {
    flag(record, "cancelable") && !flag(record, "passive")
}

/// The phase constants on `Event`, its prototype and `CustomEvent`.
const PHASES: [(&str, i32); 4] = [
    ("NONE", 0),
    ("CAPTURING_PHASE", 1),
    ("AT_TARGET", 2),
    ("BUBBLING_PHASE", 3),
];

/// The prototype of event objects: the phase constants, the propagation
/// methods, `composedPath`, the accessors and the string tag `Event`.
fn prototype<'js>(ctx: &Ctx<'js>) -> JsResult<Object<'js>> {
    let proto = Object::new(ctx.clone())?;
    for (name, value) in PHASES {
        proto.prop(name, Property::from(value).enumerable())?;
    }
    define_methods(ctx, &proto)?;
    define_accessors(ctx, &proto)?;
    proto.prop(
        PredefinedAtom::SymbolToStringTag,
        Property::from("Event").configurable(),
    )?;
    Ok(proto)
}

fn define_methods<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let method = |name: &str, function: Function<'js>| {
        proto.prop(
            name,
            Property::from(function.with_name(name)?)
                .writable()
                .enumerable()
                .configurable(),
        )
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
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<Vec<Value<'js>>> {
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

/// A prototype that inherits from `base` and holds `getModifierState`. The method reads only an
/// event whose record holds `kind` as its `kind`, and throws a `TypeError` for any other
/// receiver. Accel reads Meta on macOS and Control elsewhere.
fn modifier_prototype<'js>(
    ctx: &Ctx<'js>,
    base: &Object<'js>,
    kind: &'static str,
) -> JsResult<Object<'js>> {
    let proto = Object::new(ctx.clone())?;
    proto.set_prototype(Some(base))?;
    let get_modifier_state = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>,
              This(event): This<Object<'js>>,
              name: Coerced<String>|
              -> JsResult<bool> {
            let record = record_of(&ctx, &event)?;
            if record.get::<_, String>("kind")? != kind {
                return Err(Exception::throw_type(
                    &ctx,
                    concat!(
                        "getModifierState belongs to another kind of event, ",
                        "so call it on the event as event.getModifierState(key)"
                    ),
                ));
            }
            let name = match name.0.as_str() {
                "Accel" if cfg!(target_os = "macos") => "Meta",
                "Accel" => "Control",
                other => other,
            };
            let field = match name {
                "Control" => "ctrlKey",
                "Shift" => "shiftKey",
                "Alt" => "altKey",
                "Meta" => "metaKey",
                _ => return Ok(false),
            };
            Ok(flag(&event, field))
        },
    )?;
    proto.prop(
        "getModifierState",
        Property::from(get_modifier_state.with_name("getModifierState")?)
            .writable()
            .enumerable()
            .configurable(),
    )?;
    Ok(proto)
}

fn define_accessors<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let define_property: Function = ctx
        .globals()
        .get::<_, Object>("Object")?
        .get("defineProperty")?;
    let accessor = |key: &str, get: Function<'js>, set: Option<Function<'js>>| {
        define_accessor(&define_property, proto, key, get, set)
    };
    accessor(
        "srcElement",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<Value<'js>> {
                record_of(&ctx, &event)?;
                event.get("target")
            },
        )?,
        None,
    )?;
    accessor(
        "defaultPrevented",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<bool> {
                Ok(flag(&record_of(&ctx, &event)?, "canceled"))
            },
        )?,
        None,
    )?;
    accessor(
        "returnValue",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<bool> {
                Ok(!flag(&record_of(&ctx, &event)?, "canceled"))
            },
        )?,
        Some(Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>, value: Coerced<bool>| -> JsResult<()> {
                let record = record_of(&ctx, &event)?;
                if !value.0 && cancels(&record) {
                    record.set("canceled", true)?;
                }
                Ok(())
            },
        )?),
    )?;
    accessor(
        "cancelBubble",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>| -> JsResult<bool> {
                Ok(flag(&record_of(&ctx, &event)?, "stop"))
            },
        )?,
        Some(Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(event): This<Object<'js>>, value: Coerced<bool>| -> JsResult<()> {
                let record = record_of(&ctx, &event)?;
                if value.0 && flag(&record, "dispatching") {
                    record.set("stop", true)?;
                }
                Ok(())
            },
        )?),
    )
}

/// Defines the enumerable, configurable accessor `key` on `proto` through
/// `define_property`, the realm's `Object.defineProperty`. The getter is named
/// `get <key>` and the setter `set <key>`.
fn define_accessor<'js>(
    define_property: &Function<'js>,
    proto: &Object<'js>,
    key: &str,
    get: Function<'js>,
    set: Option<Function<'js>>,
) -> JsResult<()> {
    // rquickjs's `Accessor` takes only closures, so it cannot carry a named function.
    let descriptor = Object::new(proto.ctx().clone())?;
    descriptor.set_prototype(None)?;
    descriptor.set("get", get.with_name(format!("get {key}"))?)?;
    if let Some(set) = set {
        descriptor.set("set", set.with_name(format!("set {key}"))?)?;
    }
    descriptor.set("enumerable", true)?;
    descriptor.set("configurable", true)?;
    define_property.call((proto.clone(), key, descriptor))
}
