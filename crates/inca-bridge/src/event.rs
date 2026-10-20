// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event object a listener receives and the walk of one event over its
//! path.
//!
//! One event is one object of the native class [`IncaEvent`]. Its fields live
//! in Rust, and the getters and methods that read them sit on a prototype of
//! the event's kind, built once per realm. `isTrusted` is the one own property
//! of an event. The walk creates the event, runs the capture listeners from
//! the root down to the target and then the bubble listeners from the target
//! up, all with that same object, then resets its `currentTarget` and
//! `eventPhase`. The global constructors `Event` and `CustomEvent` build events
//! of the same class, so a script can build an event and dispatch it. The
//! prototypes sit in the realm's vault, and Rust keeps only their keys.
//!
//! The global `__inca_targets__` gives the objects that stand for the nodes of
//! a walk.

use std::cell::Cell;
use std::sync::OnceLock;
use std::time::Instant;

pub(crate) use inca_gpui::event_sink::Flags;
use inca_gpui::{EventPayload, NodeId};
use inca_jsenv::{FunctionAccessor, vault};
use rquickjs::function::This;
use rquickjs::object::Property;
use rquickjs::{Array, Class, Ctx, Exception, Function, IntoJs, Object, Result as JsResult, Value};

mod construct;
mod members;
mod payload;
mod record;
mod walk;

use record::{Held, IncaEvent, State};
pub(crate) use walk::{Walk, click, dispatch_event, listened, path_from, walk};

/// The vault key of the event store: the prototype of each kind of event
/// under the kind, and the `isTrusted` getter.
const STORE: &str = "events";
/// The store key of the getter every event owns as `isTrusted`.
const IS_TRUSTED: &str = "isTrusted";
/// The kind of scroll events and of events built with `new Event`. A kind is
/// the store key of its prototype, `Event.prototype` for this one.
const EVENT: &str = "event";
/// The kind of events built with `new CustomEvent`. Its prototype inherits
/// from the [`EVENT`] one.
const CUSTOM: &str = "custom";
/// The kind of focus events. Its prototype inherits from the [`EVENT`] one.
const FOCUS: &str = "focus";
/// The kind of mouse events with no pointer source. Its prototype inherits
/// from the [`EVENT`] one.
const MOUSE: &str = "mouse";
/// The kind of mouse events with a pointer source: `click`, `auxclick` and
/// `contextmenu`. Its prototype inherits from the [`MOUSE`] one.
const POINTER: &str = "pointer";
/// The kind of wheel events. Its prototype inherits from the [`MOUSE`] one.
const WHEEL: &str = "wheel";
/// The kind of key events. Its prototype inherits from the [`EVENT`] one.
const KEYBOARD: &str = "keyboard";

impl State {
    /// The state of an idle event.
    fn new(name: String, flags: Flags, payload: EventPayload, trusted: bool) -> Self {
        Self {
            name,
            flags,
            payload,
            time_stamp: time_stamp(),
            trusted: Cell::new(trusted),
            event_id: Cell::new(None),
            dispatching: Cell::new(false),
            stop: Cell::new(false),
            immediate: Cell::new(false),
            canceled: Cell::new(false),
            passive: Cell::new(false),
            phase: Cell::new(0),
            current: Cell::new(None),
        }
    }
}

impl<'js> IncaEvent<'js> {
    /// What stands for the node whose listeners run, `None` outside a walk.
    fn current_target(&self) -> Option<Value<'js>> {
        let index = self.state.current.get()?;
        self.held.path.borrow().get(index).cloned()
    }

    /// Whether a cancel request takes effect now: the event is cancelable and
    /// the running listener is active.
    fn cancels(&self) -> bool {
        self.state.flags.cancelable && !self.state.passive.get()
    }
}

/// Creates the prototypes of `ctx`'s realm in its vault and the global
/// constructors. The first call wins, and later calls in the same realm leave
/// it as it is.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let vault = vault(ctx)?;
    if vault.contains_key(STORE)? {
        return Ok(());
    }
    let store = Object::new(ctx.clone())?;
    store.set_prototype(None)?;
    let event = members::event_prototype(ctx)?;
    let mouse = payload::mouse_prototype(ctx, &event)?;
    store.set(POINTER, payload::pointer_prototype(ctx, &mouse)?)?;
    store.set(WHEEL, payload::wheel_prototype(ctx, &mouse)?)?;
    store.set(MOUSE, mouse)?;
    store.set(KEYBOARD, payload::keyboard_prototype(ctx, &event)?)?;
    store.set(FOCUS, payload::focus_prototype(ctx, &event)?)?;
    store.set(CUSTOM, construct::constructors(ctx, &event)?)?;
    store.set(EVENT, event)?;
    store.set(IS_TRUSTED, members::is_trusted(ctx)?)?;
    vault.set(STORE, store)
}

