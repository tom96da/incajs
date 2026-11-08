// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The arguments of `mark()`, `new PerformanceMark()` and `measure()`, and the entries they make.

use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, ParamRequirement, Params};
use rquickjs::{Ctx, Exception, FromJs, Object, Result as JsResult, Value};

use super::entries::{EntryData, Kind, append, entry, prototype, text, timeline_entries};
use crate::class::{NEEDS_NEW, given, new_target_prototype, require};
use crate::time::now;

/// The time origin, which `measure()` takes as 0.
const NAVIGATION_START: &str = "navigationStart";

/// The attributes of the `PerformanceTiming` interface. A mark cannot have one of these names.
const TIMING_NAMES: [&str; 21] = [
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

/// A time in the arguments of `measure()`: a number, or the name of a mark.
enum Time {
    At(f64),
    Mark(String),
}

/// `value` converted with `ToNumber`.
fn number<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<f64> {
    Ok(Coerced::<f64>::from_js(ctx, value.clone())?.0)
}

/// `value` as a point in time: a finite number that is not negative.
fn timestamp<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<f64> {
    let time = number(ctx, value)?;
    if time.is_nan() || time.is_infinite() || time < 0.0 {
        let message = "the time is not valid. pass a finite number that is not negative";
        return Err(Exception::throw_type(ctx, message));
    }
    Ok(time)
}

/// The data of the mark `args` describe, and its detail. The arguments of `mark()` and of
/// `new PerformanceMark()`.
fn make_mark<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<(EntryData, Value<'js>)> {
    require(ctx, "mark", args, 1)?;
    let name = text(ctx, &args[0])?;
    let mut detail = None;
    let mut start_time = None;
    if let Some(options) = given(args, 1).filter(|options| !options.is_null()) {
        let options = options.as_object().ok_or_else(|| {
            let message = "the options are not an object. pass an object or leave them out";
            Exception::throw_type(ctx, message)
        })?;
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
    Ok((data, detail_or_null(ctx, detail)))
}

/// `performance.mark()`.
pub(super) fn mark<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<Value<'js>> {
    let (data, detail) = make_mark(ctx, args)?;
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
        Ok(entry(ctx, data, detail, proto)?.into_value())
    }
}

/// The time of the most recent mark named `name`. The name `navigationStart` is 0.
fn mark_time(ctx: &Ctx<'_>, name: &str) -> JsResult<f64> {
    for class in timeline_entries(ctx)?.iter().rev() {
        let data = &class.borrow().0;
        if data.kind == Kind::Mark && data.name == name {
            return Ok(data.start_time);
        }
    }
    if name == NAVIGATION_START {
        return Ok(0.0);
    }
    if TIMING_NAMES.contains(&name) {
        let message = format!("the attribute {name} is empty. use a mark name instead");
        return Err(Exception::throw_dom(ctx, "InvalidAccessError", &message));
    }
    let message = format!("the mark {name} does not exist. call mark() with that name first");
    Err(Exception::throw_dom(ctx, "SyntaxError", &message))
}

/// The number `time` stands for.
fn resolve(ctx: &Ctx<'_>, time: Time) -> JsResult<f64> {
    match time {
        Time::At(time) => Ok(time),
        Time::Mark(name) => mark_time(ctx, &name),
    }
}

/// A `start` or `end` member: a number is a time, any other value names a mark.
fn time_member<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<Time> {
    if value.is_number() {
        Ok(Time::At(timestamp(ctx, value)?))
    } else {
        Ok(Time::Mark(text(ctx, value)?))
    }
}

/// The members of the options of `measure()`.
struct MeasureOptions<'js> {
    detail: Value<'js>,
    start: Option<Time>,
    end: Option<Time>,
    duration: Option<f64>,
}

/// Reads the options object of `measure()`. `with_end_mark` tells that the call has an end mark.
fn measure_options<'js>(
    ctx: &Ctx<'js>,
    options: &Object<'js>,
    with_end_mark: bool,
) -> JsResult<MeasureOptions<'js>> {
    let detail = defined(options.get("detail")?);
    let duration = defined(options.get("duration")?);
    let end = defined(options.get("end")?);
    let start = defined(options.get("start")?);
    if with_end_mark && (start.is_some() || end.is_some() || duration.is_some()) {
        let message = "the end mark comes with start, end or duration. pass the end in the options";
        return Err(Exception::throw_type(ctx, message));
    }
    if start.is_some() && end.is_some() && duration.is_some() {
        let message = "the options hold start, end and duration. pass two of them";
        return Err(Exception::throw_type(ctx, message));
    }
    if (detail.is_some() || duration.is_some()) && start.is_none() && end.is_none() {
        let message = "the options hold no start or end. pass start or end as well";
        return Err(Exception::throw_type(ctx, message));
    }
    Ok(MeasureOptions {
        detail: detail_or_null(ctx, detail),
        start: start.map(|value| time_member(ctx, &value)).transpose()?,
        end: end.map(|value| time_member(ctx, &value)).transpose()?,
        duration: duration.map(|value| finite(ctx, &value)).transpose()?,
    })
}

