// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use inca_gpui::{EventPayload, MousePayload, NodeId};
use inca_jsenv::{Engine, EngineError};
use rquickjs::{Ctx, Exception, Function, Null, Object, Result as JsResult, Value};

use super::construct::create;
use super::{Flags, find_record, flag};
use crate::bindings::{Host, Listener};

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
pub(crate) fn path_from(host: &Host, target: NodeId) -> Vec<NodeId> {
    std::iter::successors(Some(target), |&id| {
        host.tree.get(id).and_then(inca_gpui::VirtualNode::parent)
    })
    .collect()
}

/// The nodes whose bubble listeners the event reaches.
fn bubble_nodes(flags: Flags, path: &[NodeId]) -> &[NodeId] {
    if flags.bubbles {
        path
    } else {
        &path[..path.len().min(1)]
    }
}

/// Whether a listener of `name` exists that an event with `flags` on `path`
/// reaches. Nodes in `cut` stay silent.
pub(crate) fn listened(
    host: &Host,
    name: &str,
    flags: Flags,
    path: &[NodeId],
    cut: &[NodeId],
) -> bool {
    let hears = |capture: bool, nodes: &[NodeId]| {
        nodes.iter().any(|node| {
            !cut.contains(node)
                && host
                    .listeners
                    .listeners_for(*node, name)
                    .iter()
                    .any(|l| l.capture == capture)
        })
    };
    hears(true, path) || hears(false, bubble_nodes(flags, path))
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
    if !listened(&host.borrow(), walk.name, walk.flags, walk.path, walk.cut) {
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
    let bubble = bubble_nodes(walk.flags, walk.path);
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
            note(ctx, host, entered);
            for listener in listeners {
                if flag(record, "immediate") {
                    break;
                }
                // The full value is compared. Flags change only through unregister, then register.
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
/// reports listener errors in the order they happened.
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
    let path = path_from(&host.borrow(), target);
    let flags = Flags {
        bubbles: flag(&record, "bubbles"),
        cancelable: flag(&record, "cancelable"),
        composed: flag(&record, "composed"),
    };
    let walk = script_walk(&name, flags, target, &path, &EventPayload::None);
    event.set("target", target)?;
    event.set("isTrusted", false)?;
    if listened(&host.borrow(), walk.name, walk.flags, walk.path, walk.cut) {
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
    let path = path_from(&host.borrow(), target);
    let payload = EventPayload::Mouse(MousePayload::keyboard_click(gpui::Modifiers::none()));
    let walk = script_walk(
        "click",
        crate::dispatch::spec_of("click").flags,
        target,
        &path,
        &payload,
    );
    if listened(&host.borrow(), walk.name, walk.flags, walk.path, walk.cut) {
        run(ctx, host, &walk, next_event_id(host));
    }
    host.borrow_mut().clicking.remove(&target);
}
