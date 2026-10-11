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

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;

use inca_gpui::{EventPayload, KeyPayload, MousePayload, NodeId, PointerSource};
use inca_jsenv::{Engine, EngineError};
use rquickjs::convert::Coerced;
use rquickjs::function::{Constructor, IntoJsFunc, ParamRequirement, Params, This};
use rquickjs::object::{Accessor, Property};
use rquickjs::{Ctx, Exception, Function, Null, Object, Result as JsResult, Value};

use crate::bindings::{Host, Listener};

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

/// What one walk needs to know.
pub(crate) struct Walk<'a> {
    pub name: &'a str,
    pub flags: Flags,
    pub target: NodeId,
    /// `target` and its ancestors, nearest first.
    pub path: &'a [NodeId],
    /// The nodes whose listeners do not run.
    pub cut: &'a [NodeId],
    pub payload: &'a EventPayload,
    /// The value of `isTrusted` on the event the walk creates.
    pub trusted: bool,
}

/// What a walk asked of its caller.
#[derive(Default)]
pub(crate) struct Outcome {
    /// A listener called `preventDefault()` on a cancelable event.
    pub canceled: bool,
    /// The walk entered the engine, so jobs may be queued.
    pub entered: bool,
}

/// `target` and its ancestors, nearest first.
pub(crate) fn path_from(host: &Rc<RefCell<Host>>, target: NodeId) -> Vec<NodeId> {
    let host = host.borrow();
    std::iter::successors(Some(target), |&id| {
        host.tree.get(id).and_then(inca_gpui::VirtualNode::parent)
    })
    .collect()
}

/// The nodes whose bubble listeners the event reaches.
fn bubble_nodes<'a>(walk: &Walk<'a>) -> &'a [NodeId] {
    if walk.flags.bubbles {
        walk.path
    } else {
        &walk.path[..walk.path.len().min(1)]
    }
}

/// Whether a listener the event reaches exists.
fn listened(host: &Rc<RefCell<Host>>, walk: &Walk<'_>) -> bool {
    let hears = |capture: bool, nodes: &[NodeId]| {
        let host = host.borrow();
        nodes.iter().any(|node| {
            !walk.cut.contains(node)
                && host
                    .listeners
                    .listeners_for(*node, walk.name)
                    .iter()
                    .any(|l| l.capture == capture)
        })
    };
    hears(true, walk.path) || hears(false, bubble_nodes(walk))
}

fn next_event_id(host: &Rc<RefCell<Host>>) -> u64 {
    let mut host = host.borrow_mut();
    host.last_event_id += 1;
    host.last_event_id
}

/// Runs the listeners of `walk.name` along `walk.path` with one event object.
/// Capture listeners run from the root down to the target, then bubble
/// listeners from the target up, the latter only while the event bubbles.
/// At the target, capture listeners run before bubble listeners. A node's
/// listeners are those registered when the walk reaches it, minus any removed
/// while an earlier listener ran. The engine is entered only when the event
/// reaches a listener.
pub(crate) fn walk(engine: &Engine, host: &Rc<RefCell<Host>>, walk: &Walk<'_>) -> Outcome {
    if !listened(host, walk) {
        return Outcome::default();
    }
    let id = next_event_id(host);
    let mut outcome = engine.with(|ctx| run(&ctx, host, walk, id));
    outcome.entered = true;
    outcome
}

/// A walk of an event a script starts: no node is cut and the event is
/// untrusted.
fn script_walk<'a>(
    name: &'a str,
    flags: Flags,
    target: NodeId,
    path: &'a [NodeId],
    payload: &'a EventPayload,
) -> Walk<'a> {
    Walk {
        name,
        flags,
        target,
        path,
        cut: &[],
        payload,
        trusted: false,
    }
}

fn run(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, walk: &Walk<'_>, id: u64) -> Outcome {
    let Ok(callbacks) = ctx.globals().get::<_, Object>("__inca_callbacks__") else {
        return Outcome::default();
    };
    let (event, record) = match create(ctx, walk, id) {
        Ok(pair) => pair,
        Err(err) => {
            note(ctx, host, Err(err));
            return Outcome::default();
        }
    };
    let canceled = deliver(ctx, host, walk, &callbacks, &event, &record);
    Outcome {
        canceled,
        entered: false,
    }
}

