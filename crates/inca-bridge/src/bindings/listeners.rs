// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use inca_jsenv::callbacks;
use inca_node::NodeId;
use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use super::Natives;
use super::convert::{Flag, JsCallbackId, JsEventTargetId, JsNodeId, existing_node};
use super::host::Host;
use crate::EventTargetId;

/// One registration: the callback id, the phase it listens in and whether it
/// is passive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listener {
    /// The id of the function in the callback table.
    pub callback: u32,
    /// Listens while the event travels down to the target.
    pub capture: bool,
    /// `preventDefault()` calls from the callback are ignored.
    pub passive: bool,
}

/// Every `(target id, event name)` a JS caller has registered through
/// `addEventListener`, mapped to the [`Listener`]s it gave us. Holds owned
/// Rust data only, so Rust code outside any JS call, such as the input-event
/// dispatcher, can read it. Targets include element nodes, the global
/// document and the global window.
///
/// Nothing expires on its own: a registration lives until
/// [`remove`](Self::remove) drops it, or [`release`](Self::release)
/// does when the node goes away.
#[derive(Debug, Default)]
pub struct EventListeners {
    pub(super) by_target: HashMap<EventTargetId, HashMap<String, Vec<Listener>>>,
}

impl EventListeners {
    /// Adds `callback_id` to `(target, event)` with the given phase and
    /// passivity.
    fn insert(
        &mut self,
        target: EventTargetId,
        event: impl Into<String>,
        callback_id: u32,
        capture: bool,
        passive: bool,
    ) {
        let listeners = self
            .by_target
            .entry(target)
            .or_default()
            .entry(event.into())
            .or_default();
        listeners.push(Listener {
            callback: callback_id,
            capture,
            passive,
        });
    }

    /// Drops `callback_id` from `(target, event)`, and reports whether it
    /// was registered.
    pub fn remove(&mut self, target: EventTargetId, event: &str, callback_id: u32) -> bool {
        let Some(events) = self.by_target.get_mut(&target) else {
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
            self.by_target.remove(&target);
        }
        removed
    }

    /// Drops every registration on `node_id` and returns the callback ids
    /// that were there, in no particular order. The JS side needs them to
    /// drop the functions they name.
    pub fn release(&mut self, node_id: NodeId) -> Vec<u32> {
        self.by_target
            .remove(&EventTargetId::Node(node_id))
            .map(|events| events.into_values().flatten().map(|l| l.callback).collect())
            .unwrap_or_default()
    }

    /// Whether nothing is registered anywhere.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_target.is_empty()
    }

    /// The registrations of `(target, event)` in registration order.
    #[must_use]
    pub fn listeners_for(&self, target: EventTargetId, event: &str) -> &[Listener] {
        self.by_target
            .get(&target)
            .and_then(|events| events.get(event))
            .map_or(&[], Vec::as_slice)
    }
}

/// Registering and dropping input-event callbacks, and dispatching events
/// from script.
pub(super) fn install_events(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("addEventListener", add_event_listener)?;
    n.set("removeEventListener", remove_event_listener)?;
    n.set("dispatchEvent", dispatch_event)?;
    n.set("click", click)
}

fn add_event_listener<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsEventTargetId, String, Value<'js>, Flag, Flag) -> JsResult<u32> {
    move |ctx, JsEventTargetId(target), event, callback, capture, passive| {
        let mut host = host.borrow_mut();
        if let EventTargetId::Node(node_id) = target {
            existing_node(&ctx, &host.tree, node_id)?;
        }
        if !callback.is_function() {
            return Err(Exception::throw_type(
                &ctx,
                "addEventListener takes a function. pass a function as the callback",
            ));
        }
        let id = callbacks::store(&ctx, callback)?
            .ok_or_else(|| Exception::throw_range(&ctx, "callback ids are exhausted"))?;
        host.listeners.insert(
            target,
            event,
            id,
            capture.0.flatten().unwrap_or(false),
            passive.0.flatten().unwrap_or(false),
        );
        Ok(id)
    }
}

fn remove_event_listener<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsEventTargetId, String, JsCallbackId) -> JsResult<bool> {
    move |ctx, JsEventTargetId(target), event, JsCallbackId(callback_id)| {
        let removed = host
            .borrow_mut()
            .listeners
            .remove(target, &event, callback_id);
        if removed {
            callbacks::release(&ctx, &[callback_id])?;
        }
        Ok(removed)
    }
}

fn dispatch_event<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsEventTargetId, Value<'js>) -> JsResult<bool> {
    move |ctx, JsEventTargetId(target), event| {
        if let EventTargetId::Node(node_id) = target {
            existing_node(&ctx, &host.borrow().tree, node_id)?;
        }
        crate::event::dispatch_event(&ctx, &host, target, &event)
    }
}

