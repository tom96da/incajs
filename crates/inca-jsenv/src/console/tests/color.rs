// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use rstest::rstest;

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
            r"
            console.error('e'); console.warn('w'); console.info('i');
            console.log('l'); console.dirxml('x');
            "
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
    let source = r"
        console.group('g'); console.log('s', 1, null, undefined, { a: ['b'] });
        console.debug('d'); console.info('i'); console.warn('w'); console.error(new Error('e'));
        console.assert(false, 'x'); console.trace('t'); console.dir('q');
        console.table([{ a: 1 }, 'v']); console.count(); console.countReset('z');
        console.groupEnd(); console.log(function f() {}, Symbol('s'), 10n);
        ";

    assert!(texts(source).iter().all(|t| !t.contains('\x1b')));
}

#[test]
fn color_on_adds_only_sequences_to_the_plain_output() {
    let source = r"
        console.group('g'); console.log('s', 1, null, undefined, { a: ['b'] });
        console.debug('d'); console.info('i'); console.warn('w');
        console.error('e\nf'); console.assert(false, 'x'); console.dir('q');
        console.table([{ a: 1 }, 'v']); console.count(); console.countReset('z');
        console.groupEnd(); console.log(function f() {}, Symbol('s'), 10n);
        ";
    let plain = texts(source);
    let colored: Vec<String> = colored_texts(source).iter().map(|t| strip(t)).collect();

    assert_eq!(colored, plain);
}

