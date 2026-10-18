// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use inca_gpui::{EventPayload, MousePayload, NodeId};
use inca_jsenv::{Engine, EngineError};
use rquickjs::{Array, Ctx, Exception, Function, Null, Object, Result as JsResult, Value};

use super::construct::create;
use super::{Flags, find_record, flag, path_value, target_at, targets_of};
use crate::bindings::{Host, Listener};

/// What one walk needs to know.
pub(crate) struct Walk<'a> {
    pub(crate) name: &'a str,
    pub(crate) flags: Flags,
    pub(crate) target: NodeId,
    /// `target` and its ancestors, nearest first.
    pub(crate) path: &'a [NodeId],
    /// The nodes whose listeners do not run.
    pub(crate) cut: &'a [NodeId],
    pub(crate) payload: &'a EventPayload,
    /// The value of `isTrusted` on the event the walk creates.
    pub(crate) trusted: bool,
}

/// What a walk asked of its caller.
#[derive(Default)]
pub(crate) struct Outcome {
    /// A listener called `preventDefault()` on a cancelable event.
    pub(crate) canceled: bool,
    /// The walk entered the engine, so jobs may be queued.
    pub(crate) entered: bool,
}

/// `target` and its ancestors, nearest first.
pub(crate) fn path_from(host: &Host, target: NodeId) -> Vec<NodeId> {
    host.tree.ancestors(target).collect()
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
    let created = targets_of(ctx, walk.path).and_then(|targets| {
        let (event, record) = create(ctx, walk, id, targets.as_ref())?;
        Ok((event, record, targets))
    });
    let (event, record, targets) = match created {
        Ok(created) => created,
        Err(err) => {
            note(ctx, host, Err(err));
            return Outcome::default();
        }
    };
    let canceled = deliver(
        ctx,
        host,
        walk,
        &callbacks,
        &event,
        &record,
        targets.as_ref(),
    );
    Outcome {
        canceled,
        entered: false,
    }
}

/// Calls the listeners along the path with `event`, then leaves the event
/// idle. `targets` holds what stands for each node as `currentTarget`.
/// Returns whether it ended canceled.
fn deliver<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    walk: &Walk<'_>,
    callbacks: &Object<'js>,
    event: &Object<'js>,
    record: &Object<'js>,
    targets: Option<&Array<'js>>,
) -> bool {
    let down = (0..walk.path.len()).rev().map(|index| (true, index));
    let up = (0..bubble_nodes(walk.flags, walk.path).len()).map(|index| (false, index));
    for (capture, index) in down.chain(up) {
        let node = walk.path[index];
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
        let entered = target_at(ctx, targets, index, node)
            .and_then(|current| event.set("currentTarget", current))
            .and_then(|()| event.set("eventPhase", phase));
        note(ctx, host, entered);
        for listener in listeners {
            if flag(record, "immediate") {
                break;
            }
            // The full value is compared. Flags change only through remove, then add.
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
            break;
        }
    }
    let canceled = flag(record, "canceled");
    // The record is ours and cannot fail; clearing it first keeps the guard
    // free when a listener froze the event.
    for key in ["dispatching", "passive", "stop", "immediate"] {
        let _ = record.set(key, false);
    }
    // A kept event keeps only target and relatedTarget alive.
    let _ = record.set("path", Vec::<NodeId>::new());
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
/// Throws what `__inca_targets__` throws.
pub(crate) fn dispatch_event<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    target: NodeId,
    event: Value<'js>,
) -> JsResult<bool> {
    let not_event = || {
        Exception::throw_type(
            ctx,
            concat!(
                "dispatchEvent needs an event object, ",
                "so pass one made with new Event(type)"
            ),
        )
    };
    let event = event.into_object().ok_or_else(not_event)?;
    let record = find_record(ctx, &event)?.ok_or_else(not_event)?;
    if flag(&record, "dispatching") {
        return Err(Exception::throw_dom(
            ctx,
            "InvalidStateError",
            concat!(
                "the event is already being dispatched, ",
                "so dispatch it again after its listeners return"
            ),
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
    let targets = targets_of(ctx, walk.path)?;
    event.set("target", target_at(ctx, targets.as_ref(), 0, target)?)?;
    event.set("isTrusted", false)?;
    if listened(&host.borrow(), walk.name, walk.flags, walk.path, walk.cut) {
        let callbacks = ctx
            .globals()
            .get::<_, Object>("__inca_callbacks__")
            .or_else(|_| Object::new(ctx.clone()))?;
        event.set("eventId", next_event_id(host))?;
        record.set("path", path_value(ctx, targets.as_ref(), walk.path)?)?;
        // Last, so a failure above leaves the event free to dispatch again.
        record.set("dispatching", true)?;
        deliver(
            ctx,
            host,
            &walk,
            &callbacks,
            &event,
            &record,
            targets.as_ref(),
        );
    }
    Ok(!flag(&record, "canceled"))
}

/// Runs an untrusted `click` at `target`. A disabled button, a node that left the tree and a node
/// whose own click is still running ignore the call.
pub(crate) fn click(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, target: NodeId) {
    {
        let mut host = host.borrow_mut();
        let Some(node) = host.tree.get(target) else {
            return;
        };
        if inca_gpui::is_disabled_button(node) || !host.clicking.insert(target) {
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