fn click<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<()> {
    move |ctx, JsNodeId(node_id)| {
        existing_node(&ctx, &host.borrow().tree, node_id)?;
        crate::event::click(&ctx, &host, node_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::test_support::{callback_ids, engine_with_bindings};
    use crate::tests::callback_stored;
    use rstest::rstest;

    #[test]
    fn a_registration_keeps_its_phase_and_passivity() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "wheel", 7, true, true);
        listeners.insert(EventTargetId::Node(1), "wheel", 8, false, false);

        assert_eq!(
            listeners.listeners_for(EventTargetId::Node(1), "wheel"),
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
                r"
                const n = __inca_native__.createNode('div');
                __inca_native__.addEventListener(n, 'a', () => {});
                __inca_native__.addEventListener(n, 'a', () => {}, true);
                __inca_native__.addEventListener(n, 'a', () => {}, false, true);
                __inca_native__.addEventListener(n, 'a', () => {}, true, true);
                __inca_native__.removeEventListener(n, 'a', 4);
                ",
            )
            .unwrap();
        let host = host.borrow();
        let flags: Vec<_> = host
            .listeners
            .listeners_for(EventTargetId::Node(2), "a")
            .iter()
            .map(|l| (l.callback, l.capture, l.passive))
            .collect();
        assert_eq!(
            flags,
            [(1, false, false), (2, true, false), (3, false, true)]
        );
    }

    #[test]
    fn add_event_listener_reads_undefined_flags_as_false() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                const n = __inca_native__.createNode('div');
                __inca_native__.addEventListener(n, 'a', () => {}, undefined, undefined);
                ",
            )
            .unwrap();
        let host = host.borrow();
        let flags: Vec<_> = host
            .listeners
            .listeners_for(EventTargetId::Node(2), "a")
            .iter()
            .map(|l| (l.capture, l.passive))
            .collect();
        assert_eq!(flags, [(false, false)]);
    }

    #[test]
    fn add_event_listener_registers_each_id_under_the_target_it_names() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                const first = __inca_native__.createNode('div');
                const second = __inca_native__.createNode('div');
                __inca_native__.addEventListener(first, 'click', () => {});
                __inca_native__.addEventListener(second, 'click', () => {}, true);
                __inca_native__.addEventListener(first, 'focus', () => {}, false, true);
                ",
            )
            .unwrap();
        let host = host.borrow();

        assert_eq!(
            callback_ids(&host.listeners, EventTargetId::Node(2), "click"),
            &[1]
        );
        assert_eq!(
            callback_ids(&host.listeners, EventTargetId::Node(3), "click"),
            &[2]
        );
        assert_eq!(
            host.listeners
                .listeners_for(EventTargetId::Node(2), "focus"),
            &[Listener {
                callback: 3,
                capture: false,
                passive: true
            }]
        );
    }

    #[test]
    fn add_event_listener_returns_the_ids_in_call_order() {
        let (engine, _host) = engine_with_bindings();

        let ids: Vec<u32> = engine
            .eval(
                r"
                const n = __inca_native__.createNode('div');
                [__inca_native__.addEventListener(n, 'a', () => {}),
                 __inca_native__.addEventListener(n, 'b', () => {}, true),
                 __inca_native__.addEventListener(n, 'a', () => {})]
                ",
            )
            .unwrap();

        assert_eq!(ids, [1, 2, 3]);
    }

    #[test]
    fn add_event_listener_on_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        engine
            .eval::<()>(
                r"
                globalThis.typeError = false;
                try { __inca_native__.addEventListener(9999, 'click', () => {}); }
                catch (e) { globalThis.typeError = e instanceof TypeError; }
                ",
            )
            .unwrap();

        assert!(engine.eval::<bool>("typeError").unwrap());
    }

    /// Makes an `addEventListener` on an unknown node fail, and leaves a node `n`.
    fn failed_add(engine: &inca_jsenv::Engine) {
        engine
            .eval::<()>(
                r"
                globalThis.n = __inca_native__.createNode('div');
                try { __inca_native__.addEventListener(9999, 'click', () => {}); }
                catch (e) {}
                ",
            )
            .unwrap();
    }

    #[test]
    fn a_failed_add_event_listener_stores_nothing() {
        let (engine, _host) = engine_with_bindings();
        failed_add(&engine);

        assert!(!callback_stored(&engine, 1));
    }

    #[test]
    fn a_failed_add_event_listener_takes_no_callback_id() {
        let (engine, _host) = engine_with_bindings();
        failed_add(&engine);

        assert_eq!(
            engine
                .eval::<u32>("__inca_native__.addEventListener(n, 'click', () => {})")
                .unwrap(),
            1
        );
    }

    #[test]
    fn add_event_listener_throws_type_error_for_a_callback_that_is_not_a_function() {
        let (engine, host) = engine_with_bindings();

        let type_error: bool = engine
            .eval(
                r"
                (() => {
                    const n = __inca_native__.createNode('div');
                    try {
                        __inca_native__.addEventListener(n, 'click', true);
                        return false;
                    } catch (e) {
                        return e instanceof TypeError;
                    }
                })()
                ",
            )
            .unwrap();

        assert!(type_error);
        assert!(host.borrow().listeners.is_empty());
        assert!(!callback_stored(&engine, 1));
    }

    #[test]
    fn add_event_listener_throws_range_error_once_the_ids_run_out() {
        let (engine, host) = engine_with_bindings();
        engine.with(|ctx| {
            callbacks::exhaust_ids(&ctx).unwrap();
        });

        let range_error: bool = engine
            .eval(
                r"
                (() => { const n = __inca_native__.createNode('div');
                try { __inca_native__.addEventListener(n, 'click', () => {}); return false; }
                catch (e) { return e instanceof RangeError; } })()
                ",
            )
            .unwrap();

        assert!(range_error);
        assert!(host.borrow().listeners.is_empty());
    }

    #[test]
    fn remove_reports_whether_it_removed_anything() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "click", 7, false, false);

        assert!(listeners.remove(EventTargetId::Node(1), "click", 7));
        assert!(callback_ids(&listeners, EventTargetId::Node(1), "click").is_empty());
        assert!(!listeners.remove(EventTargetId::Node(1), "click", 7));
        assert!(!listeners.remove(EventTargetId::Node(99), "click", 7));
    }

    #[test]
    fn release_returns_every_callback_on_a_node() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "click", 7, false, false);
        listeners.insert(EventTargetId::Node(1), "focus", 8, false, false);
        listeners.insert(EventTargetId::Node(2), "click", 9, false, false);

        let mut released = listeners.release(1);
        released.sort_unstable();

        assert_eq!(released, vec![7, 8]);
        assert!(callback_ids(&listeners, EventTargetId::Node(1), "click").is_empty());
        assert_eq!(
            callback_ids(&listeners, EventTargetId::Node(2), "click"),
            &[9],
            "another node's registrations must survive"
        );
    }

    #[test]
    fn destroying_a_node_leaves_the_window_registration() {
        let (engine, host) = engine_with_bindings();
        let node_callbacks: Vec<u32> = engine
            .eval(
                r"
                const n = __inca_native__.createNode('div');
                __inca_native__.addEventListener('$window', 'click', () => {});
                __inca_native__.addEventListener(n, 'click', () => {});
                __inca_native__.destroyNode(n)
                ",
            )
            .unwrap();
        let host = host.borrow();

        assert_eq!(
            node_callbacks,
            [2],
            "freeing a node frees only its own callbacks"
        );
        assert_eq!(
            callback_ids(&host.listeners, EventTargetId::Window, "click"),
            [1]
        );
    }

    #[test]
    fn remove_event_listener_drops_the_registration() {
        let (engine, host) = engine_with_bindings();

        let removed: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                const id = __inca_native__.addEventListener(node, 'click', () => {});
                __inca_native__.removeEventListener(node, 'click', id);
                ",
            )
            .unwrap();

        assert!(removed);
        assert!(callback_ids(&host.borrow().listeners, EventTargetId::Node(2), "click").is_empty());
    }

    #[test]
    fn remove_event_listener_releases_the_table_entry() {
        let (engine, _host) = engine_with_bindings();

        engine
            .eval::<()>(
                r"
                const node = __inca_native__.createNode('div');
                const id = __inca_native__.addEventListener(node, 'click', () => {});
                __inca_native__.removeEventListener(node, 'click', id);
                ",
            )
            .unwrap();

        assert!(!callback_stored(&engine, 1));
    }

    #[test]
    fn removing_a_callback_id_that_is_not_registered_keeps_the_table_entry() {
        let (engine, _host) = engine_with_bindings();

        engine
            .eval::<()>(
                r"
                const node = __inca_native__.createNode('div');
                __inca_native__.addEventListener(node, 'click', () => {});
                __inca_native__.removeEventListener(node, 'focus', 1);
                ",
            )
            .unwrap();

        assert!(callback_stored(&engine, 1));
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

    #[rstest]
    #[case::nan("NaN")]
    #[case::fractional("1.5")]
    #[case::negative("-1")]
    #[case::infinity("Infinity")]
    #[case::above_u32("2 ** 32")]
    #[case::numeric_string("'1'")]
    fn a_callback_id_that_is_not_a_whole_u32_throws_type_error_and_removes_nothing(
        #[case] bad: &str,
    ) {
        let (engine, host) = engine_with_bindings();

        let script = format!(
            r"
            const node = __inca_native__.createNode('div');
            __inca_native__.addEventListener(node, 'click', () => {{}});
            __inca_native__.addEventListener(node, 'click', () => {{}});
            let caught = false;
            try {{
                __inca_native__.removeEventListener(node, 'click', {bad});
            }} catch (e) {{
                caught = e instanceof TypeError;
            }}
            caught;
            "
        );

        assert!(engine.eval::<bool>(&script).unwrap(), "{bad}");
        assert_eq!(
            callback_ids(&host.borrow().listeners, EventTargetId::Node(2), "click"),
            [1, 2],
            "{bad}"
        );
    }

    #[test]
    fn the_largest_u32_callback_id_is_accepted() {
        let (engine, _host) = engine_with_bindings();

        let removed: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                __inca_native__.removeEventListener(node, 'click', 4294967295);
                ",
            )
            .unwrap();

        assert!(!removed);
    }
}