/// `performance.measure()`.
pub(super) fn measure<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<Value<'js>> {
    require(ctx, "measure", args, 1)?;
    let name = text(ctx, &args[0])?;
    let end_mark = given(args, 2);
    let mut options = MeasureOptions {
        detail: detail_or_null(ctx, None),
        start: None,
        end: None,
        duration: None,
    };
    if let Some(second) = given(args, 1).filter(|second| !second.is_null()) {
        options = match second.as_object() {
            Some(object) => measure_options(ctx, object, end_mark.is_some())?,
            None => MeasureOptions {
                start: Some(Time::Mark(text(ctx, second)?)),
                ..options
            },
        };
    }
    if let Some(end_mark) = end_mark {
        options.end = Some(Time::Mark(text(ctx, end_mark)?));
    }
    let (start, end) = match (options.start, options.end, options.duration) {
        (Some(start), None, Some(duration)) => {
            let start = resolve(ctx, start)?;
            (start, start + duration)
        }
        (None, Some(end), Some(duration)) => {
            let end = resolve(ctx, end)?;
            (end - duration, end)
        }
        (start, end, _) => {
            let start = start.map_or(Ok(0.0), |time| resolve(ctx, time))?;
            let end = match end {
                Some(time) => resolve(ctx, time)?,
                None => now(ctx)?,
            };
            (start, end)
        }
    };
    let data = EntryData {
        name,
        kind: Kind::Measure,
        start_time: start,
        duration: end - start,
    };
    let entry = entry(ctx, data, options.detail, prototype(ctx, Kind::Measure)?)?.into_value();
    append(ctx, entry.clone())?;
    Ok(entry)
}

