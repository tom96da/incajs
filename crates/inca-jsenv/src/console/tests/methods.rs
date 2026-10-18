// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn every_method_writes_at_its_own_level() {
    let written = lines_written_by(
        r"
        console.trace('a'); console.debug('a'); console.log('a');
        console.info('a'); console.warn('a'); console.error('a');
        ",
    );

    let levels: Vec<Level> = written.iter().map(|(level, _)| *level).collect();
    assert_eq!(
        levels,
        vec![
            Level::Trace,
            Level::Debug,
            Level::Log,
            Level::Info,
            Level::Warn,
            Level::Error
        ]
    );
}

#[test]
fn logging_nothing_writes_an_empty_line() {
    let written = lines_written_by("console.log();");

    assert_eq!(written.len(), 1);
    assert_eq!(written[0].1, "");
}

#[test]
fn symbols_mark_error_warn_and_info_only() {
    assert_eq!(
        texts(
            r"
            console.error('e'); console.warn('w'); console.info('i');
            console.log('l'); console.debug('d'); console.dirxml('x');
            "
        ),
        ["✖ e", "⚠ w", "ℹ i", "l", "d", "x"]
    );
}

#[test]
fn a_multi_line_body_lines_up_under_the_first_lines_text() {
    assert_eq!(
        texts("console.error('a\\nb'); console.group('g'); console.warn('c\\nd');"),
        ["✖ a\n  b", "▼ g", "│ ⚠ c\n│   d"]
    );
}

#[test]
fn dirxml_writes_at_log_level() {
    assert_eq!(lines_written_by("console.dirxml(1);")[0].0, Level::Log);
}

#[test]
fn assert_is_silent_when_the_condition_holds() {
    assert!(
        texts("console.assert(true, 'no'); console.assert(1); console.assert('x');").is_empty()
    );
}

#[test]
fn assert_treats_a_missing_or_falsy_condition_as_failed() {
    assert_eq!(
        texts("console.assert(); console.assert(0); console.assert(''); console.assert(null);"),
        ["✖ Assertion failed"; 4]
    );
}

#[test]
fn assert_joins_a_string_first_datum_with_a_colon() {
    assert_eq!(
        texts("console.assert(false, 'bad', 2);"),
        ["✖ Assertion failed: bad 2"]
    );
}

#[test]
fn assert_puts_non_string_data_after_the_message() {
    assert_eq!(
        texts("console.assert(false, { a: 1 }, 'x');"),
        ["✖ Assertion failed { a: 1 } x"]
    );
}

#[test]
fn count_runs_per_label_and_defaults_to_default() {
    assert_eq!(
        texts(
            "console.count(); console.count('a'); console.count(); console.count('a'); console.count(undefined);"
        ),
        ["default: 1", "a: 1", "default: 2", "a: 2", "default: 3"]
    );
}

#[test]
fn count_reset_restarts_the_count() {
    assert_eq!(
        texts(
            "console.count('a'); console.count('a'); console.countReset('a'); console.count('a');"
        ),
        ["a: 1", "a: 2", "a: 1"]
    );
    assert_eq!(
        texts("console.count(); console.countReset(); console.count();"),
        ["default: 1", "default: 1"]
    );
}

#[test]
fn count_reset_warns_about_an_unknown_label() {
    let written = lines_written_by("console.countReset('nope'); console.countReset();");

    assert_eq!(
        written,
        [
            (Level::Warn, "⚠ Count for 'nope' does not exist".to_owned()),
            (
                Level::Warn,
                "⚠ Count for 'default' does not exist".to_owned()
            ),
        ]
    );
}

#[test]
fn time_log_prints_the_label_and_a_millisecond_figure() {
    let lines = texts("console.time('t'); console.timeLog('t'); console.timeLog('t');");

    assert_eq!(lines.len(), 2);
    assert_eq!(without_elapsed(&lines[0]), "t: Nms");
    assert_eq!(without_elapsed(&lines[1]), "t: Nms");
}

#[test]
fn time_log_appends_extra_data() {
    let lines = texts("console.time(); console.timeLog('default', 'x', { a: 1 });");

    assert_eq!(without_elapsed(&lines[0]), "default: Nms x { a: 1 }");
}

#[test]
fn time_end_prints_then_forgets_the_timer() {
    let written = lines_written_by(
        "console.time('t'); console.timeEnd('t'); console.timeEnd('t'); console.timeLog('t');",
    );

    assert_eq!(without_elapsed(&written[0].1), "t: Nms");
    assert_eq!(written[0].0, Level::Log);
    assert_eq!(
        written[1..],
        [
            (Level::Warn, "⚠ Timer 't' does not exist".to_owned()),
            (Level::Warn, "⚠ Timer 't' does not exist".to_owned()),
        ]
    );
}

#[test]
fn a_duplicate_time_warns_and_keeps_the_first_timer() {
    let written = lines_written_by("console.time('t'); console.time('t'); console.timeEnd('t');");

    assert_eq!(
        written[0],
        (Level::Warn, "⚠ Timer 't' already exists".to_owned())
    );
    assert_eq!(written.len(), 2);
}

#[test]
fn an_unknown_time_label_warns_for_both_log_and_end() {
    assert_eq!(
        texts("console.timeLog('x'); console.timeEnd();"),
        [
            "⚠ Timer 'x' does not exist",
            "⚠ Timer 'default' does not exist"
        ]
    );
}

#[test]
fn groups_indent_what_follows_and_nest() {
    assert_eq!(
        texts(
            r"
            console.group('a'); console.log('1'); console.group('b'); console.log('2');
            console.groupEnd(); console.log('3'); console.groupEnd(); console.log('4');
            "
        ),
        ["▼ a", "│ 1", "│ ▼ b", "│ │ 2", "│ 3", "4"]
    );
}

