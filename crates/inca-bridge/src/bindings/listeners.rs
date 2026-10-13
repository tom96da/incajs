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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bindings::fixtures::engine_with_bindings;

    #[test]
    fn registering_the_same_callback_twice_keeps_one_entry() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);
        listeners.register(1, "click", 7);

        assert_eq!(listeners.callbacks_for(1, "click"), &[7]);
    }

    #[test]
    fn distinct_callbacks_on_one_event_stack() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);
        listeners.register(1, "click", 8);

        assert_eq!(listeners.callbacks_for(1, "click"), &[7, 8]);
    }

    #[test]
    fn a_registration_keeps_its_phase_and_passivity_and_the_first_one_wins() {
        let mut listeners = EventListeners::default();
        listeners.register_with(1, "wheel", 7, true, true);
        listeners.register_with(1, "wheel", 7, false, false);
        listeners.register(1, "wheel", 8);

        assert_eq!(
            listeners.listeners_for(1, "wheel"),
            &[
                Listener {
                    callback: 7,
                    capture: true,
                    passive: true
                },
                Listener {
                    callback: 8,
                    capture: false,
                    passive: false
                },
            ]
        );
    }

    #[test]
    fn add_event_listener_takes_optional_capture_and_passive_flags() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>(
                "const n = __inca_native__.createNode('div'); \
                 __inca_native__.addEventListener(n, 'a'); \
                 __inca_native__.addEventListener(n, 'a', true); \
                 __inca_native__.addEventListener(n, 'a', false, true); \
                 __inca_native__.addEventListener(n, 'a', true, true); \
                 __inca_native__.removeEventListener(n, 'a', 3);",
            )
            .unwrap();
        let host = host.borrow();
        let flags: Vec<_> = host
            .listeners
            .listeners_for(1, "a")
            .iter()
            .map(|l| (l.callback, l.capture, l.passive))
            .collect();
        assert_eq!(
            flags,
            [(0, false, false), (1, true, false), (2, false, true)]
        );
    }

    #[test]
    fn add_hands_out_ids_from_zero_in_call_order_across_nodes_and_events() {
        let mut listeners = EventListeners::default();

        assert_eq!(listeners.add(1, "click", false, false), Some(0));
        assert_eq!(listeners.add(2, "click", true, false), Some(1));
        assert_eq!(listeners.add(1, "focus", false, true), Some(2));
        assert_eq!(listeners.callbacks_for(1, "click"), &[0]);
        assert_eq!(listeners.callbacks_for(2, "click"), &[1]);
        assert_eq!(listeners.callbacks_for(1, "focus"), &[2]);
        assert_eq!(
            listeners.listeners_for(1, "focus"),
            &[Listener {
                callback: 2,
                capture: false,
                passive: true
            }]
        );
    }

    #[test]
    fn add_never_reuses_an_id_after_unregister_or_release() {
        let mut listeners = EventListeners::default();
        let first = listeners.add(1, "click", false, false).unwrap();
        listeners.unregister(1, "click", first);
        let second = listeners.add(1, "click", false, false).unwrap();
        listeners.release(1);

        assert_eq!((first, second), (0, 1));
        assert_eq!(listeners.add(1, "click", false, false), Some(2));
    }

    #[test]
    fn add_returns_none_and_registers_nothing_once_the_ids_run_out() {
        let mut listeners = EventListeners {
            next_callback: u32::MAX - 1,
            ..EventListeners::default()
        };

        assert_eq!(listeners.add(1, "click", false, false), Some(u32::MAX - 1));
        assert_eq!(listeners.add(1, "focus", false, false), None);
        assert_eq!(listeners.add(1, "focus", false, false), None);
        assert!(listeners.callbacks_for(1, "focus").is_empty());
    }

    #[test]
    fn add_event_listener_returns_the_ids_in_call_order() {
        let (engine, _host) = engine_with_bindings();

        let ids: Vec<u32> = engine
            .eval(
                "const n = __inca_native__.createNode('div'); \
                 [__inca_native__.addEventListener(n, 'a'), \
                  __inca_native__.addEventListener(n, 'b', true), \
                  __inca_native__.addEventListener(n, 'a')]",
            )
            .unwrap();

        assert_eq!(ids, [0, 1, 2]);
    }

    #[test]
    fn add_event_listener_on_an_unknown_node_raises_type_error_and_takes_no_id() {
        let (engine, _host) = engine_with_bindings();

        engine
            .eval::<()>(
                "globalThis.n = __inca_native__.createNode('div'); \
                 globalThis.typeError = false; \
                 try { __inca_native__.addEventListener(9999, 'click'); } \
                 catch (e) { globalThis.typeError = e instanceof TypeError; }",
            )
            .unwrap();

        assert!(engine.eval::<bool>("typeError").unwrap());
        assert_eq!(
            engine
                .eval::<u32>("__inca_native__.addEventListener(n, 'click')")
                .unwrap(),
            0
        );
    }

    #[test]
    fn add_event_listener_raises_range_error_once_the_ids_run_out() {
        let (engine, host) = engine_with_bindings();
        host.borrow_mut().listeners.next_callback = u32::MAX;

        let range_error: bool = engine
            .eval(
                "(() => { const n = __inca_native__.createNode('div'); \
                 try { __inca_native__.addEventListener(n, 'click'); return false; } \
                 catch (e) { return e instanceof RangeError; } })()",
            )
            .unwrap();

        assert!(range_error);
        assert!(host.borrow().listeners.is_empty());
    }

    #[test]
    fn unregister_reports_whether_it_removed_anything() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);

        assert!(listeners.unregister(1, "click", 7));
        assert!(listeners.callbacks_for(1, "click").is_empty());
        assert!(!listeners.unregister(1, "click", 7));
        assert!(!listeners.unregister(99, "click", 7));
    }

    #[test]
    fn release_returns_every_callback_on_a_node() {
        let mut listeners = EventListeners::default();
        listeners.register(1, "click", 7);
        listeners.register(1, "focus", 8);
        listeners.register(2, "click", 9);

        let mut released = listeners.release(1);
        released.sort_unstable();

        assert_eq!(released, vec![7, 8]);
        assert!(listeners.callbacks_for(1, "click").is_empty());
        assert_eq!(
            listeners.callbacks_for(2, "click"),
            &[9],
            "another node's registrations must survive"
        );
    }

    #[test]
    fn remove_event_listener_drops_the_registration() {
        let (engine, host) = engine_with_bindings();

        let removed: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                const id = __inca_native__.addEventListener(node, 'click');
                __inca_native__.removeEventListener(node, 'click', id);
                ",
            )
            .unwrap();

        assert!(removed);
        assert!(host.borrow().listeners.callbacks_for(1, "click").is_empty());
    }

    #[test]
    fn remove_event_listener_on_a_gone_node_is_not_an_error() {
        let (engine, _host) = engine_with_bindings();

        let removed: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                __inca_native__.destroyNode(node);
                __inca_native__.removeEventListener(node, 'click', 3);
                ",
            )
            .unwrap();

        assert!(
            !removed,
            "removing after a destroy races normal teardown and must not throw"
        );
    }
}
