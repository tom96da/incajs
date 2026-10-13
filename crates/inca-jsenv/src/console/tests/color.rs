// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

const GUIDE: &str = "\x1b[90m│ \x1b[39m";

#[test]
fn values_are_colored_by_type_in_a_log_line() {
    assert_eq!(
        colored_texts("console.log('s', 1, true, null, undefined, { k: 'v' });"),
        [
            "s \x1b[33m1\x1b[39m \x1b[33mtrue\x1b[39m \x1b[1mnull\x1b[22m \x1b[90mundefined\x1b[39m { k: \x1b[32m'v'\x1b[39m }"
        ]
    );
}

#[test]
fn each_level_symbol_is_colored_and_its_body_is_not() {
    assert_eq!(
        colored_texts(
            "console.error('e'); console.warn('w'); console.info('i'); \
             console.log('l'); console.dirxml('x');"
        ),
        [
            "\x1b[31m✖\x1b[39m e",
            "\x1b[33m⚠\x1b[39m w",
            "\x1b[34mℹ\x1b[39m i",
            "l",
            "x"
        ]
    );
}

#[test]
fn a_failed_assert_has_a_red_symbol() {
    assert_eq!(
        colored_texts("console.assert(false, 'bad');"),
        ["\x1b[31m✖\x1b[39m Assertion failed: bad"]
    );
}

#[test]
fn a_warning_from_a_counter_or_timer_has_a_yellow_symbol() {
    assert_eq!(
        colored_texts("console.countReset('q'); console.timeEnd('q');"),
        [
            "\x1b[33m⚠\x1b[39m Count for 'q' does not exist",
            "\x1b[33m⚠\x1b[39m Timer 'q' does not exist"
        ]
    );
}

#[test]
fn a_debug_line_is_dim_as_a_whole_without_value_colors() {
    assert_eq!(
        colored_texts("console.debug('d', 1, { a: 'b' });"),
        ["\x1b[90md 1 { a: 'b' }\x1b[39m"]
    );
}

#[test]
fn every_line_of_a_multi_line_debug_body_is_dim() {
    assert_eq!(
        colored_texts("console.debug('a\\nb');"),
        ["\x1b[90ma\x1b[39m\n\x1b[90mb\x1b[39m"]
    );
}

#[test]
fn group_guides_are_dim_at_every_depth() {
    assert_eq!(
        colored_texts(
            "console.group('a'); console.group('b'); console.log('x'); console.warn('w');"
        ),
        [
            "▼ a".to_owned(),
            format!("{GUIDE}▼ b"),
            format!("{GUIDE}{GUIDE}x"),
            format!("{GUIDE}{GUIDE}\x1b[33m⚠\x1b[39m w"),
        ]
    );
}

#[test]
fn a_multi_line_body_lines_up_with_color_on() {
    assert_eq!(
        colored_texts("console.group('g'); console.error('a\\nb');"),
        [
            "▼ g".to_owned(),
            format!("{GUIDE}\x1b[31m✖\x1b[39m a\n{GUIDE}  b"),
        ]
    );
}

#[test]
fn an_empty_line_keeps_no_trailing_space_with_color_on() {
    assert_eq!(
        colored_texts("console.group(''); console.log(''); console.error('');"),
        [
            "▼".to_owned(),
            "\x1b[90m│\x1b[39m".to_owned(),
            format!("{GUIDE}\x1b[31m✖\x1b[39m"),
        ]
    );
}

#[test]
fn the_first_line_of_an_error_is_plain_and_its_frames_are_dim() {
    let text = &colored_texts("console.log(new Error('boom'));")[0];
    let mut lines = text.lines();

    assert_eq!(lines.next(), Some("Error: boom"));
    for frame in lines {
        assert!(
            frame.starts_with("\x1b[90m") && frame.ends_with("\x1b[39m"),
            "{frame:?}"
        );
    }
}

#[test]
fn trace_keeps_its_first_line_plain_and_dims_each_frame() {
    let text = &colored_texts(
        "function inner() { console.trace('hi', 1); }\nfunction outer() { inner(); }\nouter();",
    )[0];
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(lines[0], "Trace: hi \x1b[33m1\x1b[39m");
    assert!(lines.len() >= 3, "{lines:?}");
    for frame in &lines[1..] {
        assert!(
            frame.starts_with("\x1b[90m") && frame.ends_with("\x1b[39m"),
            "{frame:?}"
        );
    }
}

#[test]
fn trace_inside_a_group_dims_guides_and_frames() {
    let text = &colored_texts("console.group('g'); console.trace();")[1];

    for (index, line) in text.lines().enumerate() {
        assert!(line.starts_with(GUIDE), "{line:?}");
        let rest = &line[GUIDE.len()..];
        assert_eq!(rest.starts_with("\x1b[90m"), index > 0, "{line:?}");
    }
}

#[test]
fn dir_and_the_table_fallback_color_their_value() {
    assert_eq!(
        colored_texts("console.dir('s'); console.dir(); console.table(5); console.table();"),
        [
            "\x1b[32m's'\x1b[39m",
            "\x1b[90mundefined\x1b[39m",
            "\x1b[33m5\x1b[39m",
            "\x1b[90mundefined\x1b[39m"
        ]
    );
}

