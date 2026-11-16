// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The arguments of `mark()` and `new PerformanceMark()`, and the entries they make.

use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, ParamRequirement, Params};
use rquickjs::{Ctx, Exception, FromJs, Result as JsResult, Value};

use super::entries::{EntryData, Kind, append, entry, prototype};
use crate::class::{NEEDS_NEW, new_target_prototype, options_of, require, text};
use crate::structured_clone::clone_value;
use crate::time::now;

/// The time origin, which `measure()` takes as 0.
pub(super) const NAVIGATION_START: &str = "navigationStart";

/// The attributes of the `PerformanceTiming` interface. A mark cannot have one of these names.
pub(super) const TIMING_NAMES: [&str; 21] = [
    NAVIGATION_START,
    "unloadEventStart",
    "unloadEventEnd",
    "redirectStart",
    "redirectEnd",
    "fetchStart",
    "domainLookupStart",
    "domainLookupEnd",
    "connectStart",
    "connectEnd",
    "secureConnectionStart",
    "requestStart",
    "responseStart",
    "responseEnd",
    "domLoading",
    "domInteractive",
    "domContentLoadedEventStart",
    "domContentLoadedEventEnd",
    "domComplete",
    "loadEventStart",
    "loadEventEnd",
];

/// `value` converted with `ToNumber`.
pub(super) fn number<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<f64> {
    Ok(Coerced::<f64>::from_js(ctx, value.clone())?.0)
}

/// `value` as a point in time: a finite number that is not negative.
pub(super) fn timestamp<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<f64> {
    let time = number(ctx, value)?;
    if time.is_nan() || time.is_infinite() || time < 0.0 {
        let message = "the time is not valid. pass a finite number that is not negative";
        return Err(Exception::throw_type(ctx, message));
    }
    Ok(time)
}

/// The data of the mark `args` describe, and its detail as passed. The arguments of `mark()` and of
/// `new PerformanceMark()`.
fn make_mark<'js>(
    ctx: &Ctx<'js>,
    args: &[Value<'js>],
) -> JsResult<(EntryData, Option<Value<'js>>)> {
    require(ctx, "mark", args, 1)?;
    let name = text(ctx, &args[0])?;
    let mut detail = None;
    let mut start_time = None;
    if let Some(options) = options_of(ctx, args.get(1).cloned())? {
        detail = defined(options.get("detail")?);
        let value: Value = options.get("startTime")?;
        if !value.is_undefined() {
            start_time = Some(timestamp(ctx, &value)?);
        }
    }
    if TIMING_NAMES.contains(&name.as_str()) {
        let message = format!("the name {name} is reserved. use another name");
        return Err(Exception::throw_dom(ctx, "SyntaxError", &message));
    }
    let start_time = match start_time {
        Some(time) => time,
        None => now(ctx)?,
    };
    let data = EntryData {
        name,
        kind: Kind::Mark,
        start_time,
        duration: 0.0,
    };
    Ok((data, detail))
}

/// `performance.mark()`.
pub(super) fn mark<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<Value<'js>> {
    let (data, detail) = make_mark(ctx, args)?;
    let detail = cloned_detail(ctx, detail)?;
    let entry = entry(ctx, data, detail, prototype(ctx, Kind::Mark)?)?.into_value();
    append(ctx, entry.clone())?;
    Ok(entry)
}

/// The body of the `PerformanceMark` constructor.
pub(super) struct MarkConstructor;

impl<'js> IntoJsFunc<'js, ()> for MarkConstructor {
    fn param_requirements() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
        let ctx = params.ctx();
        if !params.is_constructor() {
            return Err(Exception::throw_type(ctx, NEEDS_NEW));
        }
        let args: Vec<_> = (0..params.len()).filter_map(|i| params.arg(i)).collect();
        let (data, detail) = make_mark(ctx, &args)?;
        let proto = new_target_prototype(&params, prototype(ctx, Kind::Mark)?)?;
        let detail = cloned_detail(ctx, detail)?;
        Ok(entry(ctx, data, detail, proto)?.into_value())
    }
}

