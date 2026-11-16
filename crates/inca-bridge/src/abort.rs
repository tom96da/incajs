// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `AbortController` and `AbortSignal`.
//!
//! Rust keeps the state record of each signal under its id. The signal objects live in JS and are
//! found by id through the signal table, a weak table in the vault. The abort steps remove nothing
//! outside the signal in this module: the `abort` event reaches the listeners of the signal
//! through the walk that serves nodes and `window`.

mod any;
mod classes;
mod statics;
mod table;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use inca_jsenv::callbacks;
use rquickjs::{Class, Ctx, Exception, Result as JsResult, Value};

pub(crate) use classes::IncaSignal;
use classes::install as install_classes;
pub(crate) use table::Table;

use crate::EventTargetId;
use crate::bindings::Host;

/// The state of one signal.
#[derive(Debug)]
struct State {
    aborted: bool,
    dependent: bool,
    sources: Vec<u32>,
    dependents: Vec<u32>,
}

/// The state records of the signals of a host, by id.
#[derive(Debug)]
pub(crate) struct Signals {
    next: Option<u32>,
    by_id: HashMap<u32, State>,
}

impl Default for Signals {
    fn default() -> Self {
        Self::starting_at(1)
    }
}

impl Signals {
    /// An empty set whose first id is `next`.
    pub(crate) fn starting_at(next: u32) -> Self {
        Self {
            next: Some(next),
            by_id: HashMap::new(),
        }
    }

    /// A new record under the next id, `None` after the last id.
    pub(crate) fn create(&mut self, dependent: bool) -> Option<u32> {
        let id = self.next?;
        self.next = id.checked_add(1);
        self.by_id.insert(
            id,
            State {
                aborted: false,
                dependent,
                sources: Vec::new(),
                dependents: Vec::new(),
            },
        );
        Some(id)
    }

    /// Marks `id` aborted, and every dependent that is not aborted yet. Returns those dependents,
    /// `None` when `id` is aborted already or unknown.
    pub(crate) fn mark_aborted(&mut self, id: u32) -> Option<Vec<u32>> {
        let state = self.by_id.get_mut(&id).filter(|state| !state.aborted)?;
        state.aborted = true;
        let dependents = state.dependents.clone();
        let mut pending = Vec::new();
        for dependent in dependents {
            if let Some(state) = self.by_id.get_mut(&dependent).filter(|s| !s.aborted) {
                state.aborted = true;
                pending.push(dependent);
            }
        }
        Some(pending)
    }

    /// Makes `result` depend on `sources`. A source that is dependent contributes its own
    /// sources, and each source counts once.
    pub(crate) fn link_dependent(&mut self, result: u32, sources: &[u32]) {
        let mut flat: Vec<u32> = Vec::new();
        for source in sources {
            let own = self
                .by_id
                .get(source)
                .map(|s| (s.dependent, s.sources.clone()));
            let (dependent, inherited) = own.unwrap_or_default();
            let given = if dependent { inherited } else { vec![*source] };
            for id in given {
                if !flat.contains(&id) {
                    flat.push(id);
                }
            }
        }
        for source in &flat {
            if let Some(state) = self.by_id.get_mut(source) {
                state.dependents.push(result);
            }
        }
        if let Some(state) = self.by_id.get_mut(&result) {
            state.sources = flat;
        }
    }

    /// Drops the record of `id` and its links. Returns the dependents that have no source left.
    pub(crate) fn release(&mut self, id: u32) -> Vec<u32> {
        let Some(state) = self.by_id.remove(&id) else {
            return Vec::new();
        };
        for source in state.sources {
            if let Some(source) = self.by_id.get_mut(&source) {
                source.dependents.retain(|d| *d != id);
            }
        }
        let mut orphaned = Vec::new();
        for dependent in state.dependents {
            if let Some(dependent_state) = self.by_id.get_mut(&dependent) {
                dependent_state.sources.retain(|s| *s != id);
                if dependent_state.sources.is_empty() {
                    orphaned.push(dependent);
                }
            }
        }
        orphaned
    }

    pub(crate) fn is_aborted(&self, id: u32) -> bool {
        self.by_id.get(&id).is_some_and(|s| s.aborted)
    }

    pub(crate) fn is_dependent(&self, id: u32) -> bool {
        self.by_id.get(&id).is_some_and(|s| s.dependent)
    }

    fn has_source(&self, id: u32) -> bool {
        self.by_id.get(&id).is_some_and(|s| !s.sources.is_empty())
    }

