// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, Function, Object, Result as JsResult, Value};

use inca_gpui::{NodeId, TreeError};

use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;

/// One registration: the callback id, the phase it listens in and whether it
/// is passive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listener {
    /// The id the JS caller stored its function under.
    pub callback: u32,
    /// Listens while the event travels down to the target.
    pub capture: bool,
    /// `preventDefault()` calls from the callback are ignored.
    pub passive: bool,
}

/// Every `(node id, event name)` a JS caller has registered through
/// `addEventListener`, mapped to the [`Listener`]s it gave us. Holds owned
/// Rust data only, so Rust code outside any JS call, such as the input-event
/// dispatcher, can read it.
///
/// Nothing expires on its own: a registration lives until
/// [`unregister`](Self::unregister) drops it, or [`release`](Self::release)
/// does when the node goes away.
#[derive(Debug, Default)]
pub struct EventListeners {
    pub(super) by_node: HashMap<NodeId, HashMap<String, Vec<Listener>>>,
    pub(super) next_callback: u32,
}

impl EventListeners {
    /// Allocates the next callback id, registers it on `(node_id, event)` with the given
    /// phase and passivity, and returns it. Ids rise from 0 in call order, and each id is
    /// issued once, also after [`unregister`](Self::unregister) or [`release`](Self::release).
    /// The last id is `u32::MAX - 1`. After it the call returns `None` and leaves the
    /// registrations unchanged.
    ///
    /// Ids from `add` and explicit ids passed to [`register`](Self::register) share one
    /// space, so a caller uses one or the other.
    pub fn add(
        &mut self,
        node_id: NodeId,
        event: impl Into<String>,
        capture: bool,
        passive: bool,
    ) -> Option<u32> {
        let id = self.next_callback;
        self.next_callback = id.checked_add(1)?;
        self.register_with(node_id, event, id, capture, passive);
        Some(id)
    }

    /// Adds `callback_id` to `(node_id, event)` as a bubble-phase, active
    /// listener. Registering an id already there leaves the registrations
    /// unchanged.
    pub fn register(&mut self, node_id: NodeId, event: impl Into<String>, callback_id: u32) {
        self.register_with(node_id, event, callback_id, false, false);
    }

    /// Adds `callback_id` to `(node_id, event)` with the given phase and
    /// passivity. Registering an id already there leaves the registrations
    /// unchanged.
    pub fn register_with(
        &mut self,
        node_id: NodeId,
        event: impl Into<String>,
        callback_id: u32,
        capture: bool,
        passive: bool,
    ) {
        let listeners = self
            .by_node
            .entry(node_id)
            .or_default()
            .entry(event.into())
            .or_default();
        if !listeners.iter().any(|l| l.callback == callback_id) {
            listeners.push(Listener {
                callback: callback_id,
                capture,
                passive,
            });
        }
    }

    /// Drops `callback_id` from `(node_id, event)`, and reports whether it
    /// was registered.
    pub fn unregister(&mut self, node_id: NodeId, event: &str, callback_id: u32) -> bool {
        let Some(events) = self.by_node.get_mut(&node_id) else {
            return false;
        };
        let Some(listeners) = events.get_mut(event) else {
            return false;
        };
        let before = listeners.len();
        listeners.retain(|l| l.callback != callback_id);
        let removed = listeners.len() != before;
        if listeners.is_empty() {
            events.remove(event);
        }
        if events.is_empty() {
            self.by_node.remove(&node_id);
        }
        removed
    }

    /// Drops every registration on `node_id` and returns the callback ids
    /// that were there, in no particular order — the JS side needs them to
    /// drop the functions they name.
    pub fn release(&mut self, node_id: NodeId) -> Vec<u32> {
        self.by_node
            .remove(&node_id)
            .map(|events| events.into_values().flatten().map(|l| l.callback).collect())
            .unwrap_or_default()
    }

    /// Whether nothing is registered anywhere.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_node.is_empty()
    }

    /// The callback ids of `(node_id, event)` in registration order.
    #[must_use]
    pub fn callbacks_for(&self, node_id: NodeId, event: &str) -> Vec<u32> {
        self.listeners_for(node_id, event)
            .iter()
            .map(|l| l.callback)
            .collect()
    }

    /// The registrations of `(node_id, event)` in registration order.
    #[must_use]
    pub fn listeners_for(&self, node_id: NodeId, event: &str) -> &[Listener] {
        self.by_node
            .get(&node_id)
            .and_then(|events| events.get(event))
            .map_or(&[], Vec::as_slice)
    }
}

/// Registering and dropping input-event callbacks, and dispatching events
/// from script.
pub(super) fn install_events<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    native: &Object<'js>,
) -> JsResult<()> {
    {
        let host = Rc::clone(host);
        native.set(
            "addEventListener",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
                      event: String,
                      capture: Opt<bool>,
                      passive: Opt<bool>|
                      -> JsResult<u32> {
                    let mut host = host.borrow_mut();
                    if host.tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    host.listeners
                        .add(
                            node_id,
                            event,
                            capture.0.unwrap_or(false),
                            passive.0.unwrap_or(false),
                        )
                        .ok_or_else(|| Exception::throw_range(&ctx, "callback ids are exhausted"))
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "removeEventListener",
            Function::new(
                ctx.clone(),
                move |JsNodeId(node_id): JsNodeId, event: String, callback_id: u32| -> bool {
                    host.borrow_mut()
                        .listeners
                        .unregister(node_id, &event, callback_id)
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "dispatchEvent",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>,
                      JsNodeId(node_id): JsNodeId,
                      event: Value<'js>|
                      -> JsResult<bool> {
                    if host.borrow().tree.get(node_id).is_none() {
                        return Err(throw_tree_error(&ctx, TreeError::NodeNotFound(node_id)));
                    }
                    crate::event::dispatch_event(&ctx, &host, node_id, event)
                },
            )?,
        )?;
    }

    {
        let host = Rc::clone(host);
        native.set(
            "click",
            Function::new(
                ctx.clone(),
                move |ctx: Ctx<'js>, JsNodeId(node_id): JsNodeId| {
                    crate::event::click(&ctx, &host, node_id);
                },
            )?,
        )?;
    }

    Ok(())
}