/// Milliseconds since the first call. The dispatcher makes the first call
/// when it starts, so the origin is the host start.
pub(crate) fn time_stamp() -> f64 {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// `event` as an object of the prototype of `kind`, with the shared
/// `isTrusted` getter as its own property.
fn new_event<'js>(
    ctx: &Ctx<'js>,
    kind: &str,
    event: IncaEvent<'js>,
) -> JsResult<Class<'js, IncaEvent<'js>>> {
    let store: Object = vault(ctx)?.get(STORE)?;
    let event = Class::instance_proto(event, store.get(kind)?)?;
    event.prop(
        IS_TRUSTED,
        FunctionAccessor::new_get(store.get(IS_TRUSTED)?).enumerable(),
    )?;
    Ok(event)
}

/// Runs `run` on the event `this` names.
///
/// # Errors
///
/// Throws a `TypeError` when `this` is not an event, and passes on what `run`
/// returns.
fn with_event<'js, T>(
    ctx: &Ctx<'js>,
    this: &Value<'js>,
    run: impl FnOnce(&IncaEvent<'js>) -> JsResult<T>,
) -> JsResult<T> {
    let event = Class::<IncaEvent>::from_value(this).map_err(|_| {
        Exception::throw_type(
            ctx,
            concat!(
                "not an event object, so call the method on an Event ",
                "such as the one a listener receives"
            ),
        )
    })?;
    run(&event.borrow())
}

/// The getter `get <key>`, which runs `read` on the event `this` names.
fn getter<'js>(
    ctx: &Ctx<'js>,
    key: &str,
    read: impl Fn(&Ctx<'js>, &IncaEvent<'js>) -> JsResult<Value<'js>> + 'js,
) -> JsResult<Function<'js>> {
    Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
            with_event(&ctx, &this, |event| read(&ctx, event))
        },
    )?
    .with_name(format!("get {key}"))
}

/// Defines on `proto` the enumerable, configurable getter `key`, which runs
/// `read` on the event `this` names.
fn field<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    key: &str,
    read: impl Fn(&Ctx<'js>, &IncaEvent<'js>) -> JsResult<Value<'js>> + 'js,
) -> JsResult<()> {
    let get = getter(ctx, key, read)?;
    proto.prop(
        key,
        FunctionAccessor::new_get(get).enumerable().configurable(),
    )
}

/// Defines `function` as the writable, enumerable, configurable method `name`
/// of `proto`.
fn method<'js>(proto: &Object<'js>, name: &str, function: Function<'js>) -> JsResult<()> {
    proto.prop(
        name,
        Property::from(function.with_name(name)?)
            .writable()
            .enumerable()
            .configurable(),
    )
}

/// Defines on `proto` the method `name`, which runs `act` on the event `this`
/// names.
fn action<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    act: fn(&IncaEvent<'_>),
) -> JsResult<()> {
    let function = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
            with_event(&ctx, &this, |event| {
                act(event);
                Ok(())
            })
        },
    )?;
    method(proto, name, function)
}

/// A new object that inherits from `parent`.
fn child_of<'js>(ctx: &Ctx<'js>, parent: &Object<'js>) -> JsResult<Object<'js>> {
    let child = Object::new(ctx.clone())?;
    child.set_prototype(Some(parent))?;
    Ok(child)
}

/// The `TypeError` that `member` of one kind of event throws for an event of
/// another kind.
fn other_kind(ctx: &Ctx<'_>, member: &str) -> rquickjs::Error {
    Exception::throw_type(
        ctx,
        &format!("{member} belongs to another kind of event, so use it on an event of that kind"),
    )
}

/// What stands for each of `ids`: the entries of the array that the global
/// `__inca_targets__` returns for them, or the ids themselves when the global
/// is not a function.
///
/// # Errors
///
/// Throws what the global throws, and a `TypeError` when it returns no array.
fn targets_of<'js>(ctx: &Ctx<'js>, ids: &[NodeId]) -> JsResult<Vec<Value<'js>>> {
    let Ok(hook) = ctx.globals().get::<_, Function>("__inca_targets__") else {
        return ids.iter().map(|id| id.into_js(ctx)).collect();
    };
    let found: Array = hook.call((array_of(ctx, ids.iter().copied())?,))?;
    (0..ids.len()).map(|index| found.get(index)).collect()
}

/// A new array of `items`. Each entry is defined, so no index setter on
/// `Array.prototype` or `Object.prototype` runs.
pub(crate) fn array_of<'js, T: IntoJs<'js>>(
    ctx: &Ctx<'js>,
    items: impl IntoIterator<Item = T>,
) -> JsResult<Array<'js>> {
    let array = Array::new(ctx.clone())?;
    for (index, item) in (0u32..).zip(items) {
        let value = item.into_js(ctx)?;
        array.as_object().prop(
            index,
            Property::from(value).writable().enumerable().configurable(),
        )?;
    }
    Ok(array)
}