#[test]
fn a_collapsed_group_has_its_own_marker_and_still_indents() {
    assert_eq!(
        texts("console.groupCollapsed('c'); console.log('x');"),
        ["▶ c", "│ x"]
    );
}

#[test]
fn a_group_label_is_formatted_like_log_arguments() {
    assert_eq!(texts("console.group('a', 1, true);"), ["▼ a 1 true"]);
}

#[test]
fn a_group_without_a_label_is_named_after_itself() {
    assert_eq!(
        texts("console.group(); console.groupCollapsed();"),
        ["▼ console.group", "│ ▶ console.group"]
    );
}

#[test]
fn group_end_with_no_open_group_does_nothing() {
    assert_eq!(
        texts(
            "console.groupEnd(); console.groupEnd(); console.log('x'); console.group('g'); console.groupEnd(); console.groupEnd(); console.log('y');"
        ),
        ["x", "▼ g", "y"]
    );
}

#[test]
fn warnings_and_symbols_sit_behind_the_guides() {
    assert_eq!(
        texts("console.group('g'); console.countReset('q');"),
        ["▼ g", "│ ⚠ Count for 'q' does not exist"]
    );
}

#[test]
fn clear_closes_every_group() {
    assert_eq!(
        texts("console.group('a'); console.group('b'); console.clear(); console.log('x');"),
        ["▼ a", "│ ▼ b", "x"]
    );
}

#[test]
fn clear_keeps_counters_and_timers() {
    let lines = texts(
        "console.count(); console.time(); console.clear(); console.count(); console.timeEnd();",
    );

    assert_eq!(lines[0], "default: 1");
    assert_eq!(lines[1], "default: 2");
    assert_eq!(without_elapsed(&lines[2]), "default: Nms");
}

#[test]
fn trace_prints_the_message_then_the_callers_frames() {
    let written = lines_written_by(
        "function inner() { console.trace('hi', 1); }\nfunction outer() { inner(); }\nouter();",
    );

    assert_eq!(written.len(), 1);
    assert_eq!(written[0].0, Level::Trace);
    let lines: Vec<&str> = written[0].1.lines().collect();
    assert_eq!(lines[0], "Trace: hi 1");
    assert!(lines[1].contains("at inner "), "{lines:?}");
    assert!(lines[2].contains("at outer "), "{lines:?}");
    assert!(
        !lines.iter().any(|l| l.contains("console.trace")),
        "{lines:?}"
    );
}

#[test]
fn trace_frames_read_at_their_source() {
    let body = crate::sourcemap::mapped_body("console.trace('hi');");
    let written = written_by(false, |ctx| {
        let f = crate::compile_async_function(ctx, &[], &body, "virtual.js").unwrap();
        ctx.globals().set("f", f).unwrap();
        ctx.eval::<(), _>("f();").unwrap();
    });

    let text = &written[0].1;
    assert!(text.contains("src/app.ts:12:5"), "{text}");
    assert!(!text.contains("virtual.js:"), "{text}");
}

#[test]
fn trace_without_data_is_just_the_word() {
    let text = &texts("console.trace();")[0];

    assert_eq!(text.lines().next(), Some("Trace"));
}

#[test]
fn trace_stack_lines_follow_the_group_guides() {
    let text = &texts("console.group('g'); console.trace('x');")[1];

    assert!(text.lines().all(|l| l.starts_with("│ ")), "{text}");
}

#[test]
fn empty_lines_carry_no_trailing_whitespace() {
    assert_eq!(
        texts(
            r"
            console.group(''); console.log(''); console.log('a\n\nb');
            console.error(''); console.error('a\n\nb'); console.log();
            "
        ),
        ["▼", "│", "│ a\n│\n│ b", "│ ✖", "│ ✖ a\n│\n│   b", "│"]
    );
    assert_eq!(texts("console.log('');"), [""]);
}

#[test]
fn install_fails_in_a_realm_without_the_intrinsics_and_leaves_nothing_pending() {
    let runtime = Runtime::new().unwrap();
    let context = Context::custom::<rquickjs::context::intrinsic::Eval>(&runtime).unwrap();
    context.with(|ctx| {
        let output: Output = Rc::new(|_, _| {});

        assert!(install(&ctx, &output, false).is_err());
        let thrown = format!("{:?}", ctx.catch());
        assert!(thrown.contains("Date is not defined"), "{thrown}");
        assert!(!ctx.has_exception());
    });
}

#[test]
fn a_context_keeps_printing_after_another_context_is_dropped() {
    let written = Rc::new(RefCell::new(Vec::new()));
    let output: Output = {
        let sink = Rc::clone(&written);
        Rc::new(move |_, message| sink.borrow_mut().push(message.to_owned()))
    };
    let runtime = Runtime::new().unwrap();
    let first = Context::full(&runtime).unwrap();
    let second = Context::full(&runtime).unwrap();
    first.with(|ctx| install(&ctx, &output, false).unwrap());
    second.with(|ctx| install(&ctx, &output, false).unwrap());
    drop(first);
    runtime.run_gc();

    second.with(|ctx| {
        ctx.eval::<(), _>("console.log(new Date(0), new Map([[1, 2]]), /x/g, new Set([3]));")
            .unwrap();
        assert!(!ctx.has_exception());
    });
    assert_eq!(
        written.take(),
        ["1970-01-01T00:00:00.000Z Map(1) { 1 => 2 } /x/g Set(1) { 3 }"]
    );
}