/// Calls the listeners along the path with `event`, then leaves the event
/// idle. Returns whether it ended canceled.
fn deliver<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    walk: &Walk<'_>,
    callbacks: &Object<'js>,
    event: &Object<'js>,
    record: &Object<'js>,
) -> bool {
    let down: Vec<NodeId> = walk.path.iter().rev().copied().collect();
    'phases: for (capture, nodes) in [(true, &down[..]), (false, bubble_nodes(walk))] {
        for &node in nodes {
            if walk.cut.contains(&node) {
                continue;
            }
            let listeners: Vec<Listener> = host
                .borrow()
                .listeners
                .listeners_for(node, walk.name)
                .iter()
                .filter(|l| l.capture == capture)
                .copied()
                .collect();
            if listeners.is_empty() {
                continue;
            }
            let phase = match (node == walk.target, capture) {
                (true, _) => 2,
                (false, true) => 1,
                (false, false) => 3,
            };
            let entered = event
                .set("currentTarget", node)
                .and_then(|()| event.set("eventPhase", phase));
            note(ctx, host, entered);
            for listener in listeners {
                if flag(record, "immediate") {
                    break;
                }
                // The full value is compared: a re-registration with other flags replaces the entry.
                if !host
                    .borrow()
                    .listeners
                    .listeners_for(node, walk.name)
                    .contains(&listener)
                {
                    continue;
                }
                let Ok(callback) = callbacks.get::<_, Function>(listener.callback) else {
                    continue;
                };
                let armed = record.set("passive", listener.passive);
                note(ctx, host, armed);
                let called = callback.call::<_, ()>((event.clone(),));
                note(ctx, host, called);
            }
            if flag(record, "stop") {
                break 'phases;
            }
        }
    }
    let canceled = flag(record, "canceled");
    // The record is ours and cannot fail; clearing it first keeps the guard
    // free when a listener froze the event.
    for key in ["dispatching", "passive", "stop", "immediate"] {
        let _ = record.set(key, false);
    }
    let left = event
        .set("currentTarget", Null)
        .and_then(|()| event.set("eventPhase", 0));
    note(ctx, host, left);
    canceled
}

/// Files a thrown error of `result` in `Host::failures`, where the dispatcher
/// reports every error in the order it happened.
fn note(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, result: JsResult<()>) {
    if let Err(err) = result {
        host.borrow_mut()
            .failures
            .push(EngineError::capture(ctx, &err));
    }
}

/// Dispatches a script-built `event` at `target` and returns `false` when a
/// listener canceled it. Listener errors go to `Host::failures`.
///
/// # Errors
///
/// Throws a `TypeError` when `event` is not an event object of this host, and
/// an `InvalidStateError` while `event` is being dispatched.
pub(crate) fn dispatch_event<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    target: NodeId,
    event: Value<'js>,
) -> JsResult<bool> {
    let not_event = || {
        Exception::throw_type(
            ctx,
            "Failed to execute 'dispatchEvent': parameter 1 is not of type 'Event'.",
        )
    };
    let event = event.into_object().ok_or_else(not_event)?;
    let record = find_record(ctx, &event)?.ok_or_else(not_event)?;
    if flag(&record, "dispatching") {
        return Err(Exception::throw_dom(
            ctx,
            "InvalidStateError",
            "Failed to execute 'dispatchEvent': The event is already being dispatched.",
        ));
    }
    let name: String = record.get("type")?;
    let path = path_from(host, target);
    let flags = Flags {
        bubbles: flag(&record, "bubbles"),
        cancelable: flag(&record, "cancelable"),
        composed: flag(&record, "composed"),
    };
    let walk = script_walk(&name, flags, target, &path, &EventPayload::None);
    event.set("target", target)?;
    event.set("isTrusted", false)?;
    if listened(host, &walk) {
        let callbacks = ctx
            .globals()
            .get::<_, Object>("__inca_callbacks__")
            .or_else(|_| Object::new(ctx.clone()))?;
        event.set("eventId", next_event_id(host))?;
        record.set("path", path.clone())?;
        // Last, so a failure above leaves the event free to dispatch again.
        record.set("dispatching", true)?;
        deliver(ctx, host, &walk, &callbacks, &event, &record);
    }
    Ok(!flag(&record, "canceled"))
}

