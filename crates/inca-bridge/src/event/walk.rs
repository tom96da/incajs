// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use inca_gpui::{EventPayload, MousePayload, NodeId};
use inca_jsenv::{Engine, EngineError};
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, Value};

use super::construct::create;
use super::{Flags, IncaEvent, targets_of};
use crate::bindings::{Host, Listener};

/// What one walk needs to know.
pub(crate) struct Walk<'a> {
    pub(crate) name: &'a str,
    pub(crate) flags: Flags,
    pub(crate) target: NodeId,
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

/// Whether a listener of `name` exists that an event with `flags` fired at
/// `target` reaches. Nodes in `cut` stay silent.
pub(crate) fn listened(
    host: &Host,
    name: &str,
    flags: Flags,
    target: NodeId,
    cut: &[NodeId],
) -> bool {
    host.tree
        .ancestors(target)
        .enumerate()
        .any(|(depth, node)| {
            !cut.contains(&node)
                && host
                    .listeners
                    .listeners_for(node, name)
                    .iter()
                    .any(|l| l.capture || flags.bubbles || depth == 0)
        })
}

fn next_event_id(host: &Rc<RefCell<Host>>) -> u64 {
    let mut host = host.borrow_mut();
    host.last_event_id += 1;
    host.last_event_id
}

/// Runs the listeners of `walk.name` along the path of `walk.target` with one
/// event object. Capture listeners run from the root down to the target, then
/// bubble listeners from the target up, the latter only while the event
/// bubbles. At the target, capture listeners run before bubble listeners. A
/// node's listeners are those registered when the walk reaches it, minus any
/// removed while an earlier listener ran. The engine is entered only when the
/// event reaches a listener.
pub(crate) fn walk(engine: &Engine, host: &Rc<RefCell<Host>>, walk: &Walk<'_>) -> Outcome {
    if !listened(&host.borrow(), walk.name, walk.flags, walk.target, walk.cut) {
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
    payload: &'a EventPayload,
) -> Walk<'a> {
    Walk {
        name,
        flags,
        target,
        cut: &[],
        payload,
        trusted: false,
    }
}

fn run(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, walk: &Walk<'_>, id: u64) -> Outcome {
    let Ok(callbacks) = ctx.globals().get::<_, Object>("__inca_callbacks__") else {
        return Outcome::default();
    };
    let path = path_from(&host.borrow(), walk.target);
    let created = targets_of(ctx, &path).and_then(|targets| create(ctx, walk, id, targets));
    let event = match created {
        Ok(event) => event,
        Err(err) => {
            note(ctx, host, Err(err));
            return Outcome::default();
        }
    };
    let canceled = deliver(ctx, host, walk, &path, &callbacks, &event);
    Outcome {
        canceled,
        entered: false,
    }
}

/// Calls the listeners along `path` with `event`, then leaves the event
/// idle. Returns whether it ended canceled.
fn deliver<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    walk: &Walk<'_>,
    path: &[NodeId],
    callbacks: &Object<'js>,
    event: &Class<'js, IncaEvent<'js>>,
) -> bool {
    let record = event.borrow();
    let state = &record.state;
    let down = (0..path.len()).rev().map(|index| (true, index));
    let up = (0..bubble_nodes(walk.flags, path).len()).map(|index| (false, index));
    for (capture, index) in down.chain(up) {
        if state.stop.get() {
            break;
        }
        let node = path[index];
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
        state.current.set(Some(index));
        state.phase.set(phase);
        for listener in listeners {
            if state.immediate.get() {
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
            state.passive.set(listener.passive);
            let called = callback.call::<_, ()>((event.clone(),));
            note(ctx, host, called);
        }
    }
    let canceled = state.canceled.get();
    end_dispatch(&record);
    canceled
}

/// Leaves `event` as a dispatch leaves it: no walk state, no stop flags,
/// no path.
fn end_dispatch(event: &IncaEvent<'_>) {
    let state = &event.state;
    for flag in [
        &state.dispatching,
        &state.passive,
        &state.stop,
        &state.immediate,
    ] {
        flag.set(false);
    }
    state.current.set(None);
    state.phase.set(0);
    // A kept event keeps only target, relatedTarget and detail alive.
    drop(event.held.path.take());
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
    event: &Value<'js>,
) -> JsResult<bool> {
    let event = Class::<IncaEvent>::from_value(event).map_err(|_| {
        Exception::throw_type(
            ctx,
            concat!(
                "dispatchEvent needs an event object, ",
                "so pass one made with new Event(type)"
            ),
        )
    })?;
    let record = event.borrow();
    let state = &record.state;
    if state.dispatching.get() {
        return Err(Exception::throw_dom(
            ctx,
            "InvalidStateError",
            concat!(
                "the event is already being dispatched, ",
                "so dispatch it again after its listeners return"
            ),
        ));
    }
    let path = path_from(&host.borrow(), target);
    let walk = script_walk(&state.name, state.flags, target, &EventPayload::None);
    let targets = targets_of(ctx, &path)?;
    drop(record.held.target.replace(targets.first().cloned()));
    state.trusted.set(false);
    if listened(&host.borrow(), walk.name, walk.flags, target, walk.cut) {
        let callbacks = ctx
            .globals()
            .get::<_, Object>("__inca_callbacks__")
            .or_else(|_| Object::new(ctx.clone()))?;
        state.event_id.set(Some(next_event_id(host)));
        drop(record.held.path.replace(targets));
        // Last, so a failure above leaves the event free to dispatch again.
        state.dispatching.set(true);
        deliver(ctx, host, &walk, &path, &callbacks, &event);
    } else {
        end_dispatch(&record);
    }
    Ok(!state.canceled.get())
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
    let payload = EventPayload::Mouse(MousePayload::keyboard_click(gpui::Modifiers::none()));
    let walk = script_walk(
        "click",
        crate::dispatch::spec_of("click").flags,
        target,
        &payload,
    );
    if listened(&host.borrow(), walk.name, walk.flags, target, walk.cut) {
        run(ctx, host, &walk, next_event_id(host));
    }
    host.borrow_mut().clicking.remove(&target);
}
