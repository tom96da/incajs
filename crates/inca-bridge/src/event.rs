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
//! `WeakMap` keeps, so no script can reach it.

use std::cell::RefCell;
use std::rc::Rc;

use inca_gpui::{EventPayload, KeyPayload, MousePayload, NodeId, PointerSource};
use inca_jsenv::{Engine, EngineError};
use rquickjs::convert::Coerced;
use rquickjs::function::This;
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
    let slots: Slots = vec![
        prototype(ctx)?.into_value(),
        remember.into_value(),
        recall.into_value(),
    ];
    ctx.store_userdata(slots).map(drop).map_err(|_| {
        rquickjs::Error::new_from_js_message(
            "events",
            "userdata",
            "the runtime's userdata is borrowed",
        )
    })
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

/// The record of `event`.
///
/// # Errors
///
/// Throws a `TypeError` when `event` is not an event object of this host.
fn record_of<'js>(ctx: &Ctx<'js>, event: &Object<'js>) -> JsResult<Object<'js>> {
    let recall = function_slot(ctx, RECALL)?;
    let record: Value = recall.call((event.clone(),))?;
    record
        .into_object()
        .ok_or_else(|| Exception::throw_type(ctx, "Illegal invocation"))
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
    pub time_stamp: f64,
}

/// What a walk asked of its caller.
#[derive(Default)]
pub(crate) struct Outcome {
    /// A listener called `preventDefault()` on a cancelable event.
    pub canceled: bool,
    /// The walk entered the engine, so jobs may be queued.
    pub entered: bool,
    /// Every error the listeners threw, in order. The caller reports them
    /// outside the engine.
    pub failures: Vec<EngineError>,
}

/// Runs the listeners of `walk.name` along `walk.path` with one event object.
/// Capture listeners run from the root down to the target, then bubble
/// listeners from the target up, the latter only while the event bubbles.
/// At the target, capture listeners run before bubble listeners. A node's
/// listeners are those registered when the walk reaches it, minus any removed
/// while an earlier listener ran. The engine is entered only when the event
/// reaches a listener.
pub(crate) fn walk(engine: &Engine, host: &Rc<RefCell<Host>>, walk: &Walk<'_>) -> Outcome {
    let bubble = if walk.flags.bubbles {
        walk.path
    } else {
        &walk.path[..walk.path.len().min(1)]
    };
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
    let listened = hears(true, walk.path) || hears(false, bubble);
    if !listened {
        return Outcome::default();
    }
    let id = {
        let mut host = host.borrow_mut();
        host.last_event_id += 1;
        host.last_event_id
    };
    let mut outcome = engine.with(|ctx| run(&ctx, host, walk, bubble, id));
    outcome.entered = true;
    outcome
}

fn run(
    ctx: &Ctx<'_>,
    host: &Rc<RefCell<Host>>,
    walk: &Walk<'_>,
    bubble: &[NodeId],
    id: u64,
) -> Outcome {
    let mut failures = Vec::new();
    let Ok(callbacks) = ctx.globals().get::<_, Object>("__inca_callbacks__") else {
        return Outcome::default();
    };
    let (event, record) = match create(ctx, walk, id) {
        Ok(pair) => pair,
        Err(err) => {
            failures.push(EngineError::capture(ctx, &err));
            return Outcome {
                failures,
                ..Outcome::default()
            };
        }
    };
    let down: Vec<NodeId> = walk.path.iter().rev().copied().collect();
    'phases: for (capture, nodes) in [(true, &down[..]), (false, bubble)] {
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
            note(ctx, &mut failures, entered);
            for listener in listeners {
                if flag(&record, "immediate") {
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
                note(ctx, &mut failures, armed);
                let called = callback.call::<_, ()>((event.clone(),));
                note(ctx, &mut failures, called);
            }
            if flag(&record, "stop") {
                break 'phases;
            }
        }
    }
    let canceled = flag(&record, "canceled");
    let left = event
        .set("currentTarget", Null)
        .and_then(|()| event.set("eventPhase", 0))
        .and_then(|()| record.set("dispatching", false))
        .and_then(|()| record.set("passive", false))
        .and_then(|()| record.set("stop", false))
        .and_then(|()| record.set("immediate", false));
    note(ctx, &mut failures, left);
    Outcome {
        canceled,
        entered: false,
        failures,
    }
}

/// Moves a thrown error of `result` into `failures`.
fn note(ctx: &Ctx<'_>, failures: &mut Vec<EngineError>, result: JsResult<()>) {
    if let Err(err) = result {
        failures.push(EngineError::capture(ctx, &err));
    }
}

/// A new event object and its record, filed in the runtime's record store.
fn create<'js>(ctx: &Ctx<'js>, walk: &Walk<'_>, id: u64) -> JsResult<(Object<'js>, Object<'js>)> {
    let Flags {
        bubbles,
        cancelable,
        composed,
    } = walk.flags;
    let proto = slot(ctx, PROTOTYPE)?
        .into_object()
        .ok_or_else(|| Exception::throw_type(ctx, "events are not installed"))?;
    let remember = function_slot(ctx, REMEMBER)?;

    let record = Object::new(ctx.clone())?;
    record.set_prototype(None)?;
    record.set("stop", false)?;
    record.set("immediate", false)?;
    record.set("canceled", false)?;
    record.set("passive", false)?;
    record.set("cancelable", cancelable)?;
    record.set("dispatching", true)?;
    record.set("path", walk.path.to_vec())?;

    let event = Object::new(ctx.clone())?;
    event.set_prototype(Some(&proto))?;
    event.set("type", walk.name)?;
    event.set("target", walk.target)?;
    event.set("currentTarget", Null)?;
    event.prop("srcElement", Property::from(walk.target))?;
    event.set("bubbles", bubbles)?;
    event.set("cancelable", cancelable)?;
    event.set("composed", composed)?;
    event.set("eventPhase", 0)?;
    event.set("isTrusted", true)?;
    event.set("timeStamp", walk.time_stamp)?;
    set_payload(&event, walk.payload)?;
    event.set("eventId", id)?;

    remember.call::<_, ()>((event.clone(), record.clone()))?;
    Ok((event, record))
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
