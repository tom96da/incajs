// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `AbortSignal.any()`.

use std::rc::Rc;

use inca_jsenv::class::{Method, iterate, method_of};
use rquickjs::{Class, Ctx, Exception, Result as JsResult, Value};

use super::table::Table;
use super::{IncaSignal, new_signal};
use crate::bindings::Host;

const ITERABLE: &str = "AbortSignal.any needs an iterable. pass an array of signals";
const ITEM: &str = "AbortSignal.any takes signals. pass only AbortSignal items";

/// `AbortSignal.any(signals)`: a new signal that aborts with the reason of the first source to
/// abort, or is aborted already with the reason of the first aborted source.
pub(super) fn any<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<Host>,
    signals: Option<&Value<'js>>,
) -> JsResult<Class<'js, IncaSignal<'js>>> {
    let not_iterable = || Exception::throw_type(ctx, ITERABLE);
    let iterable = signals.ok_or_else(not_iterable)?;
    let object = iterable.as_object().ok_or_else(not_iterable)?;
    let Method::Function(method) = method_of(object)? else {
        return Err(not_iterable());
    };
    let mut sources: Vec<Class<'js, IncaSignal<'js>>> = Vec::new();
    iterate(ctx, iterable, &method, false, |item| {
        let source =
            Class::<IncaSignal>::from_value(&item).map_err(|_| Exception::throw_type(ctx, ITEM))?;
        sources.push(source);
        Ok(())
    })?;
    let table = Table::open(ctx)?;
    let ids: Vec<u32> = sources.iter().map(|s| s.borrow().id()).collect();
    let made = host.document.borrow_mut().create_any_signal(&ids);
    let result = new_signal(ctx, &table, made.map(|made| made.id))?;
    let aborted_by = made.and_then(|made| made.aborted_by);
    if let Some(reason) = aborted_by.and_then(|index| sources[index].borrow().reason()) {
        result.borrow().set_reason(reason);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{Clocked, listen_ahead};

    /// A controller `c`, its signal `s`, and `hear(signal, name)` that logs the abort of a signal.
    const SETUP: &str = r"
        globalThis.c = new AbortController();
        globalThis.s = c.signal;
        globalThis.hear = (signal, name) =>
            __inca_native__.addEventListener(signal, 'abort', () => log.push(name));
        ";

    fn fixture() -> Clocked {
        let f = Clocked::new();
        f.engine.eval::<()>(SETUP).unwrap();
        f
    }

    /// Collects the engine twice, so a signal that unpins on the first pass goes on the second.
    fn collect(f: &Clocked) {
        for _ in 0..2 {
            f.engine.with(|ctx| ctx.run_gc());
            drop(f.engine.run_jobs());
        }
    }

    const ITERABLE: &str = "AbortSignal.any needs an iterable. pass an array of signals";
    const ITEM: &str = "AbortSignal.any takes signals. pass only AbortSignal items";

    /// The name and message of what `call` throws.
    fn thrown(f: &Clocked, call: &str) -> serde_json::Value {
        f.json(&format!(
            r"(() => {{
                try {{
                    {call};
                }} catch (e) {{
                    return [e.name, e.message];
                }}
                return 'none';
            }})()"
        ))
    }

    #[rstest]
    #[case::empty("[]")]
    #[case::one("[s]")]
    #[case::two("[s, new AbortController().signal]")]
    #[case::set("new Set([s])")]
    #[case::generator("(function* () { yield s; })()")]
    #[case::empty_map("new Map()")]
    fn any_takes_an_iterable_of_signals(#[case] list: &str) {
        let f = fixture();

        let got = f.json(&format!("AbortSignal.any({list}).aborted"));

        assert_eq!(got, json!(false));
    }

    #[test]
    fn an_empty_list_gives_a_signal_that_never_aborts() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const r = AbortSignal.any([]);
                c.abort();
                return [r.aborted, r !== s];
            })()",
        );

        assert_eq!(got, json!([false, true]));
    }

    #[rstest]
    #[case::undefined("AbortSignal.any(undefined)")]
    #[case::null("AbortSignal.any(null)")]
    #[case::number("AbortSignal.any(5)")]
    #[case::string("AbortSignal.any('ab')")]
    #[case::not_callable("AbortSignal.any({ [Symbol.iterator]: 5 })")]
    #[case::missing_method("AbortSignal.any({})")]
    #[case::no_argument("AbortSignal.any()")]
    fn a_bad_argument_throws_the_iterable_message(#[case] call: &str) {
        let f = fixture();

        assert_eq!(thrown(&f, call), json!(["TypeError", ITERABLE]));
    }

    #[rstest]
    #[case::pair("AbortSignal.any(new Map([[1, 2]]))")]
    #[case::null("AbortSignal.any([null])")]
    #[case::object("AbortSignal.any([{}])")]
    #[case::forged("AbortSignal.any([Object.create(AbortSignal.prototype)])")]
    fn a_bad_item_throws_the_item_message(#[case] call: &str) {
        let f = fixture();

        assert_eq!(thrown(&f, call), json!(["TypeError", ITEM]));
    }

    #[test]
    fn a_refused_item_leaves_the_iterator_open() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const iterator = {
                    [Symbol.iterator]() {
                        return this;
                    },
                    next() {
                        return { done: false, value: 5 };
                    },
                    return() {
                        log.push('closed');
                        return {};
                    },
                };
                try {
                    AbortSignal.any(iterator);
                } catch {}
                return log;
            })()",
        );

        assert_eq!(got, json!([]));
    }

    #[test]
    fn an_item_after_an_aborted_source_is_checked_too() {
        let f = fixture();

        let got = thrown(&f, "AbortSignal.any([AbortSignal.abort('r'), 5])");

        assert_eq!(got, json!(["TypeError", ITEM]));
    }

    #[test]
    fn a_generator_is_drained_even_when_its_first_source_is_aborted() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const items = (function* () {
                    try {
                        yield AbortSignal.abort('r');
                        yield s;
                    } finally {
                        log.push('drained');
                    }
                })();
                return [AbortSignal.any(items).reason, log];
            })()",
        );

        assert_eq!(got, json!(["r", ["drained"]]));
    }

    #[test]
    fn an_aborted_source_gives_an_aborted_result_with_its_reason() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const r = AbortSignal.any([s, AbortSignal.abort('first'), AbortSignal.abort('second')]);
                return [r.aborted, r.reason];
            })()",
        );

        assert_eq!(got, json!([true, "first"]));
    }

    #[test]
    fn an_aborted_source_fires_no_event_at_the_result() {
        let f = fixture();
        f.engine
            .eval::<()>("globalThis.a = AbortSignal.abort('r');")
            .unwrap();
        listen_ahead(&f.engine, &f.host, 3, "() => log.push('heard')");

        let got = f.json("(AbortSignal.any([a]), log)");

        assert_eq!(got, json!([]));
    }

    #[test]
    fn the_abort_of_a_source_aborts_the_result_with_one_event() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const r = AbortSignal.any([s]);
                hear(r, 'r');
                c.abort('why');
                return [r.reason, log];
            })()",
        );

        assert_eq!(got, json!(["why", ["r"]]));
    }

    #[test]
    fn every_dependent_reads_aborted_before_the_first_event() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const r1 = AbortSignal.any([s]);
                const r2 = AbortSignal.any([s]);
                __inca_native__.addEventListener(s, 'abort', () => log.push([r1.aborted, r2.aborted]));
                c.abort();
                return log;
            })()",
        );

        assert_eq!(got, json!([[true, true]]));
    }

    #[test]
    fn the_events_run_source_first_then_dependents_in_creation_order() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const r1 = AbortSignal.any([s]);
                const r2 = AbortSignal.any([s]);
                hear(r2, 'r2'); hear(r1, 'r1'); hear(s, 's');
                c.abort();
                return log;
            })()",
        );

        assert_eq!(got, json!(["s", "r1", "r2"]));
    }

    #[test]
    fn a_dependent_source_contributes_its_sources() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const r1 = AbortSignal.any([s]);
                const r2 = AbortSignal.any([r1]);
                c.abort('x');
                return [r1.reason, r2.reason];
            })()",
        );

        assert_eq!(got, json!(["x", "x"]));
    }

    #[test]
    fn a_dependent_ignores_the_abort_of_another_source() {
        let f = fixture();

        let got = f.json(
            r"(() => {
                const other = new AbortController();
                const r = AbortSignal.any([s, other.signal]);
                hear(r, 'r');
                c.abort('first'); other.abort('second');
                return [r.reason, log];
            })()",
        );

        assert_eq!(got, json!(["first", ["r"]]));
    }

    #[rstest]
    #[case::twice("AbortSignal.any([s, s])")]
    #[case::through_a_dependent("AbortSignal.any([s, AbortSignal.any([s])])")]
    fn a_source_that_counts_twice_aborts_the_result_once(#[case] make: &str) {
        let f = fixture();

        let got = f.json(&format!(
            r"(() => {{
                const r = {make};
                hear(r, 'r');
                c.abort();
                return log;
            }})()"
        ));

        assert_eq!(got, json!(["r"]));
    }

    #[test]
    fn a_dependent_with_a_listener_survives_collection_and_fires() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"(() => {
                    hear(AbortSignal.any([s]), 'r');
                })();",
            )
            .unwrap();

        collect(&f);

        assert_eq!(f.json("(c.abort(), log)"), json!(["r"]));
    }

    #[test]
    fn a_listener_of_an_aborted_dependent_keeps_nothing_alive() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"(() => {
                    const r = AbortSignal.any([s]);
                    c.abort();
                    hear(r, 'r');
                    globalThis.w = new WeakRef(r);
                })();",
            )
            .unwrap();

        collect(&f);

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }

    #[test]
    fn a_listener_of_a_dependent_with_no_source_keeps_nothing_alive() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"(() => {
                    const r = AbortSignal.any([]);
                    hear(r, 'r');
                    globalThis.w = new WeakRef(r);
                })();",
            )
            .unwrap();

        collect(&f);

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }

    #[test]
    fn a_dependent_that_lost_its_last_listener_is_collectable() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"(() => {
                    const r = AbortSignal.any([s]);
                    const id = __inca_native__.addEventListener(r, 'abort', () => {});
                    __inca_native__.removeEventListener(r, 'abort', id);
                    globalThis.w = new WeakRef(r);
                })();",
            )
            .unwrap();

        collect(&f);

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }

    #[test]
    fn a_dependent_with_a_click_listener_only_is_collectable() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"(() => {
                    const r = AbortSignal.any([s]);
                    __inca_native__.addEventListener(r, 'click', () => {});
                    globalThis.w = new WeakRef(r);
                })();",
            )
            .unwrap();

        collect(&f);

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }

    #[test]
    fn a_collected_dependent_leaves_no_state_and_the_source_aborts_normally() {
        let f = fixture();
        f.engine.eval::<()>("AbortSignal.any([s]);").unwrap();
        collect(&f);

        assert_eq!(f.json("(c.abort(), s.aborted)"), json!(true));
        assert!(!f.host.document.borrow().signals().contains(2));
    }

    #[test]
    fn a_dependent_whose_sources_are_all_collected_is_collectable() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"(() => {
                    const r = AbortSignal.any([new AbortController().signal]);
                    hear(r, 'r');
                    globalThis.w = new WeakRef(r);
                })();",
            )
            .unwrap();

        collect(&f);

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }

    #[test]
    fn a_timeout_source_aborts_the_dependent_with_the_timeout_error() {
        let f = fixture();
        f.engine
            .eval::<()>("globalThis.r = AbortSignal.any([AbortSignal.timeout(5), s]);")
            .unwrap();

        f.advance(5);

        assert_eq!(f.json("r.reason.name"), json!("TimeoutError"));
    }
}