#[test]
fn color_never_crosses_a_line_break_inside_nested_groups() {
    let lines = colored_texts(
        "console.group('a'); console.group('b'); console.warn(['x\\ny'], Symbol('p\\nq'));",
    );

    assert_eq!(
        lines[2],
        format!(
            concat!(
                "{0}{0}\x1b[33m⚠\x1b[39m [ \x1b[32m'x\x1b[39m\n",
                "{0}{0}  \x1b[32my'\x1b[39m ] \x1b[32mSymbol(p\x1b[39m\n",
                "{0}{0}  \x1b[32mq)\x1b[39m",
            ),
            GUIDE
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
fn every_color_in_every_method_is_closed_on_its_own_line() {
    let source = r"
        console.group('g\nh'); console.log(['a\nb'], Symbol('s\nt'));
        console.info({ 'k\nz': 'v\nw' }); console.error(new Error('e\nf'));
        console.trace('t\nu'); console.debug('d\ne'); console.assert(false, ['q\nr']);
        console.dir(['m\nn']); console.table([['c\nd', 1]]);
        ";

    for text in colored_texts(source) {
        for line in text.lines() {
            assert!(balanced(line), "{line:?}");
        }
    }
}

fn styled(open: u8, text: &str) -> String {
    format!("\x1b[{open}m{text}\x1b[39m")
}

#[rstest]
#[case::red("red", 31)]
#[case::green("green", 32)]
#[case::yellow("yellow", 33)]
#[case::blue("blue", 34)]
#[case::magenta("magenta", 35)]
#[case::cyan("cyan", 36)]
#[case::black("black", 30)]
#[case::white("white", 37)]
#[case::gray("gray", 90)]
#[case::grey("grey", 90)]
fn a_named_color_applies_to_the_text_that_follows(#[case] name: &str, #[case] open: u8) {
    assert_eq!(
        colored_texts(&format!("console.log('%chi', 'color: {name}');")),
        [styled(open, "hi")]
    );
}

#[test]
fn text_before_the_style_stays_plain() {
    assert_eq!(
        colored_texts("console.log('a%cb', 'color: red');"),
        [format!("a{}", styled(31, "b"))]
    );
}

#[rstest]
#[case::two_colors("color: red; color: blue", 34)]
#[case::other_property_between("color: red; margin: 0; color: green", 32)]
#[case::unusable_last_value("color: red; color: #fff", 31)]
#[case::invalid_flag_last("color: red; color: blue !importantx", 31)]
fn the_last_usable_color_declaration_wins(#[case] css: &str, #[case] open: u8) {
    assert_eq!(
        colored_texts(&format!("console.log('%ca', {css:?});")),
        [styled(open, "a")]
    );
}

#[rstest]
#[case::flag("color: red !important")]
#[case::flag_without_space("color: red!important")]
#[case::upper_case_flag("color: red !IMPORTANT")]
#[case::spaced_flag("color: red ! important ;")]
fn an_important_flag_after_the_value_is_accepted(#[case] css: &str) {
    assert_eq!(
        colored_texts(&format!("console.log('%ca', {css:?});")),
        [styled(31, "a")]
    );
}

#[rstest]
#[case::padding("  color :  red  ;")]
#[case::upper_case("COLOR: RED")]
fn a_color_declaration_ignores_case_and_whitespace(#[case] css: &str) {
    assert_eq!(
        colored_texts(&format!("console.log('%ca', {css:?});")),
        [styled(31, "a")]
    );
}

#[test]
fn a_style_covers_the_text_of_the_specifiers_after_it() {
    assert_eq!(
        colored_texts("console.log('%c%s!', 'color: red', 'x');"),
        [styled(31, "x!")]
    );
}

#[test]
fn two_styles_make_two_spans() {
    assert_eq!(
        colored_texts("console.log('%ca%cb', 'color: red', 'color: blue');"),
        [format!("{}{}", styled(31, "a"), styled(34, "b"))]
    );
}

#[rstest]
#[case::lower_o("%o")]
#[case::upper_o("%O")]
fn an_inspected_argument_inside_a_span_keeps_its_own_color(#[case] spec: &str) {
    let lines = colored_texts(&format!("console.log('%cx{spec}y', 'color: red', 'q');"));

    assert_eq!(
        lines,
        [format!(
            "{}\x1b[32m'q'\x1b[39m{}",
            styled(31, "x"),
            styled(31, "y")
        )]
    );
    assert!(balanced(&lines[0]));
}

#[test]
fn a_style_is_closed_and_reopened_around_a_line_break() {
    assert_eq!(
        colored_texts("console.log('%ca\\nb', 'color: red');"),
        [format!("{}\n{}", styled(31, "a"), styled(31, "b"))]
    );
}

#[test]
fn a_style_without_an_argument_stays_as_written() {
    assert_eq!(
        colored_texts("console.log('%cx %c', 'color: red');"),
        [styled(31, "x %c")]
    );
}

#[test]
fn arguments_after_the_format_stay_outside_the_span() {
    assert_eq!(
        colored_texts("console.log('%cx', 'color: red', 1);"),
        [format!("{} \x1b[33m1\x1b[39m", styled(31, "x"))]
    );
}

#[test]
fn a_debug_line_ignores_the_style() {
    assert_eq!(
        colored_texts("console.debug('%cx', 'color: red');"),
        ["\x1b[90mx\x1b[39m"]
    );
}

#[rstest]
#[case::unknown_name("color: nope")]
#[case::unknown_property("background: red")]
#[case::no_colon("red")]
#[case::empty_value("color:")]
#[case::important_without_bang("color: red important")]
#[case::flag_alone("color: !important")]
#[case::double_bang("color: red !!important")]
fn an_unusable_style_resets_the_color(#[case] css: &str) {
    assert_eq!(
        colored_texts(&format!("console.log('%ca%cb', 'color: red', {css:?});")),
        [format!("{}b", styled(31, "a"))]
    );
}

#[test]
fn a_non_string_style_resets_the_color() {
    assert_eq!(
        colored_texts("console.log('%ca%cb', 'color: red', 5);"),
        [format!("{}b", styled(31, "a"))]
    );
}

#[test]
fn a_style_stays_out_of_the_group_guides() {
    assert_eq!(
        colored_texts(
            r"
            console.group('g');
            console.log('%ca\nb', 'color: red');
            "
        ),
        [
            "▼ g".to_owned(),
            format!("{GUIDE}{}\n{GUIDE}{}", styled(31, "a"), styled(31, "b")),
        ]
    );
}

#[rstest]
#[case::error("error", "\x1b[31m✖\x1b[39m ")]
#[case::warn("warn", "\x1b[33m⚠\x1b[39m ")]
#[case::info("info", "\x1b[34mℹ\x1b[39m ")]
fn a_style_stays_out_of_the_level_symbol(#[case] method: &str, #[case] lead: &str) {
    assert_eq!(
        colored_texts(&format!("console.{method}('%ca', 'color: red');")),
        [format!("{lead}{}", styled(31, "a"))]
    );
}

#[test]
fn a_style_stays_out_of_the_assertion_lead() {
    assert_eq!(
        colored_texts("console.assert(false, '%ca', 'color: red');"),
        [format!(
            "\x1b[31m✖\x1b[39m Assertion failed: {}",
            styled(31, "a")
        )]
    );
}

#[test]
fn a_style_stays_out_of_the_trace_lead() {
    let text = &colored_texts("console.trace('%ca', 'color: red');")[0];

    assert_eq!(
        text.lines().next().unwrap(),
        format!("Trace: {}", styled(31, "a"))
    );
}

#[test]
fn a_multi_byte_text_around_a_span_stays_whole() {
    assert_eq!(
        colored_texts("console.log('日本%c語%cx', 'color: red', '');"),
        [format!("日本{}x", styled(31, "語"))]
    );
}

#[test]
fn a_double_percent_inside_a_span_is_painted() {
    assert_eq!(
        colored_texts("console.log('%cx%%y', 'color: red');"),
        [styled(31, "x%y")]
    );
}

#[test]
fn a_style_at_the_end_opens_nothing() {
    assert_eq!(colored_texts("console.log('a%c', 'color: red');"), ["a"]);
}

#[test]
fn a_lone_style_prints_an_empty_line() {
    assert_eq!(colored_texts("console.log('%c', 'color: red');"), [""]);
}

#[test]
fn a_colored_conversion_keeps_its_color_inside_a_span() {
    assert_eq!(
        colored_texts("console.log('%c%s tail', 'color: red', Symbol('a'));"),
        [format!("\x1b[32mSymbol(a)\x1b[39m{}", styled(31, " tail"))]
    );
}
