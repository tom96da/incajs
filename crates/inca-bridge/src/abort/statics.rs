// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The static methods of `AbortSignal`: `abort()`, `timeout()` and `any()`.

use std::rc::Rc;

use inca_jsenv::callbacks;
use inca_jsenv::class::define_sized_method;
use rquickjs::convert::Coerced;
use rquickjs::function::Rest;
use rquickjs::{Array, Class, Ctx, Exception, FromJs, Function, Object, Result as JsResult, Value};

use super::table::Table;
use super::{IncaSignal, abort_signal, any, dom_exception, new_signal};
use crate::bindings::Host;

/// The longest delay of the timer queue, in milliseconds.
const MAX_DELAY: f64 = 4_294_967_295.0;
/// The largest value `timeout()` accepts, `2^53 - 1`.
const MAX_SAFE: f64 = 9_007_199_254_740_991.0;

/// Defines `abort`, `timeout` and `any` on the constructor `class`.
pub(super) fn define<'js>(ctx: &Ctx<'js>, host: &Rc<Host>, class: &Object<'js>) -> JsResult<()> {
    let state = Rc::clone(host);
    define_sized_method(
        ctx,
        class,
        "abort",
        0,
        move |ctx: Ctx<'js>, Rest(args): Rest<Value<'js>>| abort(&ctx, &state, args.first()),
    )?;
    let state = Rc::clone(host);
    define_sized_method(
        ctx,
        class,
        "timeout",
        1,
        move |ctx: Ctx<'js>, Rest(args): Rest<Value<'js>>| timeout(&ctx, &state, args.first()),
    )?;
    let state = Rc::clone(host);
    define_sized_method(
        ctx,
        class,
        "any",
        1,
        move |ctx: Ctx<'js>, Rest(args): Rest<Value<'js>>| any::any(&ctx, &state, args.first()),
    )?;
    Ok(())
}

/// `AbortSignal.abort(reason)`: an aborted signal, with no event.
fn abort<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<Host>,
    reason: Option<&Value<'js>>,
) -> JsResult<Class<'js, IncaSignal<'js>>> {
    let signal = new_signal(ctx, host, &Table::open(ctx)?, false)?;
    let reason = match reason.filter(|v| !v.is_undefined()) {
        Some(reason) => reason.clone(),
        None => dom_exception(ctx, "AbortError", "signal is aborted without reason"),
    };
    host.document
        .borrow_mut()
        .signals
        .mark_aborted(signal.borrow().id());
    signal.borrow().set_reason(reason);
    Ok(signal)
}

/// The delay of `timeout()`: `[EnforceRange] unsigned long long`.
fn delay_of<'js>(ctx: &Ctx<'js>, value: Option<&Value<'js>>) -> JsResult<f64> {
    let number = match value {
        Some(value) => Coerced::<f64>::from_js(ctx, value.clone())?.0,
        None => f64::NAN,
    };
    let whole = number.trunc();
    if !whole.is_finite() || !(0.0..=MAX_SAFE).contains(&whole) {
        return Err(Exception::throw_type(
            ctx,
            "the delay is out of range. pass a number from 0 to 9007199254740991",
        ));
    }
    Ok(whole)
}

