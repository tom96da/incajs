// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The signal objects and the abort steps: a new signal, its abort, the hold of a signal and the release of a collected one.

use std::rc::Rc;

use inca_jsenv::callbacks;
use rquickjs::{Class, Ctx, Exception, Result as JsResult, Value};

use super::classes::install as install_classes;
use super::{IncaSignal, Table};

use crate::bindings::Host;
use inca_document::DocumentState;
use inca_document::EventTargetId;

/// Defines `AbortController` and `AbortSignal` and the signal table.
///
/// # Errors
///
/// Returns an error if the node interfaces are not installed or a definition fails.
pub(crate) fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    install_classes(ctx, host)
}

/// The Rust id of `value` when it is an `AbortSignal` object, `None` for any other value.
pub(crate) fn signal_id(value: &Value<'_>) -> Option<u32> {
    Class::<IncaSignal>::from_value(value)
        .ok()
        .map(|signal| signal.borrow().id())
}

/// The native `signalId(value)`: the id of a signal object, `null` for any other value.
// A native function takes its arguments by value.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn signal_id_native<'js>(ctx: Ctx<'js>, value: Value<'js>) -> JsResult<Value<'js>> {
    crate::bindings::convert::or_null(&ctx, signal_id(&value))
}

/// A new signal object, tracked in the table.
///
/// # Errors
///
/// Throws a `RangeError` after the last signal id.
pub(crate) fn new_signal<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<Host>,
    table: &Table<'js>,
    dependent: bool,
) -> JsResult<Class<'js, IncaSignal<'js>>> {
    let id = host
        .document
        .borrow_mut()
        .signals
        .create(dependent)
        .ok_or_else(|| Exception::throw_range(ctx, "signal ids are exhausted. restart the app"))?;
    let proto = inca_jsenv::required_prototype(ctx, "AbortSignal")?;
    let signal = Class::instance_proto(IncaSignal::new(id), proto)?;
    table.track(&signal, id)?;
    Ok(signal)
}

/// A `DOMException` named `name`.
pub(crate) fn dom_exception<'js>(ctx: &Ctx<'js>, name: &str, message: &str) -> Value<'js> {
    drop(Exception::throw_dom(ctx, name, message));
    ctx.catch()
}

/// Aborts `signal` with `reason`, or an `AbortError` when `reason` is `None`. The signal and its
/// dependents take the reason, then the abort event runs for each. A signal aborted already stays
/// as it is.
///
/// # Errors
///
/// Returns an error when the engine fails, not when a listener throws.
pub(crate) fn abort_signal<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<Host>,
    table: &Table<'js>,
    signal: &Class<'js, IncaSignal<'js>>,
    reason: Option<Value<'js>>,
) -> JsResult<()> {
    let id = signal.borrow().id();
    let Some(dependents) = host.document.borrow_mut().signals.mark_aborted(id) else {
        return Ok(());
    };
    let reason = match reason {
        Some(reason) => reason,
        None => dom_exception(ctx, "AbortError", "signal is aborted without reason"),
    };
    signal.borrow().set_reason(reason.clone());
    for dependent in &dependents {
        if let Some(object) = table.find(*dependent)?
            && let Some(dependent) = Class::<IncaSignal>::from_object(&object)
        {
            dependent.borrow().set_reason(reason.clone());
        }
    }
    run_abort_steps(ctx, host, table, id)?;
    for dependent in dependents {
        run_abort_steps(ctx, host, table, dependent)?;
    }
    Ok(())
}

/// Runs the abort steps of signal `id`: removes the registrations that name it as their `signal`
/// option, tells core their ids, fires the event and ends the strong hold.
fn run_abort_steps(ctx: &Ctx<'_>, host: &Rc<Host>, table: &Table<'_>, id: u32) -> JsResult<()> {
    let removed = {
        let mut host = host.document.borrow_mut();
        let linked = host.listeners.links.take(id);
        let mut ids = Vec::new();
        for (callback, link) in linked {
            if host.listeners.remove(link.target, &link.event, callback) {
                ids.push(callback);
                if let EventTargetId::Signal(target) = link.target {
                    after_removal(&host, table, target)?;
                }
            }
        }
        settle_freed(ctx, &mut host)?;
        ids
    };
    callbacks::release(ctx, &removed)?;
    if !removed.is_empty() {
        crate::script::notify_released(ctx, host, &[], &removed);
    }
    if table.find(id)?.is_some() {
        crate::event::fire_abort(ctx, host, id);
    }
    table.unpin(id)
}