/// Runs an untrusted `click` at `target`. A disabled button, a node that left the tree and a node whose own click is
/// still running ignore the call.
pub(crate) fn click(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, target: NodeId) {
    {
        let mut host = host.borrow_mut();
        let Some(node) = host.tree.get(target) else {
            return;
        };
        let disabled = node.tag_name() == "button" && node.attributes().contains_key("disabled");
        if disabled || !host.clicking.insert(target) {
            return;
        }
    }
    let path = path_from(host, target);
    let payload = EventPayload::Mouse(MousePayload::keyboard_click(gpui::Modifiers::none()));
    let walk = script_walk(
        "click",
        crate::dispatch::flags_of("click"),
        target,
        &path,
        &payload,
    );
    if listened(host, &walk) {
        run(ctx, host, &walk, next_event_id(host));
    }
    host.borrow_mut().clicking.remove(&target);
}

/// A new idle event object and its record, filed in the runtime's record
/// store.
fn new_event<'js>(
    ctx: &Ctx<'js>,
    kind: &str,
    flags: Flags,
    trusted: bool,
) -> JsResult<(Object<'js>, Object<'js>)> {
    let Flags {
        bubbles,
        cancelable,
        composed,
    } = flags;
    let proto = slot(ctx, PROTOTYPE)?
        .into_object()
        .ok_or_else(|| Exception::throw_type(ctx, "events are not installed"))?;
    let remember = function_slot(ctx, REMEMBER)?;

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

/// A new event object for a walk, ready to deliver.
fn create<'js>(ctx: &Ctx<'js>, walk: &Walk<'_>, id: u64) -> JsResult<(Object<'js>, Object<'js>)> {
    let (event, record) = new_event(ctx, walk.name, walk.flags, walk.trusted)?;
    record.set("dispatching", true)?;
    record.set("path", walk.path.to_vec())?;
    event.set("target", walk.target)?;
    set_payload(&event, walk.payload)?;
    event.set("eventId", id)?;
    Ok((event, record))
}

/// Installs the global constructors `Event` and `CustomEvent`.
fn constructors<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let event = Constructor::new_prototype(
        ctx,
        proto.clone(),
        Construct {
            name: "Event",
            custom: false,
        },
    )?;
    event.set_name("Event")?;
    let custom_proto = Object::new(ctx.clone())?;
    custom_proto.set_prototype(Some(proto))?;
    let custom = Constructor::new_prototype(
        ctx,
        custom_proto,
        Construct {
            name: "CustomEvent",
            custom: true,
        },
    )?;
    custom.set_name("CustomEvent")?;
    custom.set_prototype(Some(&event))?;
    for (name, value) in [
        ("NONE", 0),
        ("CAPTURING_PHASE", 1),
        ("AT_TARGET", 2),
        ("BUBBLING_PHASE", 3),
    ] {
        event.prop(name, Property::from(value))?;
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
            Exception::throw_type(ctx, &format!("Failed to construct '{name}': {message}"))
        };
        if !params.is_constructor() {
            return Err(type_error(
                "Please use the 'new' operator, this constructor cannot be called as a function.",
            ));
        }
        let Some(kind) = params.arg(0) else {
            return Err(type_error("1 argument required, but only 0 present."));
        };
        let kind = kind.get::<Coerced<String>>()?.0;
        let init = match params.arg(1) {
            Some(value) if !value.is_undefined() && !value.is_null() => Some(
                value
                    .into_object()
                    .ok_or_else(|| type_error("The provided value is not of type 'EventInit'."))?,
            ),
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
        let (event, _) = new_event(ctx, &kind, flags, false)?;
        if self.custom {
            let detail = init
                .as_ref()
                .map(|init| init.get::<_, Value>("detail"))
                .transpose()?
                .filter(|detail| !detail.is_undefined());
            match detail {
                Some(detail) => event.set("detail", detail)?,
                None => event.set("detail", Null)?,
            }
        }
        Ok(event.into_value())
    }
}

/// Writes `payload`'s fields onto `event_object`, DOM-named.
fn set_payload(event_object: &Object, payload: &EventPayload) -> JsResult<()> {
    match payload {
        EventPayload::None => {}
        EventPayload::Mouse(mouse) => set_mouse_fields(event_object, mouse)?,
        EventPayload::Wheel(wheel) => {
            set_mouse_fields(event_object, &wheel.mouse)?;
            event_object.set("deltaX", wheel.delta_x)?;
            event_object.set("deltaY", wheel.delta_y)?;
            event_object.set("deltaZ", wheel.delta_z)?;
            event_object.set("deltaMode", wheel.delta_mode)?;
        }
        EventPayload::Key(key) => set_key_fields(event_object, key)?,
        EventPayload::Focus { related_target } => match related_target {
            Some(id) => event_object.set("relatedTarget", *id)?,
            None => event_object.set("relatedTarget", Null)?,
        },
    }
    Ok(())
}