/// `AbortSignal.timeout(milliseconds)`: a signal that aborts with a `TimeoutError` after the delay.
fn timeout<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<Host>,
    delay: Option<&Value<'js>>,
) -> JsResult<Class<'js, IncaSignal<'js>>> {
    let delay = delay_of(ctx, delay)?;
    let signal = new_signal(ctx, host, &Table::open(ctx)?, false)?;
    let state = Rc::clone(host);
    let handler = Function::new(ctx.clone(), move |ctx: Ctx<'js>, signal: Value<'js>| {
        let signal = Class::<IncaSignal>::from_value(&signal)?;
        let reason = dom_exception(&ctx, "TimeoutError", "signal timed out");
        abort_signal(&ctx, &state, &Table::open(&ctx)?, &signal, Some(reason))
    })?;
    let entry = Array::new(ctx.clone())?;
    entry.set(0, handler)?;
    entry.set(1, signal.clone())?;
    let id = callbacks::store(ctx, entry.into_value())?
        .ok_or_else(|| Exception::throw_range(ctx, "timer ids are exhausted. restart the app"))?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let wait = delay.min(MAX_DELAY) as u32;
    host.js.borrow().timers.start_without_handle(id, wait);
    Ok(signal)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{Clocked, error_name, listen_ahead};

    type Fixture = Clocked;

    fn fixture() -> Fixture {
        Clocked::new()
    }

    #[test]
    fn a_static_abort_gives_the_default_reason() {
        let got = fixture().json(
            r"(() => {
                const s = AbortSignal.abort();
                return [s.aborted, s.reason.name, s.reason.code];
            })()",
        );

        assert_eq!(got, json!([true, "AbortError", 20]));
    }

    #[rstest]
    #[case::object("({})")]
    #[case::falsy("0")]
    fn a_static_abort_keeps_the_given_value(#[case] reason: &str) {
        let got = fixture().json(&format!(
            r"(() => {{
                const r = {reason};
                return Object.is(AbortSignal.abort(r).reason, r);
            }})()"
        ));

        assert_eq!(got, json!(true));
    }

    #[test]
    fn a_static_abort_with_undefined_gives_the_default_reason() {
        let got = fixture().json("AbortSignal.abort(undefined).reason.name");

        assert_eq!(got, json!("AbortError"));
    }

    #[test]
    fn throw_if_aborted_of_a_static_signal_throws() {
        let got = fixture().json(
            r"(() => {
                try {
                    AbortSignal.abort('r').throwIfAborted();
                } catch (e) {
                    return e;
                }
            })()",
        );

        assert_eq!(got, json!("r"));
    }

    #[test]
    fn a_static_abort_fires_no_event() {
        let f = fixture();
        listen_ahead(&f.engine, &f.host, 1, "() => log.push('heard')");

        let got = f.json("(AbortSignal.abort(), log)");

        assert_eq!(got, json!([]));
    }

    #[test]
    fn a_timeout_aborts_after_its_delay_and_not_before() {
        let f = fixture();
        f.engine
            .eval::<()>("globalThis.s = AbortSignal.timeout(10);")
            .unwrap();

        f.advance(9);
        let before = f.json("s.aborted");
        f.advance(1);
        let after = f.json("s.aborted");

        assert_eq!([before, after], [json!(false), json!(true)]);
    }

    #[test]
    fn a_timeout_reason_is_a_timeout_error() {
        let f = fixture();
        f.engine
            .eval::<()>("globalThis.s = AbortSignal.timeout(1);")
            .unwrap();

        f.advance(1);

        assert_eq!(
            f.json("[s.reason.name, s.reason.code, s.reason.message]"),
            json!(["TimeoutError", 23, "signal timed out"])
        );
    }

    #[rstest]
    #[case::zero("0", true)]
    #[case::text("'10'", true)]
    #[case::fraction("10.7", true)]
    #[case::small_negative("-0.5", true)]
    #[case::negative_zero("-0", true)]
    #[case::null("null", true)]
    #[case::true_("true", true)]
    #[case::largest("2 ** 53 - 1", true)]
    #[case::negative("-1", false)]
    #[case::nan("NaN", false)]
    #[case::infinity("Infinity", false)]
    #[case::too_large("2 ** 53", false)]
    #[case::huge("1e21", false)]
    #[case::undefined("undefined", false)]
    fn the_delay_follows_the_enforce_range_conversion(#[case] delay: &str, #[case] accepted: bool) {
        let f = fixture();

        let name = error_name(&f.engine, &format!("AbortSignal.timeout({delay})"));

        assert_eq!(name, if accepted { "none" } else { "TypeError" }, "{delay}");
    }

    #[test]
    fn a_timeout_with_no_argument_throws_a_type_error() {
        let f = fixture();

        assert_eq!(error_name(&f.engine, "AbortSignal.timeout()"), "TypeError");
    }

    #[test]
    fn a_delay_above_the_queue_limit_waits_the_limit() {
        let f = fixture();
        f.engine
            .eval::<()>("globalThis.s = AbortSignal.timeout(2 ** 40);")
            .unwrap();

        f.advance(4_294_967_294);
        let before = f.json("s.aborted");
        f.advance(1);

        assert_eq!([before, f.json("s.aborted")], [json!(false), json!(true)]);
    }

    #[test]
    fn a_timeout_after_the_last_timer_id_throws_a_range_error() {
        let f = fixture();
        f.engine
            .with(|ctx| inca_jsenv::callbacks::exhaust_ids(&ctx).unwrap());

        assert_eq!(
            error_name(&f.engine, "AbortSignal.timeout(5)"),
            "RangeError"
        );
        assert!(f.host.js.borrow().timers.next_due().is_none());
    }

    #[test]
    fn the_abort_runs_from_the_timer_queue_after_an_earlier_zero_timer() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                setTimeout(() => log.push('timer'), 0);
                globalThis.s = AbortSignal.timeout(0);
                __inca_native__.addEventListener(s, 'abort', () => log.push('abort'), false);
                ",
            )
            .unwrap();

        f.advance(0);

        assert_eq!(f.json("log"), json!(["timer", "abort"]));
    }

    #[test]
    fn a_timeout_started_deep_in_a_timer_chain_is_not_clamped() {
        let f = fixture();
        f.engine
            .eval::<()>(
                r"
                globalThis.chain = (depth) => {
                    if (depth < 6) return setTimeout(() => chain(depth + 1), 0);
                    globalThis.s = AbortSignal.timeout(0);
                    setTimeout(() => log.push('plain'), 0);
                };
                setTimeout(() => chain(1), 0);
                ",
            )
            .unwrap();
        for _ in 0..6 {
            f.advance(4);
        }

        f.advance(0);

        assert_eq!(f.json("[s.aborted, log.length]"), json!([true, 0]));
        f.advance(4);
        assert_eq!(f.json("log"), json!(["plain"]));
    }

    #[test]
    fn clearing_the_timer_id_leaves_the_timeout_running() {
        let f = fixture();
        f.engine
            .eval::<()>("globalThis.s = AbortSignal.timeout(5); clearTimeout(1);")
            .unwrap();

        f.advance(5);

        assert_eq!(f.json("s.aborted"), json!(true));
    }

    #[test]
    fn a_fired_timeout_signal_is_collected() {
        let f = fixture();
        f.engine
            .eval::<()>("globalThis.w = new WeakRef(AbortSignal.timeout(5));")
            .unwrap();
        f.advance(5);

        f.engine.with(|ctx| ctx.run_gc());
        drop(f.engine.run_jobs());

        assert_eq!(f.json("w.deref() === undefined"), json!(true));
    }

    /// A timeout signal with an `abort` listener and nothing else that refers to it, collected once.
    fn pending_timeout(f: &Clocked) {
        f.engine
            .eval::<()>(
                r"
                (() => {
                    const s = AbortSignal.timeout(5);
                    __inca_native__.addEventListener(s, 'abort', () => log.push('abort'));
                    globalThis.w = new WeakRef(s);
                })();
                ",
            )
            .unwrap();
        f.engine.with(|ctx| ctx.run_gc());
        drop(f.engine.run_jobs());
    }

    #[test]
    fn a_pending_timeout_signal_survives_collection() {
        let f = fixture();
        pending_timeout(&f);

        assert_eq!(f.json("w.deref() !== undefined"), json!(true));
    }

    #[test]
    fn a_pending_timeout_signal_fires_after_a_collection() {
        let f = fixture();
        pending_timeout(&f);

        f.advance(5);

        assert_eq!(f.json("log"), json!(["abort"]));
    }
}
