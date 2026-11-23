// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The number conversions of `%d`, `%i` and `%f`.

use rquickjs::{Coerced, Ctx, Value};

pub(super) fn number_text(ctx: &Ctx<'_>, number: f64) -> String {
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
pub(super) fn parse_int(text: &str) -> f64 {
    let (negative, text) = sign(text.trim_start_matches(is_js_space));
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return f64::NAN;
    }
    let value: f64 = text[..digits].parse().unwrap_or(f64::NAN);
    if negative { -value } else { value }
}

pub(super) fn parse_float(text: &str) -> f64 {
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
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::with_realm;

    #[test]
    fn parse_int_matches_the_engines() {
        with_realm(|ctx| {
            for input in INPUTS {
                ctx.globals().set("x", *input).unwrap();
                let engine: f64 = ctx.eval("parseInt(x, 10)").unwrap();
                same(parse_int(input), engine, input);
            }
        });
    }

    #[test]
    fn parse_float_matches_the_engines() {
        with_realm(|ctx| {
            for input in INPUTS {
                ctx.globals().set("x", *input).unwrap();
                let engine: f64 = ctx.eval("parseFloat(x)").unwrap();
                same(parse_float(input), engine, input);
            }
        });
    }

    #[test]
    fn white_space_is_the_engines() {
        with_realm(|ctx| {
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