#[test]
fn time_log_colors_its_extra_data() {
    let lines = colored_texts("console.time('t'); console.timeLog('t', 2);");

    assert!(lines[0].ends_with(" \x1b[33m2\x1b[39m"), "{}", lines[0]);
    assert!(lines[0].starts_with("t: "), "{}", lines[0]);
}

#[test]
fn a_group_label_and_assert_data_are_colored() {
    assert_eq!(
        colored_texts("console.group('a', 1); console.assert(false, 'm', 2);"),
        [
            "▼ a \x1b[33m1\x1b[39m".to_owned(),
            format!("{GUIDE}\x1b[31m✖\x1b[39m Assertion failed: m \x1b[33m2\x1b[39m"),
        ]
    );
}

#[test]
fn format_specifiers_color_inspected_arguments() {
    assert_eq!(
        colored_texts("console.log('%o|%s', 'q', 'r', 3);"),
        ["\x1b[32m'q'\x1b[39m|r \x1b[33m3\x1b[39m"]
    );
}

#[test]
fn color_off_writes_no_escape_sequences_for_any_method() {
    let source = "console.group('g'); console.log('s', 1, null, undefined, { a: ['b'] }); \
         console.debug('d'); console.info('i'); console.warn('w'); console.error(new Error('e')); \
         console.assert(false, 'x'); console.trace('t'); console.dir('q'); \
         console.table([{ a: 1 }, 'v']); console.count(); console.countReset('z'); \
         console.groupEnd(); console.log(function f() {}, Symbol('s'), 10n);";

    assert!(texts(source).iter().all(|t| !t.contains('\x1b')));
}

#[test]
fn color_on_adds_only_sequences_to_the_plain_output() {
    let source = "console.group('g'); console.log('s', 1, null, undefined, { a: ['b'] }); \
         console.debug('d'); console.info('i'); console.warn('w'); \
         console.error('e\\nf'); console.assert(false, 'x'); console.dir('q'); \
         console.table([{ a: 1 }, 'v']); console.count(); console.countReset('z'); \
         console.groupEnd(); console.log(function f() {}, Symbol('s'), 10n);";
    let plain = texts(source);
    let colored: Vec<String> = colored_texts(source).iter().map(|t| strip(t)).collect();

    assert_eq!(colored, plain);
}

#[test]
fn color_does_not_change_the_levels() {
    let source = "console.trace('a'); console.debug('a'); console.log('a'); \
         console.info('a'); console.warn('a'); console.error('a');";
    let levels = |color| -> Vec<Level> {
        written_with_color(source, color)
            .iter()
            .map(|(level, _)| *level)
            .collect()
    };

    assert_eq!(levels(true), levels(false));
}

#[test]
fn color_never_crosses_a_line_break_in_a_nested_string() {
    assert_eq!(
        colored_texts("console.log(['a\\nb']);"),
        ["[ \x1b[32m'a\x1b[39m\n\x1b[32mb'\x1b[39m ]"]
    );
}

#[test]
fn color_never_crosses_a_line_break_inside_nested_groups() {
    let lines = colored_texts(
        "console.group('a'); console.group('b'); console.warn(['x\\ny'], Symbol('p\\nq'));",
    );

    assert_eq!(
        lines[2],
        format!(
            "{GUIDE}{GUIDE}\x1b[33m⚠\x1b[39m [ \x1b[32m'x\x1b[39m\n\
             {GUIDE}{GUIDE}  \x1b[32my'\x1b[39m ] \x1b[32mSymbol(p\x1b[39m\n\
             {GUIDE}{GUIDE}  \x1b[32mq)\x1b[39m"
        )
    );
    for line in lines[2].lines() {
        assert!(balanced(line), "{line:?}");
    }
}

#[test]
fn a_multi_line_error_message_is_balanced_on_every_line() {
    let text = &colored_texts("console.error(new Error('x\\ny'));")[0];
    let lines: Vec<&str> = text.lines().collect();

    assert_eq!(lines[0], "\x1b[31m✖\x1b[39m Error: x");
    assert_eq!(lines[1], "  y");
    assert!(lines.len() > 2);
    assert!(lines.iter().all(|line| balanced(line)), "{lines:?}");
}

#[test]
fn a_debug_line_with_a_multi_line_value_is_dim_on_each_line() {
    assert_eq!(
        colored_texts("console.group('g'); console.debug({ a: 'x\\ny' });")[1],
        format!("{GUIDE}\x1b[90m{{ a: 'x\x1b[39m\n{GUIDE}\x1b[90my' }}\x1b[39m")
    );
}

#[test]
fn every_color_in_every_method_is_closed_on_its_own_line() {
    let source = "console.group('g\\nh'); console.log(['a\\nb'], Symbol('s\\nt')); \
         console.info({ 'k\\nz': 'v\\nw' }); console.error(new Error('e\\nf')); \
         console.trace('t\\nu'); console.debug('d\\ne'); console.assert(false, ['q\\nr']); \
         console.dir(['m\\nn']); console.table([['c\\nd', 1]]);";

    for text in colored_texts(source) {
        for line in text.lines() {
            assert!(balanced(line), "{line:?}");
        }
    }
}