/// Writes [`MousePayload`]'s fields onto `event_object`, DOM-named — shared
/// by [`EventPayload::Mouse`] and [`EventPayload::Wheel`] (DOM's
/// `WheelEvent` extends `MouseEvent`).
fn set_mouse_fields(event_object: &Object, mouse: &MousePayload) -> JsResult<()> {
    event_object.set("clientX", mouse.client_x)?;
    event_object.set("clientY", mouse.client_y)?;
    event_object.set("screenX", mouse.screen_x)?;
    event_object.set("screenY", mouse.screen_y)?;
    event_object.set("x", mouse.client_x)?;
    event_object.set("y", mouse.client_y)?;
    match mouse.related_target {
        Some(id) => event_object.set("relatedTarget", id)?,
        None => event_object.set("relatedTarget", Null)?,
    }
    // Identical to clientX/clientY today — nothing here scrolls the page
    // itself, which is the only thing that would tell them apart.
    event_object.set("pageX", mouse.client_x)?;
    event_object.set("pageY", mouse.client_y)?;
    event_object.set("movementX", mouse.movement_x)?;
    event_object.set("movementY", mouse.movement_y)?;
    event_object.set("button", mouse.button)?;
    event_object.set("buttons", mouse.buttons)?;
    event_object.set("detail", mouse.detail)?;
    if let Some(source) = mouse.pointer {
        let (id, kind, primary) = match source {
            PointerSource::Mouse => (1, "mouse", true),
            PointerSource::Keyboard => (-1, "", false),
        };
        event_object.set("pointerId", id)?;
        event_object.set("pointerType", kind)?;
        event_object.set("isPrimary", primary)?;
        event_object.set("width", 1)?;
        event_object.set("height", 1)?;
        // Pointer events report 0.5 while a button is held.
        event_object.set("pressure", if mouse.buttons == 0 { 0.0 } else { 0.5 })?;
    }
    set_modifier_fields(event_object, mouse.modifiers)
}

/// Writes [`KeyPayload`]'s fields onto `event_object`, DOM-named.
fn set_key_fields(event_object: &Object, key: &KeyPayload) -> JsResult<()> {
    event_object.set("key", key.key.clone())?;
    event_object.set("repeat", key.repeat)?;
    // gpui reports no key location or composition state.
    event_object.set("location", 0)?;
    event_object.set("isComposing", false)?;
    set_modifier_fields(event_object, key.modifiers)
}

/// Writes `modifiers`' fields onto `event_object`, DOM-named — shared by
/// [`set_mouse_fields`] and [`set_key_fields`].
fn set_modifier_fields(event_object: &Object, modifiers: gpui::Modifiers) -> JsResult<()> {
    event_object.set("ctrlKey", modifiers.control)?;
    event_object.set("shiftKey", modifiers.shift)?;
    event_object.set("altKey", modifiers.alt)?;
    event_object.set("metaKey", modifiers.platform)?;
    event_object.set(
        "getModifierState",
        Function::new(event_object.ctx().clone(), move |name: String| {
            modifier_state(modifiers, &name)
        })?,
    )?;
    Ok(())
}

/// Whether the modifier `name` is held. `Accel` is Meta on macOS and Control
/// elsewhere. Other names return false.
fn modifier_state(modifiers: gpui::Modifiers, name: &str) -> bool {
    match name {
        "Control" => modifiers.control,
        "Shift" => modifiers.shift,
        "Alt" => modifiers.alt,
        "Meta" => modifiers.platform,
        "Accel" => {
            if cfg!(target_os = "macos") {
                modifiers.platform
            } else {
                modifiers.control
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_state_reports_each_held_modifier_and_no_lock() {
        let mut mismatches = Vec::new();
        for (name, held) in [
            ("Control", gpui::Modifiers::control()),
            ("Shift", gpui::Modifiers::shift()),
            ("Alt", gpui::Modifiers::alt()),
            ("Meta", gpui::Modifiers::command()),
        ] {
            for other in ["Control", "Shift", "Alt", "Meta", "CapsLock", "NumLock"] {
                if modifier_state(held, other) != (other == name) {
                    mismatches.push(format!("{name} held, {other} queried"));
                }
            }
        }
        let accel = if cfg!(target_os = "macos") {
            gpui::Modifiers::command()
        } else {
            gpui::Modifiers::control()
        };
        if !modifier_state(accel, "Accel") || modifier_state(gpui::Modifiers::default(), "Accel") {
            mismatches.push("Accel".to_string());
        }
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }
}