/// `value`, `None` when it is `undefined`.
pub(super) fn defined(value: Value<'_>) -> Option<Value<'_>> {
    (!value.is_undefined()).then_some(value)
}

/// The clone of `detail`, or `null` when it is absent.
///
/// # Errors
///
/// Returns a `DataCloneError` for a detail that cannot be cloned.
pub(super) fn cloned_detail<'js>(
    ctx: &Ctx<'js>,
    detail: Option<Value<'js>>,
) -> JsResult<Value<'js>> {
    match detail {
        Some(detail) => clone_value(ctx, &detail, &[]),
        None => Ok(Value::new_null(ctx.clone())),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::class::{NEEDS_NEW, NOT_AN_OBJECT};
    use crate::tests::{FUNCTION_DETAIL, TIME, run_after, run_at, thrown_marked};

    const NO_MARK_NAME: &str = "TypeError: too few arguments for mark. pass at least 1";

    #[test]
    fn mark_constructor_without_new_throws() {
        assert_eq!(
            thrown_marked("PerformanceMark('a')"),
            format!("TypeError: {NEEDS_NEW}")
        );
    }

    #[test]
    fn mark_returns_an_entry_that_starts_now() {
        let got = run_after(
            10,
            r"
            const m = performance.mark('a');
            JSON.stringify([
                m.entryType,
                m.name,
                m.startTime === performance.now() && m.startTime > 0,
                m.duration,
            ]);
            ",
        );

        assert_eq!(got, r#"["mark","a",true,0]"#);
    }

    #[rstest]
    #[case::no_argument("performance.mark()", NO_MARK_NAME)]
    #[case::negative("performance.mark('a', { startTime: -1 })", TIME)]
    #[case::nan("performance.mark('a', { startTime: NaN })", TIME)]
    #[case::infinite("performance.mark('a', { startTime: Infinity })", TIME)]
    #[case::options_number(
        "performance.mark('a', 5)",
        &format!("TypeError: {NOT_AN_OBJECT}")
    )]
    fn mark_rejects_an_invalid_call(#[case] call: &str, #[case] message: &str) {
        assert_eq!(thrown_marked(call), message);
    }

    #[rstest]
    #[case::zero("{ startTime: 0 }", "0")]
    #[case::string("{ startTime: '7' }", "7")]
    #[case::null_options("null", "5")]
    fn mark_takes_the_start_time(#[case] options: &str, #[case] start: &str) {
        let got = run_after(
            5,
            &format!("performance.mark('a', {options}).startTime.toFixed(0)"),
        );

        assert_eq!(got, start);
    }

    #[rstest]
    #[case::undefined("undefined", "undefined")]
    #[case::object("{}", "[object Object]")]
    #[case::number("5", "5")]
    fn mark_converts_the_name_to_a_string(#[case] name: &str, #[case] want: &str) {
        assert_eq!(run_at(&format!("performance.mark({name}).name")), want);
    }

    #[rstest]
    #[case::minus_zero("{ startTime: -0 }", "Object.is(m.startTime, -0)")]
    #[case::null("{ startTime: null }", "Object.is(m.startTime, 0)")]
    #[case::undefined(
        "{ startTime: undefined }",
        "m.startTime === performance.now() && m.startTime > 0"
    )]
    fn mark_reads_the_start_time_option_edges(#[case] options: &str, #[case] check: &str) {
        let got = run_after(
            7,
            &format!(
                r"
                const m = performance.mark('a', {options});
                String({check});
                "
            ),
        );

        assert_eq!(got, "true");
    }

    #[test]
    fn mark_detail_is_not_the_passed_object() {
        assert_eq!(
            run_at(
                r"
                const d = {};
                String(performance.mark('a', { detail: d }).detail !== d);
                "
            ),
            "true"
        );
    }

    #[test]
    fn a_change_of_the_passed_detail_does_not_show_in_the_mark() {
        assert_eq!(
            run_at(
                r"
                const d = {};
                const m = performance.mark('a', { detail: d });
                d.x = 1;
                String(m.detail.x === undefined);
                "
            ),
            "true"
        );
    }

    #[rstest]
    #[case::absent("performance.mark('a').detail === null")]
    #[case::undefined("performance.mark('a', { detail: undefined }).detail === null")]
    fn mark_detail_defaults_to_null(#[case] check: &str) {
        assert_eq!(run_at(&format!("String({check})")), "true");
    }

    #[test]
    fn mark_detail_that_is_a_primitive_reads_as_the_value() {
        assert_eq!(
            run_at("String(performance.mark('a', { detail: 5 }).detail)"),
            "5"
        );
    }

    #[test]
    fn mark_detail_reads_as_one_object() {
        assert_eq!(
            run_at(
                r"
                const m = performance.mark('a', { detail: {} });
                String(m.detail === m.detail);
                "
            ),
            "true"
        );
    }

    #[test]
    fn mark_throws_for_a_function_detail() {
        assert_eq!(
            thrown_marked("performance.mark('a', { detail: () => {} })"),
            FUNCTION_DETAIL
        );
    }

    #[test]
    fn mark_with_a_function_detail_adds_no_entry() {
        assert_eq!(
            run_at(
                r"
                try {
                    performance.mark('a', { detail: () => {} });
                } catch (e) {}
                String(performance.getEntries().length);
                "
            ),
            "0"
        );
    }

    #[test]
    fn the_constructor_throws_for_a_function_detail() {
        assert_eq!(
            thrown_marked("new PerformanceMark('b', { detail: () => {} })"),
            FUNCTION_DETAIL
        );
    }

    #[test]
    fn the_constructor_stores_a_clone_of_the_detail() {
        assert_eq!(
            run_at(
                r"
                const d = {};
                String(new PerformanceMark('b', { detail: d }).detail !== d);
                "
            ),
            "true"
        );
    }

    #[test]
    fn a_reserved_name_throws_before_the_detail_is_cloned() {
        assert_eq!(
            thrown_marked("performance.mark('fetchStart', { detail: () => {} })"),
            "SyntaxError: the name fetchStart is reserved. use another name"
        );
    }

    #[test]
    fn a_bad_start_time_throws_before_the_detail_is_cloned() {
        assert_eq!(
            thrown_marked("performance.mark('a', { startTime: -1, detail: () => {} })"),
            TIME
        );
    }

    #[test]
    fn the_prototype_of_new_target_is_read_before_the_detail_is_cloned() {
        assert_eq!(
            run_at(
                r"
                const C = new Proxy(function () {}, {
                    get(target, key) {
                        if (key === 'prototype') {
                            throw new RangeError('prototype');
                        }
                        return target[key];
                    },
                });
                try {
                    Reflect.construct(PerformanceMark, ['x', { detail: () => {} }], C);
                } catch (e) {
                    e.message;
                }
                "
            ),
            "prototype"
        );
    }

    #[rstest]
    #[case::navigation_start("navigationStart")]
    #[case::unload_event_start("unloadEventStart")]
    #[case::unload_event_end("unloadEventEnd")]
    #[case::redirect_start("redirectStart")]
    #[case::redirect_end("redirectEnd")]
    #[case::fetch_start("fetchStart")]
    #[case::domain_lookup_start("domainLookupStart")]
    #[case::domain_lookup_end("domainLookupEnd")]
    #[case::connect_start("connectStart")]
    #[case::connect_end("connectEnd")]
    #[case::secure_connection_start("secureConnectionStart")]
    #[case::request_start("requestStart")]
    #[case::response_start("responseStart")]
    #[case::response_end("responseEnd")]
    #[case::dom_loading("domLoading")]
    #[case::dom_interactive("domInteractive")]
    #[case::dom_content_loaded_event_start("domContentLoadedEventStart")]
    #[case::dom_content_loaded_event_end("domContentLoadedEventEnd")]
    #[case::dom_complete("domComplete")]
    #[case::load_event_start("loadEventStart")]
    #[case::load_event_end("loadEventEnd")]
    fn mark_throws_for_a_timing_name(#[case] name: &str) {
        assert_eq!(
            thrown_marked(&format!("performance.mark('{name}')")),
            format!("SyntaxError: the name {name} is reserved. use another name")
        );
    }

    #[test]
    fn mark_accepts_a_name_that_extends_a_timing_name() {
        assert_eq!(
            run_at("performance.mark('fetchStarts').name"),
            "fetchStarts"
        );
    }

    #[rstest]
    #[case::negative("new PerformanceMark('a', { startTime: -1 })", TIME)]
    #[case::no_name("new PerformanceMark()", NO_MARK_NAME)]
    #[case::timing_name(
        "new PerformanceMark('fetchStart')",
        "SyntaxError: the name fetchStart is reserved. use another name"
    )]
    fn mark_constructor_throws(#[case] call: &str, #[case] message: &str) {
        assert_eq!(thrown_marked(call), message);
    }

    #[test]
    fn mark_constructor_leaves_the_timeline_empty() {
        let got = run_at(
            r"
            new PerformanceMark('a');
            String(performance.getEntries().length);
            ",
        );

        assert_eq!(got, "0");
    }

    #[test]
    fn mark_constructor_takes_the_prototype_from_new_target() {
        let got = run_at(
            r"
            class Later extends PerformanceMark {}
            String(new Later('a') instanceof Later);
            ",
        );

        assert_eq!(got, "true");
    }

    #[rstest]
    #[case::negative("performance.mark('fetchStart', { startTime: -1 })", TIME)]
    #[case::options_number(
        "performance.mark('fetchStart', 5)",
        &format!("TypeError: {NOT_AN_OBJECT}")
    )]
    fn mark_checks_the_options_before_the_name(#[case] call: &str, #[case] message: &str) {
        assert_eq!(thrown_marked(call), message);
    }
}
