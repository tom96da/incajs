// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The arguments of `measure()` and the entries it makes.

use rquickjs::{Ctx, Exception, Object, Result as JsResult, Value};

use super::entries::{EntryData, Kind, append, entry, prototype, timeline_entries};
use super::record::{NAVIGATION_START, TIMING_NAMES, cloned_detail, defined, number, timestamp};
use crate::class::{given, require, text};
use crate::time::now;

/// A time in the arguments of `measure()`: a number, or the name of a mark.
enum Time {
    At(f64),
    Mark(String),
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
    detail: Option<Value<'js>>,
    start: Option<Time>,
    end: Option<Time>,
    duration: Option<f64>,
}

/// Reads the options object of `measure()`. Each member is converted right after it is read, in
/// the order `detail`, `duration`, `end`, `start`. `with_end_mark` tells that the call has an end
/// mark.
fn measure_options<'js>(
    ctx: &Ctx<'js>,
    options: &Object<'js>,
    with_end_mark: bool,
) -> JsResult<MeasureOptions<'js>> {
    let detail = defined(options.get("detail")?);
    let duration = defined(options.get("duration")?)
        .map(|value| finite(ctx, &value))
        .transpose()?;
    let end = defined(options.get("end")?)
        .map(|value| time_member(ctx, &value))
        .transpose()?;
    let start = defined(options.get("start")?)
        .map(|value| time_member(ctx, &value))
        .transpose()?;
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
        detail,
        start,
        end,
        duration,
    })
}

/// `performance.measure()`.
pub(super) fn measure<'js>(ctx: &Ctx<'js>, args: &[Value<'js>]) -> JsResult<Value<'js>> {
    require(ctx, "measure", args, 1)?;
    let name = text(ctx, &args[0])?;
    let end_mark = given(args, 2);
    let mut options = MeasureOptions {
        detail: None,
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
    let detail = cloned_detail(ctx, options.detail)?;
    let entry = entry(ctx, data, detail, prototype(ctx, Kind::Measure)?)?.into_value();
    append(ctx, entry.clone())?;
    Ok(entry)
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

    use crate::tests::{FUNCTION_DETAIL, MARKS, TIME, run_after, run_at, thrown_marked};

    const FINITE: &str = "TypeError: the duration is not valid. pass a finite number";
    const NO_START: &str = "TypeError: the options hold no start or end. pass start or end as well";

    const EMPTY: &str =
        "InvalidAccessError: the attribute fetchStart is empty. use a mark name instead";

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

    #[test]
    fn measure_detail_is_not_the_passed_object() {
        assert_eq!(
            run_at(
                r"
                const d = {};
                String(performance.measure('m', { start: 1, detail: d }).detail !== d);
                "
            ),
            "true"
        );
    }

    #[test]
    fn a_change_of_the_passed_detail_does_not_show_in_the_measure() {
        assert_eq!(
            run_at(
                r"
                const d = {};
                const m = performance.measure('m', { start: 1, detail: d });
                d.x = 1;
                String(m.detail.x === undefined);
                "
            ),
            "true"
        );
    }

    #[test]
    fn measure_detail_defaults_to_null() {
        assert_eq!(
            run_at("String(performance.measure('m').detail === null)"),
            "true"
        );
    }

    #[test]
    fn measure_throws_for_a_function_detail() {
        assert_eq!(
            thrown_marked("performance.measure('m', { start: 0, detail: () => {} })"),
            FUNCTION_DETAIL
        );
    }

    #[test]
    fn measure_with_a_function_detail_adds_no_entry() {
        assert_eq!(
            run_at(
                r"
                try {
                    performance.measure('m', { start: 0, detail: () => {} });
                } catch (e) {}
                String(performance.getEntries().length);
                "
            ),
            "0"
        );
    }

    #[test]
    fn a_missing_mark_throws_before_the_detail_is_cloned() {
        assert_eq!(
            thrown_marked("performance.measure('m', { start: 'nomark', detail: () => {} })"),
            "SyntaxError: the mark nomark does not exist. call mark() with that name first"
        );
    }

    #[test]
    fn a_missing_start_or_end_throws_before_the_detail_is_cloned() {
        assert_eq!(
            thrown_marked("performance.measure('m', { detail: () => {} })"),
            NO_START
        );
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

    #[test]
    fn measure_converts_duration_before_it_reads_end() {
        assert_eq!(
            run_at(
                r"
                let end = 1;
                const options = {
                    duration: {
                        valueOf() {
                            end = 9;
                            return 5;
                        },
                    },
                    get end() {
                        return end;
                    },
                };
                const m = performance.measure('m', options);
                `${m.startTime},${m.duration}`;
                "
            ),
            "4,5"
        );
    }

    #[test]
    fn measure_reads_the_options_in_the_order_detail_duration_end_start() {
        assert_eq!(
            run_at(
                r"
                const log = [];
                const options = {
                    get detail() {
                        log.push('detail');
                    },
                    get duration() {
                        log.push('duration');
                        return 1;
                    },
                    get end() {
                        log.push('end');
                        return 2;
                    },
                    get start() {
                        log.push('start');
                        return 3;
                    },
                };
                try {
                    performance.measure('m', options);
                } catch (e) {
                    log.push(e.name);
                }
                log.join();
                "
            ),
            "detail,duration,end,start,TypeError"
        );
    }

    #[test]
    fn a_throwing_duration_conversion_stops_the_later_reads() {
        assert_eq!(
            run_at(
                r"
                const log = [];
                const options = {
                    get duration() {
                        return {
                            valueOf() {
                                throw new RangeError('boom');
                            },
                        };
                    },
                    get end() {
                        log.push('end');
                        return 1;
                    },
                    get start() {
                        return 2;
                    },
                };
                let text;
                try {
                    performance.measure('m', options);
                } catch (e) {
                    text = `${e.name}: ${e.message}`;
                }
                `${text}|${log.join()}`;
                "
            ),
            "RangeError: boom|"
        );
    }

    #[test]
    fn measure_throws_for_all_three_options_after_it_converts_them() {
        assert_eq!(
            run_at(
                r"
                const log = [];
                const options = {
                    get duration() {
                        log.push('get duration');
                        return {
                            valueOf() {
                                log.push('duration');
                                return 1;
                            },
                        };
                    },
                    get end() {
                        log.push('get end');
                        return {
                            toString() {
                                log.push('end');
                                return 'e';
                            },
                        };
                    },
                    get start() {
                        log.push('get start');
                        return {
                            toString() {
                                log.push('start');
                                return 's';
                            },
                        };
                    },
                };
                let name;
                try {
                    performance.measure('m', options);
                } catch (e) {
                    name = e.name;
                }
                `${log.join()}|${name}`;
                "
            ),
            "get duration,duration,get end,end,get start,start|TypeError"
        );
    }
}