/// `value`, `None` when it is `undefined`.
fn defined(value: Value<'_>) -> Option<Value<'_>> {
    (!value.is_undefined()).then_some(value)
}

/// `detail`, or `null` when it is absent.
fn detail_or_null<'js>(ctx: &Ctx<'js>, detail: Option<Value<'js>>) -> Value<'js> {
    detail.unwrap_or_else(|| Value::new_null(ctx.clone()))
}

/// `value` as a duration: a finite number.
fn finite<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<f64> {
    let duration = number(ctx, value)?;
    if duration.is_nan() || duration.is_infinite() {
        let message = "the duration is not valid. pass a finite number";
        return Err(Exception::throw_type(ctx, message));
    }
    Ok(duration)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::class::NEEDS_NEW;
    use crate::tests::{run_after, run_at, thrown_at};

    const TIME: &str =
        "TypeError: the time is not valid. pass a finite number that is not negative";
    const FINITE: &str = "TypeError: the duration is not valid. pass a finite number";
    const NO_START: &str = "TypeError: the options hold no start or end. pass start or end as well";
    const NO_MARK_NAME: &str = "TypeError: too few arguments for mark. pass at least 1";

    /// Two marks, `s` at 10 and `e` at 20.
    const MARKS: &str = r"
        performance.mark('s', { startTime: 10 });
        performance.mark('e', { startTime: 20 });
        ";

    /// What the statements `body` throw after the marks `s` and `e`.
    fn thrown_marked(body: &str) -> String {
        thrown_at(&format!("{MARKS} {body}"))
    }

    /// `expression` evaluated after the marks `s` and `e`.
    fn after_marks(expression: &str) -> String {
        run_at(&format!(
            r"
            {MARKS}
            {expression};
            "
        ))
    }

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
        "TypeError: the options are not an object. pass an object or leave them out"
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

    #[rstest]
    #[case::object("performance.mark('a', { detail: d }).detail === d")]
    #[case::absent("performance.mark('a').detail === null")]
    #[case::undefined("performance.mark('a', { detail: undefined }).detail === null")]
    fn mark_stores_the_detail(#[case] check: &str) {
        let got = run_at(&format!(
            r"
            const d = {{}};
            String({check});
            "
        ));

        assert_eq!(got, "true");
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

    #[test]
    fn measure_without_marks_spans_the_start_of_the_clock_to_now() {
        let got = run_after(
            10,
            r"
            const m = performance.measure('m');
            JSON.stringify([
                m.entryType,
                m.startTime,
                m.duration === performance.now() && m.duration > 0,
            ]);
            ",
        );

        assert_eq!(got, r#"["measure",0,true]"#);
    }

    #[rstest]
    #[case::start_mark("measure('m', 's')", "10,-10")]
    #[case::two_marks("measure('m', 's', 'e')", "10,10")]
    #[case::end_mark("measure('m', undefined, 'e')", "0,20")]
    #[case::null_second("measure('m', null)", "0,0")]
    #[case::undefined_end_mark("measure('m', 's', undefined)", "10,-10")]
    fn measure_takes_the_start_times_of_the_marks(#[case] call: &str, #[case] want: &str) {
        let got = after_marks(&format!(
            r"
            const m = performance.{call};
            `${{m.startTime}},${{m.duration}}`;
            "
        ));

        assert_eq!(got, want);
    }

    #[test]
    fn measure_resolves_a_name_to_the_mark_of_that_name() {
        let got = after_marks(
            r"
            performance.measure('s');
            String(performance.measure('m', 's').startTime);
            ",
        );

        assert_eq!(got, "10");
    }

    #[test]
    fn measure_uses_the_most_recent_mark_of_a_name() {
        let got = run_at(
            r"
            performance.mark('s', { startTime: 10 });
            performance.mark('s', { startTime: 30 });
            String(performance.measure('m', 's').startTime);
            ",
        );

        assert_eq!(got, "30");
    }

    #[rstest]
    #[case::empty("{}", "0,0")]
    #[case::undefined_start("{ start: undefined }", "0,0")]
    #[case::end("{ end: 4 }", "0,4")]
    #[case::start("{ start: 3 }", "3,-3")]
    #[case::start_and_duration("{ start: 2, duration: 5 }", "2,5")]
    #[case::end_and_duration("{ end: 9, duration: 5 }", "4,5")]
    fn measure_options_give_the_span(#[case] options: &str, #[case] want: &str) {
        let got = run_at(&format!(
            r"
            const m = performance.measure('m', {options});
            `${{m.startTime}},${{m.duration}}`;
            "
        ));

        assert_eq!(got, want);
    }

    #[rstest]
    #[case::start("{ start: 's' }", "10,-10")]
    #[case::end("{ end: 'e' }", "0,20")]
    fn measure_options_take_mark_names(#[case] options: &str, #[case] want: &str) {
        let got = after_marks(&format!(
            r"
            const m = performance.measure('m', {options});
            `${{m.startTime}},${{m.duration}}`;
            "
        ));

        assert_eq!(got, want);
    }

    #[rstest]
    #[case::stored("performance.measure('m', { start: 1, detail: d }).detail === d")]
    #[case::default("performance.measure('m').detail === null")]
    fn measure_stores_the_detail(#[case] check: &str) {
        let got = run_at(&format!(
            r"
            const d = {{}};
            String({check});
            "
        ));

        assert_eq!(got, "true");
    }

    #[rstest]
    #[case::derived_start("{ end: 3, duration: 5 }", "-2,5")]
    #[case::string_duration("{ start: 5, duration: '3' }", "5,3")]
    #[case::start_mark_and_duration("{ start: 's', duration: 3 }", "10,3")]
    #[case::end_mark_and_duration("{ end: 'e', duration: 3 }", "17,3")]
    fn measure_derives_the_missing_time(#[case] options: &str, #[case] want: &str) {
        let got = after_marks(&format!(
            r"
            const m = performance.measure('m', {options});
            `${{m.startTime}},${{m.duration}}`;
            "
        ));

        assert_eq!(got, want);
    }

    #[rstest]
    #[case::nan_duration("{ start: 1, duration: NaN }", FINITE)]
    #[case::infinite_duration("{ start: 1, duration: Infinity }", FINITE)]
    #[case::infinite_end("{ end: Infinity }", TIME)]
    fn measure_rejects_a_non_finite_number(#[case] options: &str, #[case] message: &str) {
        assert_eq!(
            thrown_marked(&format!("performance.measure('m', {options})")),
            message
        );
    }

    #[rstest]
    #[case::undefined_detail("{ detail: undefined }")]
    #[case::undefined_duration("{ duration: undefined }")]
    fn measure_counts_an_undefined_member_as_absent(#[case] options: &str) {
        assert_eq!(
            thrown_marked(&format!("performance.measure('m', {options})")),
            "no throw"
        );
    }

    const EMPTY: &str =
        "InvalidAccessError: the attribute fetchStart is empty. use a mark name instead";

    #[rstest]
    #[case::start("performance.measure('m', 'fetchStart')")]
    #[case::end_mark("performance.measure('m', 's', 'fetchStart')")]
    fn measure_throws_for_a_timing_name_with_no_value(#[case] call: &str) {
        assert_eq!(thrown_marked(call), EMPTY);
    }

    #[rstest]
    #[case::start("String(performance.measure('m', 'navigationStart').startTime)", "0")]
    #[case::name("performance.measure('navigationStart').name", "navigationStart")]
    fn measure_takes_navigation_start(#[case] expression: &str, #[case] want: &str) {
        assert_eq!(run_at(expression), want);
    }

    #[rstest]
    #[case::start("{ end: 1e308, duration: -1e308 }", "startTime", "Infinity")]
    #[case::duration("{ start: 1e308, duration: 1e308 }", "duration", "Infinity")]
    fn measure_keeps_a_sum_that_overflows(
        #[case] options: &str,
        #[case] member: &str,
        #[case] want: &str,
    ) {
        assert_eq!(
            run_at(&format!(
                "String(performance.measure('m', {options}).{member})"
            )),
            want
        );
    }

    #[rstest]
    #[case::negative("performance.mark('fetchStart', { startTime: -1 })", TIME)]
    #[case::options_number(
        "performance.mark('fetchStart', 5)",
        "TypeError: the options are not an object. pass an object or leave them out"
    )]
    fn mark_checks_the_options_before_the_name(#[case] call: &str, #[case] message: &str) {
        assert_eq!(thrown_marked(call), message);
    }

    #[rstest]
    #[case::all_three(
        "performance.measure('m', { start: 1, end: 2, duration: 1 })",
        "TypeError: the options hold start, end and duration. pass two of them"
    )]
    #[case::end_mark_with_options(
        "performance.measure('m', { start: 1 }, 'e')",
        "TypeError: the end mark comes with start, end or duration. pass the end in the options"
    )]
    #[case::end_mark_with_end(
        "performance.measure('m', { end: 1 }, 'e')",
        "TypeError: the end mark comes with start, end or duration. pass the end in the options"
    )]
    #[case::end_mark_with_duration(
        "performance.measure('m', { duration: 1 }, 'e')",
        "TypeError: the end mark comes with start, end or duration. pass the end in the options"
    )]
    #[case::negative_start("performance.measure('m', { start: -1 })", TIME)]
    #[case::detail_alone("performance.measure('m', { detail: 1 })", NO_START)]
    #[case::duration_alone("performance.measure('m', { duration: 1 })", NO_START)]
    #[case::no_argument(
        "performance.measure()",
        "TypeError: too few arguments for measure. pass at least 1"
    )]
    fn measure_rejects_an_invalid_call(#[case] call: &str, #[case] message: &str) {
        assert_eq!(thrown_marked(call), message);
    }

    #[rstest]
    #[case::start("performance.measure('m', 'nope')", "nope")]
    #[case::end("performance.measure('m', 's', 'nope')", "nope")]
    fn measure_throws_for_a_missing_mark(#[case] call: &str, #[case] name: &str) {
        assert_eq!(
            thrown_marked(call),
            format!(
                "SyntaxError: the mark {name} does not exist. call mark() with that name first"
            )
        );
    }

    #[rstest]
    #[case::reversed_pair("{ start: 9, end: 5 }", "9,-4")]
    #[case::negative_duration("{ start: 5, duration: -1 }", "5,-1")]
    fn measure_keeps_a_negative_duration(#[case] options: &str, #[case] want: &str) {
        let got = run_at(&format!(
            r"
            const m = performance.measure('m', {options});
            `${{m.startTime}},${{m.duration}}`;
            "
        ));

        assert_eq!(got, want);
    }

    #[test]
    fn measure_reads_a_number_as_a_mark_name() {
        assert_eq!(
            thrown_marked("performance.measure('m', 5)"),
            "SyntaxError: the mark 5 does not exist. call mark() with that name first"
        );
    }
}