/// Ends the strong hold of signal `id` when nothing needs it any more. That is the case when it is
/// dependent with no source left, or when no `abort` listener is left on it.
pub(crate) fn after_removal(host: &DocumentState, table: &Table<'_>, id: u32) -> JsResult<()> {
    let listened = host
        .listeners
        .has_listener(EventTargetId::Signal(id), "abort")
        || host.listeners.links.has(id);
    if host.signals.pins(id) && listened {
        Ok(())
    } else {
        table.unpin(id)
    }
}

/// Ends the strong hold of each signal that lost a link, where nothing needs it any more. Runs no
/// script.
pub(crate) fn settle_freed(ctx: &Ctx<'_>, host: &mut DocumentState) -> JsResult<()> {
    let freed = host.listeners.links.take_freed();
    if freed.is_empty() {
        return Ok(());
    }
    let table = Table::open(ctx)?;
    for id in freed {
        after_removal(host, &table, id)?;
    }
    Ok(())
}

/// Drops everything of the collected signal `id`: its state, its own registrations and the
/// callback table entries, and tells core.
pub(crate) fn release_signal(ctx: &Ctx<'_>, host: &Rc<Host>, id: u32) -> JsResult<()> {
    let table = Table::open(ctx)?;
    let ids = {
        let mut host = host.document.borrow_mut();
        let orphaned = host.signals.release(id);
        host.listeners.links.forget(id);
        let ids = host.listeners.release_target(EventTargetId::Signal(id));
        for dependent in orphaned {
            after_removal(&host, &table, dependent)?;
        }
        settle_freed(ctx, &mut host)?;
        table.forget(id)?;
        ids
    };
    callbacks::release(ctx, &ids)?;
    crate::script::notify_released(ctx, host, &[], &ids);
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{Clocked, callback_ids, engine_with_bindings, error_name, json_of};
    use inca_document::{EventTargetId, Signals};

    /// A node, a controller `c`, its signal `s`, a log, `listen(fn, capture)` on the signal and
    /// `add(target, fn, signal, capture)`, which registers a `click` listener with the signal
    /// option.
    const SETUP: &str = r"
        globalThis.log = [];
        globalThis.n = __inca_native__;
        globalThis.node = n.createNode('div');
        globalThis.c = new AbortController();
        globalThis.s = c.signal;
        globalThis.listen = (fn, capture = false) =>
            __inca_native__.addEventListener(s, 'abort', fn, capture);
        globalThis.add = (target, fn, signal, capture = false) =>
            n.addEventListener(target, 'click', fn, capture, false, signal);
        ";

    /// `expr` after [`SETUP`], as JSON.
    fn json(expr: &str) -> serde_json::Value {
        let (engine, _host) = engine_with_bindings();
        engine.eval::<()>(SETUP).unwrap();
        json_of(&engine, expr)
    }

    #[test]
    fn abort_gives_an_abort_error_with_the_default_message() {
        let got = json("(c.abort(), [s.reason.name, s.reason.code, s.reason.message])");

        assert_eq!(
            got,
            json!(["AbortError", 20, "signal is aborted without reason"])
        );
    }

    #[rstest]
    #[case::object("({})")]
    #[case::falsy("0")]
    fn abort_keeps_the_given_reason_as_it_is(#[case] reason: &str) {
        let got = json(&format!(
            r"(() => {{
                const r = {reason};
                c.abort(r);
                return Object.is(s.reason, r);
            }})()"
        ));

        assert_eq!(got, json!(true));
    }

    #[test]
    fn abort_with_undefined_gives_the_default_reason() {
        assert_eq!(
            json("(c.abort(undefined), s.reason.name)"),
            json!("AbortError")
        );
    }

    #[test]
    fn a_second_abort_keeps_the_reason_and_fires_no_second_event() {
        let got = json(
            r"(() => {
                listen(() => log.push('e'));
                const first = c.abort('a');
                const second = c.abort('b');
                return [s.reason, log.length, first === undefined, second === undefined];
            })()",
        );

        assert_eq!(got, json!(["a", 1, true, true]));
    }

    #[test]
    fn a_signal_reads_not_aborted_with_no_reason_before_the_abort() {
        assert_eq!(
            json("[s.aborted, s.reason === undefined]"),
            json!([false, true])
        );
    }

    #[test]
    fn throw_if_aborted_does_nothing_before_the_abort() {
        assert_eq!(json("s.throwIfAborted() === undefined"), json!(true));
    }

    #[rstest]
    #[case::object("({})")]
    #[case::falsy("0")]
    fn throw_if_aborted_throws_the_reason_itself(#[case] reason: &str) {
        let got = json(&format!(
            r"(() => {{
                const r = {reason};
                c.abort(r);
                try {{
                    s.throwIfAborted();
                }} catch (e) {{
                    return Object.is(e, r);
                }}
                return 'none';
            }})()"
        ));

        assert_eq!(got, json!(true));
    }

    #[rstest]
    #[case::type_("e.type", json!("abort"))]
    #[case::bubbles("e.bubbles", json!(false))]
    #[case::cancelable("e.cancelable", json!(false))]
    #[case::trusted("e.isTrusted", json!(true))]
    #[case::target("e.target === s", json!(true))]
    #[case::current_target("e.currentTarget === s", json!(true))]
    #[case::phase("e.eventPhase", json!(2))]
    fn the_abort_event_carries_its_fields(#[case] field: &str, #[case] want: serde_json::Value) {
        let got = json(&format!(
            "(listen((e) => log.push({field})), c.abort(), log[0])"
        ));

        assert_eq!(got, want);
    }

    #[test]
    fn the_event_leaves_no_current_target_after_the_walk() {
        let got = json(
            r"(() => {
                listen((e) => {
                    globalThis.kept = e;
                });
                c.abort();
                return [kept.currentTarget, kept.eventPhase, kept.composedPath().length];
            })()",
        );

        assert_eq!(got, json!([null, 0, 0]));
    }

    #[test]
    fn the_path_of_the_event_is_the_signal_alone() {
        let got = json("(listen((e) => log.push(e.composedPath().length)), c.abort(), log[0])");

        assert_eq!(got, json!(1));
    }

    #[test]
    fn listeners_run_before_abort_returns() {
        let got = json("(listen(() => log.push('e')), c.abort(), log.length)");

        assert_eq!(got, json!(1));
    }

    #[test]
    fn the_capture_listener_runs_before_the_bubble_listener() {
        let got = json(
            "(listen(() => log.push('bubble')), listen(() => log.push('capture'), true), c.abort(), log)",
        );

        assert_eq!(got, json!(["capture", "bubble"]));
    }

    #[test]
    fn a_throwing_listener_leaves_abort_normal_and_the_next_listener_runs() {
        let (engine, host) = engine_with_bindings();
        engine.eval::<()>(SETUP).unwrap();

        let got = json_of(
            &engine,
            r"(() => {
                listen(() => {
                    throw new Error('boom');
                });
                listen(() => log.push('next'));
                c.abort();
                return log;
            })()",
        );

        assert_eq!(got, json!(["next"]));
        assert_eq!(host.js.borrow().failures.len(), 1);
    }

    #[test]
    fn a_listener_sees_the_signal_aborted_with_its_reason() {
        let got = json("(listen(() => log.push([s.aborted, s.reason.name])), c.abort(), log[0])");

        assert_eq!(got, json!([true, "AbortError"]));
    }

    #[test]
    fn a_listener_that_aborts_its_own_signal_changes_nothing() {
        let got = json(
            r"(() => {
                listen(() => {
                    log.push(1);
                    c.abort('x');
                });
                c.abort('first');
                return [log.length, s.reason];
            })()",
        );

        assert_eq!(got, json!([1, "first"]));
    }

    #[test]
    fn dispatching_an_abort_event_leaves_the_signal_unaborted() {
        let got = json(
            "(listen(() => log.push(1)), __inca_native__.dispatchEvent(s, new Event('abort')), [log.length, s.aborted])",
        );

        assert_eq!(got, json!([1, false]));
    }

    #[rstest]
    #[case::window_capture("__inca_native__.windowTargetId()", true)]
    #[case::window_bubble("__inca_native__.windowTargetId()", false)]
    #[case::node_capture("__inca_native__.rootNodeId()", true)]
    fn no_other_target_hears_the_abort(#[case] target: &str, #[case] capture: bool) {
        let got = json(&format!(
            "(__inca_native__.addEventListener({target}, 'abort', () => log.push('other'), {capture}), listen(() => log.push('signal')), c.abort(), log)"
        ));

        assert_eq!(got, json!(["signal"]));
    }

    #[test]
    fn two_signals_have_different_ids() {
        let got = json(
            "__inca_native__.signalId(new AbortController().signal) !== __inca_native__.signalId(s)",
        );

        assert_eq!(got, json!(true));
    }

    #[test]
    fn the_native_signal_id_gives_null_for_anything_but_a_signal() {
        let got = json(
            "[{}, Object.create(AbortSignal.prototype), c, null, 5, 'x'].map((v) => __inca_native__.signalId(v))",
        );

        assert_eq!(got, json!([null, null, null, null, null, null]));
    }

    #[test]
    fn a_collected_signal_leaves_no_state() {
        let (engine, host) = engine_with_bindings();
        engine.eval::<()>("new AbortController();").unwrap();

        engine.with(|ctx| ctx.run_gc());
        drop(engine.run_jobs());

        assert!(!host.document.borrow().signals.contains(1));
    }

    #[test]
    fn a_collected_signal_id_is_not_reused() {
        let (engine, host) = engine_with_bindings();
        engine.eval::<()>("new AbortController();").unwrap();
        engine.with(|ctx| ctx.run_gc());
        drop(engine.run_jobs());

        engine
            .eval::<()>("globalThis.second = new AbortController().signal;")
            .unwrap();

        assert!(host.document.borrow().signals.contains(2));
    }

    #[test]
    fn a_collected_signal_drops_its_listeners_and_callback_entries() {
        let (engine, host) = engine_with_bindings();
        let id: u32 = engine
            .eval(
                r"
                (() => {
                    const s = new AbortController().signal;
                    return __inca_native__.addEventListener(s, 'abort', () => {});
                })();
                ",
            )
            .unwrap();

        engine.with(|ctx| ctx.run_gc());
        drop(engine.run_jobs());

        assert!(host.document.borrow().listeners.is_empty());
        assert!(!crate::tests::callback_stored(&engine, id));
    }

    #[test]
    fn a_collected_signal_tells_core_the_ids_it_dropped() {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                globalThis.released = [];
                globalThis.__inca_released__ = (nodes, ids) => released.push([nodes, ids]);
                globalThis.id = (() => {
                    const s = new AbortController().signal;
                    return __inca_native__.addEventListener(s, 'abort', () => {});
                })();
                ",
            )
            .unwrap();

        engine.with(|ctx| ctx.run_gc());
        drop(engine.run_jobs());

        assert_eq!(json_of(&engine, "[released, id]"), json!([[[[], [1]]], 1]));
    }

    #[test]
    fn the_class_throws_a_range_error_after_the_last_signal_id() {
        let (engine, host) = engine_with_bindings();
        host.document.borrow_mut().signals = Signals::starting_at(u32::MAX);
        engine.eval::<()>("new AbortController();").unwrap();

        assert_eq!(error_name(&engine, "new AbortController()"), "RangeError");
    }

    fn fixture() -> Clocked {
        let f = Clocked::new();
        f.engine.eval::<()>(SETUP).unwrap();
        f
    }

    fn node(f: &Clocked) -> u32 {
        f.engine.eval("node").unwrap()
    }

    fn clicks(f: &Clocked) -> Vec<u32> {
        callback_ids(
            &f.host.document.borrow().listeners,
            EventTargetId::Node(node(f)),
            "click",
        )
    }

    #[test]
    fn a_registration_with_a_signal_is_removed_at_the_abort() {
        let f = fixture();
        f.engine.eval::<()>("add(node, () => {}, s);").unwrap();

        f.engine.eval::<()>("c.abort();").unwrap();

        assert!(clicks(&f).is_empty());
    }

    #[test]
    fn one_signal_removes_listeners_of_two_nodes() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                globalThis.other = n.createNode('div');
                for (let i = 0; i < 5; i++) {
                    add(node, () => {}, s);
                    add(other, () => {}, s);
                }
                c.abort();
                ",
            )
            .unwrap();
        let other: u32 = f.engine.eval("other").unwrap();

        let left = callback_ids(
            &f.host.document.borrow().listeners,
            EventTargetId::Node(other),
            "click",
        );

        assert!(clicks(&f).is_empty() && left.is_empty());
    }

    #[test]
    fn a_capture_registration_with_a_signal_is_removed_at_the_abort() {
        let f = fixture();
        f.engine
            .eval::<()>("add(node, () => {}, s, true);")
            .unwrap();

        f.engine.eval::<()>("c.abort();").unwrap();

        assert!(clicks(&f).is_empty());
    }

    #[test]
    fn the_removal_releases_the_callback_table_entry() {
        let f = fixture();
        let id: u32 = f.engine.eval("add(node, () => {}, s)").unwrap();

        f.engine.eval::<()>("c.abort();").unwrap();

        assert!(!crate::tests::callback_stored(&f.engine, id));
    }

    #[test]
    fn core_hears_the_removed_ids_with_an_empty_node_list() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                globalThis.seen = [];
                globalThis.__inca_released__ = (nodes, ids) => seen.push([nodes, ids]);
                add(node, () => {}, s);
                c.abort();
                ",
            )
            .unwrap();

        assert_eq!(f.json("seen"), json!([[[], [1]]]));
    }

    #[test]
    fn core_hears_the_removed_ids_before_the_abort_event() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                globalThis.__inca_released__ = () => log.push('released');
                add(node, () => {}, s);
                n.addEventListener(s, 'abort', () => log.push('event'));
                c.abort();
                ",
            )
            .unwrap();

        assert_eq!(f.json("log"), json!(["released", "event"]));
    }

    #[test]
    fn a_throwing_released_hook_is_filed_and_the_abort_goes_on() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                globalThis.__inca_released__ = () => {
                    throw new Error('hook');
                };
                add(node, () => {}, s);
                n.addEventListener(s, 'abort', () => log.push('event'));
                c.abort();
                ",
            )
            .unwrap();

        assert_eq!(f.json("log"), json!(["event"]));
        assert_eq!(f.host.js.borrow().failures.len(), 1);
    }

    #[test]
    fn the_removals_run_before_the_abort_event() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                add(node, () => log.push('click'), s);
                n.addEventListener(s, 'abort', () => n.dispatchEvent(node, new Event('click')));
                c.abort();
                ",
            )
            .unwrap();

        assert_eq!(f.json("log"), json!([]));
    }

    #[test]
    fn a_listener_added_during_the_event_with_another_signal_is_removed_by_its_abort() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                globalThis.other = new AbortController();
                n.addEventListener(s, 'abort', () => add(node, () => {}, other.signal));
                c.abort();
                other.abort();
                ",
            )
            .unwrap();

        assert!(clicks(&f).is_empty());
    }

    #[test]
    fn a_listener_on_another_dependent_unpins_it_when_the_abort_empties_it() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                (() => {
                    globalThis.keep = new AbortController().signal;
                    const dependent = AbortSignal.any([
                        new AbortController().signal,
                        globalThis.keep,
                    ]);
                    n.addEventListener(dependent, 'abort', () => {}, false, false, s);
                    globalThis.w = new WeakRef(dependent);
                })();
                c.abort();
                ",
            )
            .unwrap();

        for _ in 0..2 {
            f.engine.with(|ctx| ctx.run_gc());
            drop(f.engine.run_jobs());
        }

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }

    #[test]
    fn a_listener_added_with_two_signals_in_turn_leaves_with_each_signal_alone() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                globalThis.second = new AbortController();
                const listener = () => {};
                add(node, listener, s);
                add(node, listener, second.signal);
                second.abort();
                ",
            )
            .unwrap();
        let after_second = clicks(&f).len();

        f.engine.eval::<()>("c.abort();").unwrap();

        assert_eq!([after_second, clicks(&f).len()], [1, 0]);
    }

    #[test]
    fn a_listener_added_without_a_signal_stays_after_the_abort_of_a_later_registration() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                const listener = () => {};
                add(node, listener);
                add(node, listener, s);
                c.abort();
                ",
            )
            .unwrap();

        assert_eq!(clicks(&f).len(), 1);
    }

    #[test]
    fn a_timeout_signal_removes_its_listener_when_the_timer_runs() {
        let f = fixture();
        f.engine
            .eval::<()>("add(node, () => {}, AbortSignal.timeout(5));")
            .unwrap();

        f.advance(5);

        assert!(clicks(&f).is_empty());
    }

    #[test]
    fn a_collected_signal_drops_its_links() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                (() => {
                    add(node, () => {}, new AbortController().signal);
                })();
                ",
            )
            .unwrap();

        f.engine.with(|ctx| ctx.run_gc());
        drop(f.engine.run_jobs());

        assert_eq!(f.host.document.borrow().listeners.links.len(), 0);
        assert_eq!(clicks(&f).len(), 1);
    }

    #[test]
    fn an_abort_unpins_a_signal_whose_last_link_a_sweep_dropped() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                (() => {
                    const dependent = AbortSignal.any([s]);
                    add(n.createNode('div'), () => {}, dependent);
                    globalThis.w = new WeakRef(dependent);
                })();
                globalThis.other = new AbortController();
                ",
            )
            .unwrap();
        f.host
            .document
            .borrow_mut()
            .sweep(&mut *f.host.window.borrow_mut());

        f.engine.eval::<()>("other.abort();").unwrap();
        for _ in 0..2 {
            f.engine.with(|ctx| ctx.run_gc());
            drop(f.engine.run_jobs());
        }

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }
}
