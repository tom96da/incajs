// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn accessor_text_is_cyan() {
    assert_eq!(
        colored("({ get a() { return 1; }, set b(v) {}, get c() { return 1; }, set c(v) {} })"),
        format!(
            "{{ a: {}, b: {}, c: {} }}",
            cyan("[Getter]"),
            cyan("[Setter]"),
            cyan("[Getter/Setter]")
        )
    );
}

#[test]
fn quoted_quotes_a_string_and_leaves_the_rest_alone() {
    with_realm(|ctx| {
        let text: Value<'_> = ctx.eval("'it\\'s'").unwrap();
        let object: Value<'_> = ctx.eval("({ a: 'b' })").unwrap();
        let number: Value<'_> = ctx.eval("7").unwrap();

        assert_eq!(quoted(&text, false), "'it\\'s'");
        assert_eq!(quoted(&object, false), "{ a: 'b' }");
        assert_eq!(quoted(&number, false), "7");
    });
}

#[test]
fn several_arguments_are_space_separated() {
    let out = with_realm(|ctx| {
        let values: Vec<Value<'_>> = ctx.eval("['a', 1, { k: true }]").unwrap();
        line(&values, false)
    });

    assert_eq!(out, "a 1 { k: true }");
}

#[test]
fn numbers_bigints_and_booleans_are_yellow() {
    assert_eq!(colored("42"), yellow("42"));
    assert_eq!(colored("1.5"), yellow("1.5"));
    assert_eq!(colored("NaN"), yellow("NaN"));
    assert_eq!(colored("1 / 0"), yellow("Infinity"));
    assert_eq!(colored("-1 / 0"), yellow("-Infinity"));
    assert_eq!(colored("10n"), yellow("10n"));
    assert_eq!(colored("true"), yellow("true"));
    assert_eq!(colored("false"), yellow("false"));
}

#[test]
fn null_is_bold_and_undefined_is_grey() {
    assert_eq!(colored("null"), "\x1b[1mnull\x1b[22m");
    assert_eq!(colored("undefined"), grey("undefined"));
}

#[test]
fn a_symbol_is_green() {
    assert_eq!(colored("Symbol('tag')"), green("Symbol(tag)"));
}

#[test]
fn a_top_level_string_stays_uncolored() {
    assert_eq!(colored("'plain'"), "plain");
    assert_eq!(colored("''"), "");
}

#[test]
fn a_nested_string_is_green_with_its_quotes() {
    assert_eq!(
        colored("['a', 1]"),
        format!("[ {}, {} ]", green("'a'"), yellow("1"))
    );
    assert_eq!(colored("['it\\'s']"), format!("[ {} ]", green("'it\\'s'")));
}

#[test]
fn an_identifier_key_is_uncolored_and_a_quoted_key_is_green() {
    assert_eq!(
        colored("({ k: 'v', n: 1 })"),
        format!("{{ k: {}, n: {} }}", green("'v'"), yellow("1"))
    );
    let source = "({ '1': 2, 'a b': 1, _ok9: 3 })";

    assert_eq!(
        colored(source),
        format!(
            "{{ {}: {}, {}: {}, _ok9: {} }}",
            green("'1'"),
            yellow("2"),
            green("'a b'"),
            yellow("1"),
            yellow("3")
        )
    );
    assert_eq!(rendered(source), "{ '1': 2, 'a b': 1, _ok9: 3 }");
}

#[test]
fn functions_are_cyan() {
    assert_eq!(
        colored("function named() {}; named"),
        cyan("[Function: named]")
    );
    assert_eq!(colored("(() => {})"), cyan("[Function (anonymous)]"));
}

#[test]
fn depth_placeholders_are_cyan() {
    assert_eq!(
        colored("({ a: { b: { c: { d: 1 } } } })"),
        format!("{{ a: {{ b: {{ c: {} }} }} }}", cyan("[Object]"))
    );
    assert_eq!(
        colored("[[[[1]]]]"),
        format!("[ [ [ {} ] ] ]", cyan("[Array]"))
    );
}

