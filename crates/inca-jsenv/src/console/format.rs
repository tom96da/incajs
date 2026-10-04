// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Format specifiers for the console methods that take a format string.

use rquickjs::{Coerced, Ctx, Type, Value};

use crate::inspect;

/// Formats a call's arguments into the line it prints, applying the format
/// specifiers of the first argument when it is a string.
///
/// `%s`, `%d`/`%i`, `%f`, `%o`/`%O`, `%c` and `%%` are replaced; anything else
/// stays as written. A conversion that throws prints the argument as
/// `console.dir` shows it.
pub(super) fn line(ctx: &Ctx<'_>, values: &[Value<'_>], color: bool) -> String {
    let Some((first, rest)) = values.split_first() else {
        return String::new();
    };
    let Some(mut target) = first.as_string().and_then(|text| text.to_string().ok()) else {
        return inspect::line(values, color);
    };
    if rest.is_empty() {
        return inspect::line(values, color);
    }

    let mut args = rest.iter();
    let mut at = 0;
    while let Some((pos, spec)) = next_specifier(&target, at) {
        if spec == b'%' {
            target.replace_range(pos..pos + 2, "%");
            at = pos + 1;
            continue;
        }
        let Some(arg) = args.next() else {
            at = pos + 2;
            continue;
        };
        target.replace_range(pos..pos + 2, &convert(ctx, spec, arg, color));
        at = pos;
    }

    let rest = args.as_slice();
    if rest.is_empty() {
        target
    } else {
        format!("{target} {}", inspect::line(rest, color))
    }
}

fn next_specifier(target: &str, from: usize) -> Option<(usize, u8)> {
    let bytes = target.as_bytes();
    let mut pos = from;
    while let Some(found) = target[pos..].find('%') {
        pos += found;
        match bytes.get(pos + 1) {
            Some(&spec) if b"sdifoOc%".contains(&spec) => return Some((pos, spec)),
            _ => pos += 1,
        }
    }
    None
}

fn convert(ctx: &Ctx<'_>, spec: u8, arg: &Value<'_>, color: bool) -> String {
    // A proxy converts as its target. A revoked one has none and converts as
    // a symbol does, which has no string form either.
    let target = inspect::unproxied(arg);
    let symbol = arg.type_of() == Type::Symbol || target.is_none();
    let arg = target.as_ref().unwrap_or(arg);
    let negative_zero = arg
        .as_number()
        .is_some_and(|number| number.to_bits() == (-0.0_f64).to_bits());
    let converted = match spec {
        b'c' => Some(String::new()),
        b's' | b'd' if negative_zero => Some("-0".to_owned()),
        b'o' | b'O' => Some(inspect::quoted(arg, color)),
        b's' if symbol => Some(inspect::quoted(arg, color)),
        b's' => string_of(ctx, arg),
        _ if symbol => Some("NaN".to_owned()),
        b'f' => string_of(ctx, arg).map(|text| number_text(ctx, parse_float(&text))),
        _ => string_of(ctx, arg).map(|text| number_text(ctx, parse_int(&text))),
    };
    converted.unwrap_or_else(|| inspect::quoted(arg, color))
}

fn string_of(ctx: &Ctx<'_>, arg: &Value<'_>) -> Option<String> {
    inspect::settled(ctx, arg.get::<Coerced<String>>()).map(|text| text.0)
}

fn number_text(ctx: &Ctx<'_>, number: f64) -> String {
    Value::new_number(ctx.clone(), number)
        .get::<Coerced<String>>()
        .map_or_else(|_| number.to_string(), |text| text.0)
}

fn is_js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

fn sign(text: &str) -> (bool, &str) {
    match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    }
}

/// Mirrors the engine's parseInt(text, 10), which a script can replace.
fn parse_int(text: &str) -> f64 {
    let (negative, text) = sign(text.trim_start_matches(is_js_space));
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return f64::NAN;
    }
    let value: f64 = text[..digits].parse().unwrap_or(f64::NAN);
    if negative { -value } else { value }
}

