// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn a_label_that_cannot_be_stringified_falls_back_to_default() {
    assert_eq!(texts("console.count(Symbol('s'));"), ["default: 1"]);
}

#[test]
fn a_label_whose_to_string_throws_falls_back_to_default() {
    assert_eq!(
        texts("console.count({ toString() { throw new Error('no'); } });"),
        ["default: 1"]
    );
}

#[test]
fn a_non_string_label_is_stringified() {
    assert_eq!(
        texts("console.count(7); console.count(null);"),
        ["7: 1", "null: 1"]
    );
}

#[test]
fn every_formatting_method_substitutes_specifiers() {
    let written = lines_written_by(
        "console.log('%s=%d', 'a', 1); console.debug('%s', 'x', 'y'); \
         console.info('%o', 'i'); console.warn('%f', '1.5'); \
         console.error('%s', Symbol('e')); console.dirxml('%i', '7.9');",
    );

    assert_eq!(
        written,
        [
            (Level::Log, "a=1".to_owned()),
            (Level::Debug, "x y".to_owned()),
            (Level::Info, "ℹ 'i'".to_owned()),
            (Level::Warn, "⚠ 1.5".to_owned()),
            (Level::Error, "✖ Symbol(e)".to_owned()),
            (Level::Log, "7".to_owned()),
        ]
    );
}

#[test]
fn a_lone_argument_is_never_formatted() {
    assert_eq!(
        texts(
            "console.log('%s %%'); console.error('%d'); console.group('%c'); console.trace('%o');"
        )
        .iter()
        .map(|t| t.lines().next().unwrap().to_owned())
        .collect::<Vec<_>>(),
        ["%s %%", "✖ %d", "▼ %c", "│ Trace: %o"]
    );
}

#[test]
fn formatting_follows_the_group_guides_and_symbols() {
    assert_eq!(
        texts("console.group('g'); console.warn('%s\\n%s', 'a', 'b');"),
        ["▼ g", "│ ⚠ a\n│   b"]
    );
}

#[test]
fn assert_formats_its_message_and_data() {
    assert_eq!(
        texts(
            "console.assert(false, '%s is %d', 'x', 4, 'end'); \
             console.assert(false, 'plain %s'); \
             console.assert(false, '%o', 'q'); \
             console.assert(false, '%s', 'a'); \
             console.assert(false, 7, '%s', 'b'); \
             console.assert(false, { a: 1 }); \
             console.assert(false, 'v', '%s');"
        ),
        [
            "✖ Assertion failed: x is 4 end",
            "✖ Assertion failed: plain %s",
            "✖ Assertion failed: 'q'",
            "✖ Assertion failed: a",
            "✖ Assertion failed 7 %s b",
            "✖ Assertion failed { a: 1 }",
            "✖ Assertion failed: v %s",
        ]
    );
}

#[test]
fn assert_does_not_format_when_the_condition_holds() {
    assert!(
        texts("console.assert(true, '%s', { toString() { console.log('ran'); } });").is_empty()
    );
}

#[test]
fn a_group_label_substitutes_specifiers() {
    assert_eq!(
        texts(
            "console.group('%s (%d)', 'job', 3, 'extra'); console.groupCollapsed('%c%s', 'css', 'c'); console.group('%s');"
        ),
        ["▼ job (3) extra", "│ ▶ c", "│ │ ▼ %s"]
    );
}

#[test]
fn trace_substitutes_specifiers_in_its_label() {
    let text = &texts("console.trace('%s:%d', 'f', 2, 'x');")[0];

    assert_eq!(text.lines().next(), Some("Trace: f:2 x"));
}

#[test]
fn time_log_prints_its_extra_data_without_substituting_specifiers() {
    let lines = texts(
        "console.time('%s'); console.timeLog('%s', '%d items', '3'); \
         console.timeLog('%s', 'a', '%s');",
    );

    assert_eq!(without_elapsed(&lines[0]), "%s: Nms %d items 3");
    assert_eq!(without_elapsed(&lines[1]), "%s: Nms a %s");
}

#[test]
fn dir_table_and_the_counters_do_not_format() {
    assert_eq!(
        texts(
            "console.dir('%s', 'x'); console.table('%s'); console.count('%s'); \
             console.countReset('%d'); console.timeEnd('%i');"
        ),
        [
            "'%s'",
            "%s",
            "%s: 1",
            "⚠ Count for '%d' does not exist",
            "⚠ Timer '%i' does not exist",
        ]
    );
}

#[test]
fn time_labels_are_not_formatted() {
    let lines = texts("console.time('%d'); console.timeEnd('%d');");

    assert_eq!(without_elapsed(&lines[0]), "%d: Nms");
}

#[test]
fn a_throwing_conversion_leaves_no_exception_behind() {
    assert_eq!(
        texts(
            "console.log('%s', { toString() { throw new Error('no'); } }); \
             try { null.x; } catch (e) { console.log('caught', e instanceof TypeError); }"
        ),
        ["{ toString: [Function: toString] }", "caught true"]
    );
}

#[test]
fn accessors_and_a_failing_stack_in_any_method_leave_nothing_pending() {
    assert_eq!(
        texts(
            "console.log({ get a() { throw 1; } }); \
             console.log([Object.defineProperty([1], 0, { get() { throw 1; } })]); \
             console.log(Object.defineProperty(new Error('e'), 'stack', { get() { throw 1; } })); \
             console.dir({ get a() { throw 1; } }); \
             try { null.x; } catch (e) { console.log('caught'); }"
        )
        .iter()
        .map(|t| t.lines().next().unwrap().to_owned())
        .collect::<Vec<_>>(),
        [
            "{ a: [Getter] }",
            "[ [ [Getter] ] ]",
            "Error: e",
            "{ a: [Getter] }",
            "caught"
        ]
    );
}

#[test]
fn replacing_the_global_parsers_does_not_change_formatting() {
    assert_eq!(
        texts(
            "globalThis.parseInt = () => 99; globalThis.parseFloat = () => 98; console.log('%d %i %f %s', '1', '2', '3.5', 4);"
        ),
        ["1 2 3.5 4"]
    );
}