    /// Whether an `abort` listener on `id` keeps the signal alive: a dependent that is not
    /// aborted and has a source left.
    pub(crate) fn pins(&self, id: u32) -> bool {
        self.is_dependent(id) && !self.is_aborted(id) && self.has_source(id)
    }
}

/// Defines `AbortController` and `AbortSignal` and the signal table.
///
/// # Errors
///
/// Returns an error if the node interfaces are not installed or a definition fails.
pub(crate) fn install(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
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
    host: &Rc<RefCell<Host>>,
    table: &Table<'js>,
    dependent: bool,
) -> JsResult<Class<'js, IncaSignal<'js>>> {
    let id =
        host.borrow_mut().signals.create(dependent).ok_or_else(|| {
            Exception::throw_range(ctx, "signal ids are exhausted. restart the app")
        })?;
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

/// Aborts `signal` with `reason`, or an `AbortError` when `reason` is `None`: the signal and its
/// dependents take the reason, then the abort event runs for each. A signal aborted already stays
/// as it is.
///
/// # Errors
///
/// Returns an error when the engine fails, not when a listener throws.
pub(crate) fn abort_signal<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    table: &Table<'js>,
    signal: &Class<'js, IncaSignal<'js>>,
    reason: Option<Value<'js>>,
) -> JsResult<()> {
    let id = signal.borrow().id();
    let Some(dependents) = host.borrow_mut().signals.mark_aborted(id) else {
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

/// Runs the abort steps of signal `id`: fires the event and ends the strong hold.
fn run_abort_steps(
    ctx: &Ctx<'_>,
    host: &Rc<RefCell<Host>>,
    table: &Table<'_>,
    id: u32,
) -> JsResult<()> {
    if table.find(id)?.is_some() {
        crate::event::fire_abort(ctx, host, id);
    }
    table.unpin(id)
}

/// Ends the strong hold of signal `id` when nothing needs it any more: it is dependent with no
/// source left, or no `abort` listener is left on it.
pub(crate) fn after_removal(host: &Host, table: &Table<'_>, id: u32) -> JsResult<()> {
    let listened = host
        .listeners
        .has_listener(EventTargetId::Signal(id), "abort");
    if host.signals.pins(id) && listened {
        Ok(())
    } else {
        table.unpin(id)
    }
}

/// Drops everything of the collected signal `id`: its state, its own registrations and the
/// callback table entries, and tells core.
pub(crate) fn release_signal(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>, id: u32) -> JsResult<()> {
    let table = Table::open(ctx)?;
    let ids = {
        let mut host = host.borrow_mut();
        let orphaned = host.signals.release(id);
        let ids = host.listeners.release_target(EventTargetId::Signal(id));
        for dependent in orphaned {
            after_removal(&host, &table, dependent)?;
        }
        table.forget(id)?;
        ids
    };
    callbacks::release(ctx, &ids)?;
    crate::dispatch::notify_released(ctx, host, &[], &ids);
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use super::Signals;
    use crate::test_support::{engine_with_bindings, error_name, json_of};

    /// A controller `c`, its signal `s`, a log and `listen(fn, capture)` on the signal.
    const SETUP: &str = r"
        globalThis.log = [];
        globalThis.c = new AbortController();
        globalThis.s = c.signal;
        globalThis.listen = (fn, capture = false) =>
            __inca_native__.addEventListener(s, 'abort', fn, capture);
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
        assert_eq!(host.borrow().failures.len(), 1);
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

        assert!(!host.borrow().signals.by_id.contains_key(&1));
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

        assert!(host.borrow().signals.by_id.contains_key(&2));
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

        assert!(host.borrow().listeners.is_empty());
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
    fn signals_run_out_of_ids_after_the_last_one() {
        let mut signals = Signals::starting_at(u32::MAX);

        assert_eq!(signals.create(false), Some(u32::MAX));
        assert_eq!(signals.create(false), None);
    }

    #[test]
    fn the_class_throws_a_range_error_after_the_last_signal_id() {
        let (engine, host) = engine_with_bindings();
        host.borrow_mut().signals = Signals::starting_at(u32::MAX);
        engine.eval::<()>("new AbortController();").unwrap();

        assert_eq!(error_name(&engine, "new AbortController()"), "RangeError");
    }

    #[test]
    fn a_source_that_appears_twice_is_linked_once() {
        let mut signals = Signals::default();
        let (a, b) = (
            signals.create(false).unwrap(),
            signals.create(true).unwrap(),
        );

        signals.link_dependent(b, &[a, a]);

        assert_eq!(signals.by_id[&a].dependents, [b]);
        assert_eq!(signals.by_id[&b].sources, [a]);
        assert_eq!(signals.mark_aborted(a), Some(vec![b]));
    }

}