fn parse_float(text: &str) -> f64 {
    let (negative, text) = sign(text.trim_start_matches(is_js_space));
    let value = if text.starts_with("Infinity") {
        f64::INFINITY
    } else {
        let bytes = text.as_bytes();
        let run = |from: usize| {
            bytes[from..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count()
        };
        let mut end = run(0);
        let mut digits = end;
        if bytes.get(end) == Some(&b'.') {
            let fraction = run(end + 1);
            digits += fraction;
            end += 1 + fraction;
        }
        if digits == 0 {
            return f64::NAN;
        }
        if matches!(bytes.get(end), Some(b'e' | b'E')) {
            let exponent = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
            let count = run(end + 1 + exponent);
            if count > 0 {
                end += 1 + exponent + count;
            }
        }
        text[..end].parse().unwrap_or(f64::NAN)
    };
    if negative { -value } else { value }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rquickjs::{Context, Runtime};

    use super::*;

    /// Formats the arguments `args` (a JS array literal) with [`line`].
    fn formatted(args: &str) -> String {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let values: Vec<Value<'_>> = ctx.eval(args).unwrap();
            let out = line(&ctx, &values, false);
            assert!(!ctx.has_exception(), "a conversion left an exception");
            out
        })
    }

    #[test]
    fn nothing_formats_to_an_empty_line() {
        assert_eq!(formatted("[]"), "");
    }

    #[test]
    fn a_single_argument_is_printed_as_it_is() {
        assert_eq!(formatted("['%s %d %%']"), "%s %d %%");
        assert_eq!(formatted("[{ a: '%s' }]"), "{ a: '%s' }");
    }

    #[test]
    fn a_first_argument_that_is_not_a_string_is_not_a_format() {
        assert_eq!(formatted("[5, '%s', 1]"), "5 %s 1");
        assert_eq!(formatted("[{ a: 1 }, '%d']"), "{ a: 1 } %d");
    }

    #[test]
    fn a_first_string_without_a_specifier_is_followed_by_the_rest() {
        assert_eq!(
            formatted("['plain', 1, 'two', { a: 3 }]"),
            "plain 1 two { a: 3 }"
        );
    }

    #[test]
    fn s_uses_the_string_form() {
        assert_eq!(formatted("['%s', 'text', 'x']"), "text x");
        assert_eq!(formatted("['%s', 42]"), "42");
        assert_eq!(formatted("['%s', 1.5]"), "1.5");
        assert_eq!(formatted("['%s', -0]"), "-0");
        assert_eq!(formatted("['%s', NaN]"), "NaN");
        assert_eq!(formatted("['%s', 1 / 0]"), "Infinity");
        assert_eq!(formatted("['%s', true]"), "true");
        assert_eq!(formatted("['%s', null]"), "null");
        assert_eq!(formatted("['%s', undefined]"), "undefined");
        assert_eq!(formatted("['%s', 10n]"), "10");
        assert_eq!(formatted("['%s', Symbol('tag')]"), "Symbol(tag)");
        assert_eq!(formatted("['%s', Symbol()]"), "Symbol()");
        assert_eq!(formatted("['%s', { a: 1 }]"), "[object Object]");
        assert_eq!(formatted("['%s', [1, [2, 3]]]"), "1,2,3");
        assert_eq!(
            formatted("['%s', { toString() { return 'custom'; } }]"),
            "custom"
        );
        assert_eq!(formatted("['%s', function f() {}]"), "function f() {}");
    }

    #[test]
    fn s_falls_back_to_the_inspected_value_when_the_string_form_throws() {
        assert_eq!(
            formatted("['%s', { toString() { throw new Error('no'); } }]"),
            "{ toString: [Function: toString] }"
        );
        assert_eq!(
            formatted("['%s', { toString() { throw 1; } }, 'after']"),
            "{ toString: [Function: toString] } after"
        );
        assert_eq!(
            formatted("['%s', Object.create(null), 'after']"),
            "{} after"
        );
    }

    #[test]
    fn d_and_i_parse_an_integer() {
        for spec in ["%d", "%i"] {
            let f = |arg: &str| formatted(&format!("['{spec}', {arg}]"));
            assert_eq!(f("'  7px'"), "7");
            assert_eq!(f("-42.9"), "-42");
            assert_eq!(f("'0x10'"), "0");
            assert_eq!(f("'abc'"), "NaN");
            assert_eq!(f("-0"), if spec == "%d" { "-0" } else { "0" });
            assert_eq!(f("123n"), "123");
            assert_eq!(f("Symbol('s')"), "NaN");
            assert_eq!(f("'9'.repeat(30)"), "1e+30");
            assert_eq!(f("NaN"), "NaN");
            assert_eq!(f("1 / 0"), "NaN");
            assert_eq!(f("true"), "NaN");
            assert_eq!(f("null"), "NaN");
            assert_eq!(f("undefined"), "NaN");
            assert_eq!(f("-5n"), "-5");
            assert_eq!(f("{}"), "NaN");
            assert_eq!(f("[12, 3]"), "12");
            assert_eq!(f("{ toString() { return '9 lives'; } }"), "9");
        }
    }

    #[test]
    fn accessors_in_an_inspected_value_are_named_and_not_run() {
        assert_eq!(
            formatted("['%o', { get a() { throw 1; } }]"),
            "{ a: [Getter] }"
        );
        assert_eq!(
            formatted("['%O', { get a() { throw 1; }, b: 2 }, 'x']"),
            "{ a: [Getter], b: 2 } x"
        );
        assert_eq!(
            formatted("['%o', [1, { get a() { throw 1; } }]]"),
            "[ 1, { a: [Getter] } ]"
        );
        assert_eq!(
            formatted("['%o', Object.defineProperty([1, 2], 0, { get() { throw 1; } })]"),
            "[ [Getter], 2 ]"
        );
    }

    /// A proxy of `target` whose handler counts every trap lookup in `calls`.
    fn spied(target: &str) -> String {
        format!(
            "(globalThis.calls = 0, new Proxy({target}, new Proxy({{}}, {{ get() {{ calls++; }} }})))"
        )
    }

    fn calls_after(args: &str) -> (String, i32) {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let values: Vec<Value<'_>> = ctx.eval(args).unwrap();
            let out = line(&ctx, &values, false);
            assert!(!ctx.has_exception());
            (out, ctx.eval("calls").unwrap())
        })
    }

    #[test]
    fn a_proxy_converts_as_its_target_and_calls_no_trap() {
        for target in [
            "[1, 2]",
            "function named() {}",
            "({ toString() { return 'custom'; } })",
            "({ a: 1 })",
            "new Map([[1, 2]])",
        ] {
            for spec in ["%o", "%O", "%s", "%d", "%i", "%f", "%c"] {
                let plain = calls_after(&format!("['{spec}', (globalThis.calls = 0, {target})]"));

                assert_eq!(
                    calls_after(&format!("['{spec}', {}]", spied(target))),
                    (plain.0, 0),
                    "{spec} {target}"
                );
            }
        }
        assert_eq!(
            calls_after("['%s', (globalThis.calls = 0, new Proxy([1, 2], new Proxy({}, { get() { calls++; } })))]").0,
            "1,2"
        );
    }

    #[test]
    fn negative_zero_keeps_its_sign_under_s_and_d() {
        assert_eq!(formatted("['%s %d %i %f', -0, -0, -0, -0]"), "-0 -0 0 0");
        assert_eq!(formatted("['%o %O', -0, -0]"), "-0 -0");
        assert_eq!(formatted("['%s %d', 0, 0]"), "0 0");
        assert_eq!(formatted("['%s', -0, -0]"), "-0 -0");
    }

    #[test]
    fn a_revoked_proxy_converts_without_leaving_an_exception() {
        let revoked =
            "(() => { const r = Proxy.revocable({}, {}); r.revoke(); return r.proxy; })()";
        for (spec, shown) in [
            ("%s", "<Revoked Proxy>"),
            ("%o", "<Revoked Proxy>"),
            ("%O", "<Revoked Proxy>"),
            ("%d", "NaN"),
            ("%i", "NaN"),
            ("%f", "NaN"),
            ("%c", ""),
        ] {
            assert_eq!(
                formatted(&format!("['{spec}', {revoked}]")),
                shown,
                "{spec}"
            );
        }
    }

    #[test]
    fn d_falls_back_to_the_inspected_value_when_the_string_form_throws() {
        assert_eq!(
            formatted("['%d', { toString() { throw new Error('no'); } }, 1]"),
            "{ toString: [Function: toString] } 1"
        );
    }

    #[test]
    fn f_parses_a_float() {
        let f = |arg: &str| formatted(&format!("['%f', {arg}]"));
        assert_eq!(f("'1.5abc'"), "1.5");
        assert_eq!(f("'  .5'"), "0.5");
        assert_eq!(f("'-.5e1'"), "-5");
        assert_eq!(f("'1e'"), "1");
        assert_eq!(f("'Infinityx'"), "Infinity");
        assert_eq!(f("'.'"), "NaN");
        assert_eq!(f("1e21"), "1e+21");
        assert_eq!(f("12n"), "12");
        assert_eq!(f("Symbol('s')"), "NaN");
        assert_eq!(f("true"), "NaN");
        assert_eq!(f("null"), "NaN");
        assert_eq!(f("undefined"), "NaN");
        assert_eq!(f("{}"), "NaN");
        assert_eq!(f("[2.5, 9]"), "2.5");
        assert_eq!(
            formatted("['%f', { toString() { throw 1; } }]"),
            "{ toString: [Function: toString] }"
        );
    }

    #[test]
    fn o_and_o_show_the_value_the_way_dir_does() {
        for spec in ["%o", "%O"] {
            let f = |arg: &str| formatted(&format!("['{spec}', {arg}]"));
            assert_eq!(f("'text'"), "'text'");
            assert_eq!(f("'it\\'s'"), "'it\\'s'");
            assert_eq!(f("5"), "5");
            assert_eq!(f("null"), "null");
            assert_eq!(f("undefined"), "undefined");
            assert_eq!(f("{ a: 'b', c: [1, 'd'] }"), "{ a: 'b', c: [ 1, 'd' ] }");
            assert_eq!(f("[]"), "[]");
            assert_eq!(f("Symbol('s')"), "Symbol(s)");
            assert_eq!(f("7n"), "7n");
            assert_eq!(f("function named() {}"), "[Function: named]");
            assert_eq!(
                f("{ toString() { throw 1; } }"),
                "{ toString: [Function: toString] }"
            );
        }
    }

    #[test]
    fn c_takes_its_argument_and_prints_nothing() {
        assert_eq!(formatted("['a%cb', 'color: red']"), "ab");
        assert_eq!(
            formatted("['%cstyled%c', 'color: red', '', 'tail']"),
            "styled tail"
        );
        assert_eq!(formatted("['%c', 'color: red']"), "");
    }

    #[test]
    fn a_double_percent_is_one_percent_and_takes_no_argument() {
        assert_eq!(formatted("['100%%', 1]"), "100% 1");
        assert_eq!(formatted("['%%', 'x']"), "% x");
        assert_eq!(formatted("['%% %s', 'x']"), "% x");
        assert_eq!(formatted("['%s %%', 'x']"), "x %");
        assert_eq!(formatted("['%%%s', 'x']"), "%x");
        assert_eq!(formatted("['%%s', 'x']"), "%s x");
        assert_eq!(formatted("['%%%%', 'x']"), "%% x");
        assert_eq!(formatted("['%%%', 'x']"), "%% x");
    }

    #[test]
    fn a_lone_or_unknown_percent_stays_as_written() {
        assert_eq!(formatted("['50%', 1]"), "50% 1");
        assert_eq!(formatted("['%', 1]"), "% 1");
        assert_eq!(formatted("['%x %s', 1, 2]"), "%x 1 2");
        assert_eq!(formatted("['%j', 1]"), "%j 1");
        assert_eq!(formatted("['%S %D %F', 1]"), "%S %D %F 1");
        assert_eq!(formatted("['% s', 1]"), "% s 1");
        assert_eq!(formatted("['%é', 1]"), "%é 1");
        assert_eq!(formatted("['%%x%', 1]"), "%x% 1");
    }

    #[test]
    fn a_specifier_with_no_argument_left_stays_as_written() {
        assert_eq!(formatted("['%s %s', 'a']"), "a %s");
        assert_eq!(formatted("['%d %i %f %o %O %c', 1]"), "1 %i %f %o %O %c");
        assert_eq!(formatted("['%s %% %s', 'a']"), "a % %s");
    }

    #[test]
    fn arguments_beyond_the_specifiers_follow_in_order() {
        assert_eq!(
            formatted("['%s', 'a', 'b', 3, { k: 'v' }]"),
            "a b 3 { k: 'v' }"
        );
        assert_eq!(formatted("['no spec', 'a', 1]"), "no spec a 1");
    }

    #[test]
    fn specifiers_take_arguments_left_to_right() {
        assert_eq!(
            formatted("['%s-%d-%f-%o', 'a', '2', '3.5', 'z']"),
            "a-2-3.5-'z'"
        );
        assert_eq!(formatted("['%s %s %s', 1, 2, 3, 4]"), "1 2 3 4");
        assert_eq!(formatted("['%d%%, %s', 50, 'ok']"), "50%, ok");
    }

    #[test]
    fn a_specifier_in_a_later_argument_is_not_one() {
        assert_eq!(formatted("['a', '%s', 'b']"), "a %s b");
        assert_eq!(formatted("[1, '%d %s']"), "1 %d %s");
    }

    #[test]
    fn substituted_text_is_scanned_for_the_next_specifier() {
        assert_eq!(formatted("['%s %s', '%d', 5]"), "5 %s");
        assert_eq!(formatted("['%s %s', '%s', 'x']"), "x %s");
        assert_eq!(formatted("['%s', '%d']"), "%d");
        assert_eq!(formatted("['%s', '%s', 'b']"), "b");
        assert_eq!(formatted("['%s %s', '%%', 'x']"), "% x");
    }

    #[test]
    fn a_specifier_substituted_for_itself_ends_with_the_arguments() {
        assert_eq!(
            formatted("['%s%s%s%s', '%s', '%s', '%s', 'end']"),
            "end%s%s%s"
        );
    }

    #[test]
    fn a_non_ascii_target_is_split_on_byte_boundaries() {
        assert_eq!(formatted("['日本%s語', '・']"), "日本・語");
        assert_eq!(formatted("['é%%é%é', 1]"), "é%é%é 1");
    }

    #[test]
    fn parse_int_matches_the_engines() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            for input in INPUTS {
                ctx.globals().set("x", *input).unwrap();
                let engine: f64 = ctx.eval("parseInt(x, 10)").unwrap();
                same(parse_int(input), engine, input);
            }
        });
    }

    #[test]
    fn parse_float_matches_the_engines() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            for input in INPUTS {
                ctx.globals().set("x", *input).unwrap();
                let engine: f64 = ctx.eval("parseFloat(x)").unwrap();
                same(parse_float(input), engine, input);
            }
        });
    }

    #[test]
    fn white_space_is_the_engines() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            for code in 0..0x3100u32 {
                let Some(c) = char::from_u32(code) else {
                    continue;
                };
                if c.is_ascii_digit() || c == '+' || c == '-' {
                    continue;
                }
                let engine: bool = ctx
                    .eval(format!("parseInt('{}7', 10) === 7", c.escape_unicode()))
                    .unwrap();
                assert_eq!(is_js_space(c), engine, "U+{code:04X}");
            }
        });
    }

    /// Both results are the same number, a `NaN` or a signed zero included.
    fn same(ours: f64, engine: f64, input: &str) {
        assert!(
            ours.to_bits() == engine.to_bits() || (ours.is_nan() && engine.is_nan()),
            "{input:?}: {ours} against the engine's {engine}"
        );
    }

    const INPUTS: &[&str] = &[
        "",
        " ",
        "0",
        "-0",
        "+0",
        "7",
        "-7",
        "+7",
        "007",
        "12.5",
        "-12.5",
        ".5",
        "-.5",
        "5.",
        ".",
        "-",
        "+",
        "e5",
        "1e",
        "1e+",
        "1e-",
        "1e3",
        "1E3",
        "1e+3",
        "1e-3",
        "1.5e3x",
        "1e400",
        "-1e400",
        "1e-400",
        "0x10",
        "0X10",
        "0b11",
        "0o7",
        "Infinity",
        "-Infinity",
        "+Infinity",
        "Infinityx",
        "infinity",
        "NaN",
        "nan",
        "abc",
        "12abc",
        "  12",
        "\t\n\u{b}\u{c}\r 12",
        "\u{a0}12",
        "\u{feff}12",
        "\u{2028}12",
        "\u{2029}12",
        "\u{85}12",
        "\u{200b}12",
        "\u{3000}12",
        "\u{180e}12",
        "- 5",
        "--5",
        "+-5",
        "1_000",
        "1,000",
        "9007199254740993",
        "123456789012345678901234567890",
        "0.1",
        "0.0000001",
        "00.5",
        "1.2.3",
        "1e2e3",
        "٣",
        "１２",
    ];
}
