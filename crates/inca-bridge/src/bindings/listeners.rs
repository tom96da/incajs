// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use inca_jsenv::callbacks;
use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use super::Natives;
use super::convert::{Flag, JsCallbackId, JsEventTargetId, JsNodeId, existing_node, or_null};
use super::host::Host;
use crate::abort::{Table, after_removal, settle_freed};
use crate::event_target::HookTarget;
use inca_document::{EventTargetId, Link};

/// Registering and dropping input-event callbacks, and dispatching events
/// from script.
pub(super) fn install_events(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("addEventListener", add_event_listener)?;
    n.set("removeEventListener", remove_event_listener)?;
    n.set("dispatchEvent", dispatch_event)?;
    n.set("windowTargetId", window_target_id)?;
    n.set("click", click)
}

/// The target that names the window in the listener and dispatch calls.
fn window_target_id(_host: Rc<RefCell<Host>>) -> impl Fn() -> HookTarget {
    || HookTarget::Window
}

/// The message of a `signal` option that is no `AbortSignal`.
const NOT_A_SIGNAL: &str =
    "the signal option is not an AbortSignal. pass the signal of an AbortController";

/// `addEventListener(target, event, callback, capture, passive, signal)`: registers `callback`
/// and returns its id, or `null` when `signal` is aborted already, which stores nothing. A
/// registration with a signal is removed when the signal aborts.
fn add_event_listener<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(
    Ctx<'js>,
    JsEventTargetId,
    String,
    Value<'js>,
    Flag,
    Flag,
    Opt<Value<'js>>,
) -> JsResult<Value<'js>> {
    move |ctx, JsEventTargetId(target), event, callback, capture, passive, signal| {
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
        let signal = match signal.0.filter(|value| !value.is_undefined()) {
            Some(value) => Some(
                crate::abort::signal_id(&value)
                    .ok_or_else(|| Exception::throw_type(&ctx, NOT_A_SIGNAL))?,
            ),
            None => None,
        };
        if signal.is_some_and(|signal| host.signals.is_aborted(signal)) {
            return or_null::<u32>(&ctx, None);
        }
        let id = callbacks::store(&ctx, callback)?
            .ok_or_else(|| Exception::throw_range(&ctx, "callback ids are exhausted"))?;
        let pins = matches!(target, EventTargetId::Signal(own) if event == "abort" && host.signals.pins(own));
        let link = Link {
            target,
            event: event.clone(),
        };
        host.listeners.insert(
            target,
            event,
            id,
            capture.0.flatten().unwrap_or(false),
            passive.0.flatten().unwrap_or(false),
        );
        if let EventTargetId::Signal(own) = target
            && pins
        {
            Table::open(&ctx)?.pin(own)?;
        }
        if let Some(signal) = signal {
            host.listeners.links.link(signal, id, link);
            if host.signals.pins(signal) {
                Table::open(&ctx)?.pin(signal)?;
            }
        }
        or_null(&ctx, Some(id))
    }
}

fn remove_event_listener<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsEventTargetId, String, JsCallbackId) -> JsResult<bool> {
    move |ctx, JsEventTargetId(target), event, JsCallbackId(callback_id)| {
        let mut host = host.borrow_mut();
        let removed = host.listeners.remove(target, &event, callback_id);
        if removed {
            callbacks::release(&ctx, &[callback_id])?;
            if let EventTargetId::Signal(signal) = target {
                after_removal(&host, &Table::open(&ctx)?, signal)?;
            }
            settle_freed(&ctx, &mut host)?;
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
    use inca_document::Listener;
    use rstest::rstest;

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
    fn a_listener_on_the_window_target_registers_on_the_window() {
        let (engine, host) = engine_with_bindings();
        engine
            .eval::<u32>(
                r"
                const n = __inca_native__;
                n.addEventListener(n.windowTargetId(), 'click', () => {})
                ",
            )
            .unwrap();

        assert_eq!(
            callback_ids(&host.borrow().listeners, EventTargetId::Window, "click"),
            [1]
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

    #[test]
    fn dispatching_at_a_signal_needs_no_node() {
        let (engine, _host) = engine_with_bindings();

        let dispatched: bool = engine
            .eval(
                r"
                const s = new AbortController().signal;
                __inca_native__.dispatchEvent(s, new Event('abort'));
                ",
            )
            .unwrap();

        assert!(dispatched);
    }
}