#[test]
fn empty_containers_are_uncolored() {
    assert_eq!(colored("[]"), "[]");
    assert_eq!(colored("({})"), "{}");
}

#[test]
fn an_error_keeps_its_first_line_and_dims_every_frame() {
    let out = colored("new Error('boom')");
    let mut lines = out.lines();

    assert_eq!(lines.next(), Some("Error: boom"));
    let frames: Vec<&str> = lines.collect();
    assert!(!frames.is_empty());
    for frame in frames {
        assert!(frame.starts_with("\x1b[90m"), "{frame:?}");
        assert!(frame.ends_with("\x1b[39m"), "{frame:?}");
        assert!(frame.contains("at "), "{frame:?}");
    }
}

#[test]
fn an_error_without_a_stack_has_nothing_to_dim() {
    assert_eq!(
        colored("const e = new TypeError('bad'); e.stack = ''; e"),
        "TypeError: bad"
    );
}

#[test]
fn arguments_are_colored_one_by_one_and_joined_plainly() {
    let out = with_realm(|ctx| {
        let values: Vec<Value<'_>> = ctx.eval("['a', 1, null]").unwrap();
        line(&values, true)
    });

    assert_eq!(out, format!("a {} \x1b[1mnull\x1b[22m", yellow("1")));
}

#[test]
fn quoted_colors_a_string_green_and_a_number_yellow() {
    with_realm(|ctx| {
        let text: Value<'_> = ctx.eval("'s'").unwrap();
        let number: Value<'_> = ctx.eval("7").unwrap();

        assert_eq!(quoted(&text, true), green("'s'"));
        assert_eq!(quoted(&number, true), yellow("7"));
    });
}

#[test]
fn color_never_crosses_a_line_break() {
    assert_eq!(
        colored("['a\\nb']"),
        "[ \x1b[32m'a\x1b[39m\n\x1b[32mb'\x1b[39m ]"
    );
    assert_eq!(
        colored("Symbol('a\\nb')"),
        "\x1b[32mSymbol(a\x1b[39m\n\x1b[32mb)\x1b[39m"
    );
    assert_eq!(
        colored("({ 'k': 'a\\n\\nb' })"),
        "{ k: \x1b[32m'a\x1b[39m\n\n\x1b[32mb'\x1b[39m }"
    );
}

#[test]
fn a_multi_line_error_message_stays_plain() {
    let out = colored("new Error('x\\ny')");
    let lines: Vec<&str> = out.lines().collect();

    assert_eq!(lines[..2], ["Error: x", "y"]);
    assert!(lines[2].starts_with("\x1b[90m"), "{lines:?}");
}

#[test]
fn escaped_text_has_no_control_character_left() {
    assert_eq!(
        escaped("['a\\nb\\tc\\x01\\x1b[1m']", false),
        "[ 'a\\nb\\tc\\x01\\x1b[1m' ]"
    );
    assert_eq!(escaped("Symbol('a\\nb')", false), "Symbol(a\\nb)");
    assert_eq!(escaped("({ 'k\\n': 1 })", false), "{ 'k\\n': 1 }");
    assert_eq!(escaped("'top\\n'", false), "'top\\n'");
}

#[test]
fn an_escaped_error_is_one_line() {
    let out = escaped("new Error('x\\ny')", false);

    assert!(!out.contains('\n'), "{out}");
    assert!(out.starts_with("Error: x\\ny\\n    at "), "{out}");
}

#[test]
fn escaping_happens_before_painting() {
    assert_eq!(
        escaped("['\\x1b[31m']", true),
        "[ \x1b[32m'\\x1b[31m'\x1b[39m ]"
    );
    assert_eq!(escaped("['a\\nb']", true), "[ \x1b[32m'a\\nb'\x1b[39m ]");
}
