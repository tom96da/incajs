// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders a JS value as the text `console` prints.
//!
//! Modelled on Node's `util.inspect` rather than `JSON.stringify`, which is
//! the wrong tool here: it throws on a cycle, and silently drops functions,
//! `undefined` and symbols — exactly the values someone reaches for `console`
//! to look at.

use std::fmt::Write;

use rquickjs::{Coerced, Ctx, Result as JsResult, Type, Value};

use crate::paint::{Style, paint};

/// Nesting depth after which `[Array]` or `[Object]` is printed. It also bounds a cycle.
const MAX_DEPTH: usize = 3;

/// The value of a read, or `None` after clearing the exception it threw.
pub(crate) fn settled<T>(ctx: &Ctx<'_>, result: JsResult<T>) -> Option<T> {
    if result.is_err() {
        drop(ctx.catch());
    }
    result.ok()
}

pub(crate) fn line(values: &[Value<'_>], color: bool) -> String {
    let mode = Mode {
        color,
        escape: false,
    };
    let mut out = String::new();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        write_value(&mut out, value, 0, mode);
    }
    out
}

/// Renders one value as a nested one: a string is quoted, as in `dir` and
/// `table` cells, and anything else reads as it does in [`line`].
pub(crate) fn quoted(value: &Value<'_>, color: bool) -> String {
    render_quoted(
        value,
        Mode {
            color,
            escape: false,
        },
    )
}

/// As [`quoted`], with every control character in the value's text spelled
/// as an escape sequence, so the result is a single line.
pub(crate) fn quoted_escaped(value: &Value<'_>, color: bool) -> String {
    render_quoted(
        value,
        Mode {
            color,
            escape: true,
        },
    )
}

fn render_quoted(value: &Value<'_>, mode: Mode) -> String {
    let mut out = String::new();
    let depth = usize::from(value.type_of() == Type::String);
    write_value(&mut out, value, depth, mode);
    out
}

/// How a value is rendered.
#[derive(Clone, Copy)]
struct Mode {
    color: bool,
    escape: bool,
}

/// Spells every control character as an escape sequence.
pub(crate) fn escape_controls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() && u32::from(c) < 0x100 => {
                let _ = write!(out, "\\x{:02x}", u32::from(c));
            }
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out
}

/// Appends the value's own text.
fn plain(out: &mut String, mode: Mode, text: &str) {
    if mode.escape {
        out.push_str(&escape_controls(text));
    } else {
        out.push_str(text);
    }
}

fn put(out: &mut String, mode: Mode, style: Style, text: &str) {
    let mut shown = String::new();
    plain(&mut shown, mode, text);
    out.push_str(&paint(mode.color, style, &shown));
}

/// A top-level string prints bare (`console.log('a')` gives `a`); one nested
/// in an array or object is quoted, so `['a']` doesn't read as `[a]`.
fn write_value(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) {
    match value.type_of() {
        Type::String => {
            let text = value.as_string().and_then(|s| s.to_string().ok());
            match (text, depth) {
                (Some(text), 0) => plain(out, mode, &text),
                (Some(text), _) => {
                    let quoted = format!("'{}'", text.replace('\'', "\\'"));
                    put(out, mode, Style::Green, &quoted);
                }
                (None, _) => out.push_str("[String]"),
            }
        }
        Type::Undefined | Type::Uninitialized => put(out, mode, Style::Grey, "undefined"),
        Type::Null => put(out, mode, Style::Bold, "null"),
        Type::Bool => put(
            out,
            mode,
            Style::Yellow,
            if value.as_bool() == Some(true) {
                "true"
            } else {
                "false"
            },
        ),
        Type::Int | Type::Float => {
            let text = match value.as_number() {
                // Rust's own `Display` for `f64` spells these `inf`/`-inf`.
                Some(number) if number.is_infinite() => {
                    if number.is_sign_positive() {
                        "Infinity".to_owned()
                    } else {
                        "-Infinity".to_owned()
                    }
                }
                Some(number) => number.to_string(),
                None => "NaN".to_owned(),
            };
            put(out, mode, Style::Yellow, &text);
        }
        Type::BigInt => write_bigint(out, value, mode),
        Type::Symbol => write_symbol(out, value, mode),
        Type::Exception => write_error(out, value, mode),
        Type::Function | Type::Constructor => write_function(out, value, mode),
        Type::Array => write_array(out, value, depth, mode),
        Type::Object | Type::Promise | Type::Proxy | Type::Module | Type::Unknown => {
            write_object(out, value, depth, mode);
        }
    }
}

/// The value's own string coercion, with the `n` suffix.
fn write_bigint(out: &mut String, value: &Value<'_>, mode: Mode) {
    let text = match settled(value.ctx(), value.get::<Coerced<String>>()) {
        Some(text) => format!("{}n", text.0),
        None => "[BigInt]".to_owned(),
    };
    put(out, mode, Style::Yellow, &text);
}

/// A symbol has no string coercion — `String(sym)` throws — so this reads the
/// description the way `Symbol.prototype.toString` would render it.
fn write_symbol(out: &mut String, value: &Value<'_>, mode: Mode) {
    let description = value
        .as_symbol()
        .and_then(|symbol| symbol.description().ok())
        .and_then(|description| description.get::<Option<String>>().ok().flatten())
        .unwrap_or_default();
    put(out, mode, Style::Green, &format!("Symbol({description})"));
}

/// `name: message`, then the stack, one dimmed line per frame.
fn write_error(out: &mut String, value: &Value<'_>, mode: Mode) {
    let Some(object) = value.as_object() else {
        out.push_str("[Error]");
        return;
    };

    let ctx = object.ctx();
    let name = settled(ctx, object.get::<_, Option<String>>("name"))
        .flatten()
        .unwrap_or_else(|| "Error".to_owned());
    match settled(ctx, object.get::<_, Option<String>>("message")).flatten() {
        Some(message) if !message.is_empty() => plain(out, mode, &format!("{name}: {message}")),
        _ => plain(out, mode, &name),
    }

    if let Some(Some(stack)) = settled(ctx, object.get::<_, Option<String>>("stack"))
        && !stack.trim().is_empty()
    {
        for frame in stack.trim_end().split('\n') {
            out.push_str(if mode.escape { "\\n" } else { "\n" });
            put(out, mode, Style::Grey, frame);
        }
    }
}

fn write_function(out: &mut String, value: &Value<'_>, mode: Mode) {
    let name = value
        .as_object()
        .and_then(|object| settled(object.ctx(), object.get::<_, Option<String>>("name")).flatten())
        .filter(|name| !name.is_empty());
    let text = match name {
        Some(name) => format!("[Function: {name}]"),
        None => "[Function (anonymous)]".to_owned(),
    };
    put(out, mode, Style::Cyan, &text);
}

fn write_array(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) {
    let Some(array) = value.as_array() else {
        put(out, mode, Style::Cyan, "[Array]");
        return;
    };
    if depth >= MAX_DEPTH {
        put(out, mode, Style::Cyan, "[Array]");
        return;
    }
    if array.is_empty() {
        out.push_str("[]");
        return;
    }

    out.push_str("[ ");
    for (index, item) in array.clone().into_iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        match settled(array.ctx(), item) {
            Some(item) => write_value(out, &item, depth + 1, mode),
            None => out.push_str("[unreadable]"),
        }
    }
    out.push_str(" ]");
}

fn write_object(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) {
    let Some(object) = value.as_object() else {
        put(out, mode, Style::Cyan, "[Object]");
        return;
    };
    if depth >= MAX_DEPTH {
        put(out, mode, Style::Cyan, "[Object]");
        return;
    }

    let mut entries = 0;
    let mut body = String::new();
    for key in object.keys::<String>() {
        let Ok(key) = key else {
            drop(object.ctx().catch());
            continue;
        };
        if entries > 0 {
            body.push_str(", ");
        }
        plain(&mut body, mode, &format!("{key}: "));
        match settled(object.ctx(), object.get::<_, Value<'_>>(key.as_str())) {
            Some(item) => write_value(&mut body, &item, depth + 1, mode),
            None => body.push_str("[unreadable]"),
        }
        entries += 1;
    }

    if entries == 0 {
        out.push_str("{}");
    } else {
        let _ = write!(out, "{{ {body} }}");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rquickjs::{Context, Runtime};

    use super::*;

    /// Evaluates `expression` and renders its value the way `console` would
    /// render a single argument.
    fn rendered(expression: &str) -> String {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let value: Value<'_> = ctx.eval(expression).unwrap();
            line(&[value], false)
        })
    }

    #[test]
    fn primitives_read_as_themselves() {
        assert_eq!(rendered("'plain'"), "plain");
        assert_eq!(rendered("42"), "42");
        assert_eq!(rendered("1.5"), "1.5");
        assert_eq!(rendered("true"), "true");
        assert_eq!(rendered("false"), "false");
        assert_eq!(rendered("null"), "null");
        assert_eq!(rendered("undefined"), "undefined");
        assert_eq!(rendered("10n"), "10n");
        assert_eq!(rendered("Symbol('tag')"), "Symbol(tag)");
    }

    #[test]
    fn infinities_read_the_way_js_spells_them() {
        assert_eq!(rendered("1 / 0"), "Infinity");
        assert_eq!(rendered("-1 / 0"), "-Infinity");
    }

    #[test]
    fn a_nested_string_is_quoted_so_it_reads_as_one() {
        assert_eq!(rendered("['a', 1]"), "[ 'a', 1 ]");
        assert_eq!(rendered("({ k: 'v' })"), "{ k: 'v' }");
    }

    #[test]
    fn empty_containers_stay_short() {
        assert_eq!(rendered("[]"), "[]");
        assert_eq!(rendered("({})"), "{}");
    }

    #[test]
    fn nesting_is_walked_until_the_depth_cap() {
        assert_eq!(
            rendered("({ a: { b: { c: 1 } } })"),
            "{ a: { b: { c: 1 } } }"
        );
        assert_eq!(
            rendered("({ a: { b: { c: { d: 1 } } } })"),
            "{ a: { b: { c: [Object] } } }",
            "the cap is what keeps a cycle from recursing forever"
        );
    }

    #[test]
    fn a_cycle_terminates() {
        assert_eq!(
            rendered("const a = {}; a.self = a; a"),
            "{ self: { self: { self: [Object] } } }",
            "JSON.stringify throws here, which is why console does not use it"
        );
    }

    #[test]
    fn an_error_without_a_stack_still_names_itself() {
        assert_eq!(
            rendered("const e = new TypeError('bad'); e.stack = ''; e"),
            "TypeError: bad"
        );
    }

    #[test]
    fn an_error_leads_with_its_message_then_its_frames() {
        let rendered = rendered("new Error('boom')");
        let (first, rest) = rendered.split_once('\n').unwrap();

        assert_eq!(first, "Error: boom", "the message has to come first");
        assert!(rest.contains("at "), "{rest}");
    }

    #[test]
    fn a_function_prints_its_name() {
        assert_eq!(rendered("function named() {}; named"), "[Function: named]");
        assert_eq!(rendered("(() => {})"), "[Function (anonymous)]");
    }

    #[test]
    fn values_json_would_drop_are_kept() {
        // `fn:` gives the arrow function that name, per named evaluation.
        assert_eq!(
            rendered("({ fn: () => {}, missing: undefined })"),
            "{ fn: [Function: fn], missing: undefined }"
        );
    }

    #[test]
    fn quoted_quotes_a_string_and_leaves_the_rest_alone() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
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
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let out = context.with(|ctx| {
            let values: Vec<Value<'_>> = ctx.eval("['a', 1, { k: true }]").unwrap();
            line(&values, false)
        });

        assert_eq!(out, "a 1 { k: true }");
    }

    /// As [`rendered`], with colours on.
    fn colored(expression: &str) -> String {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let value: Value<'_> = ctx.eval(expression).unwrap();
            line(&[value], true)
        })
    }

    fn yellow(text: &str) -> String {
        format!("\x1b[33m{text}\x1b[39m")
    }

    fn green(text: &str) -> String {
        format!("\x1b[32m{text}\x1b[39m")
    }

    fn cyan(text: &str) -> String {
        format!("\x1b[36m{text}\x1b[39m")
    }

    fn grey(text: &str) -> String {
        format!("\x1b[90m{text}\x1b[39m")
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
    fn a_top_level_string_stays_uncoloured() {
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
    fn object_keys_stay_uncoloured() {
        assert_eq!(
            colored("({ k: 'v', n: 1 })"),
            format!("{{ k: {}, n: {} }}", green("'v'"), yellow("1"))
        );
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
    fn empty_containers_and_unreadable_slots_are_uncoloured() {
        assert_eq!(colored("[]"), "[]");
        assert_eq!(colored("({})"), "{}");
        assert_eq!(colored("({ get a() { throw 1; } })"), "{ a: [unreadable] }");
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
    fn arguments_are_coloured_one_by_one_and_joined_plainly() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let out = context.with(|ctx| {
            let values: Vec<Value<'_>> = ctx.eval("['a', 1, null]").unwrap();
            line(&values, true)
        });

        assert_eq!(out, format!("a {} \x1b[1mnull\x1b[22m", yellow("1")));
    }

    #[test]
    fn quoted_colours_a_string_green_and_a_number_yellow() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let text: Value<'_> = ctx.eval("'s'").unwrap();
            let number: Value<'_> = ctx.eval("7").unwrap();

            assert_eq!(quoted(&text, true), green("'s'"));
            assert_eq!(quoted(&number, true), yellow("7"));
        });
    }

    #[test]
    fn colour_never_crosses_a_line_break() {
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

    /// Renders `expression` through [`quoted_escaped`].
    fn escaped(expression: &str, color: bool) -> String {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let value: Value<'_> = ctx.eval(expression).unwrap();
            quoted_escaped(&value, color)
        })
    }

    #[test]
    fn escaped_text_has_no_control_character_left() {
        assert_eq!(
            escaped("['a\\nb\\tc\\x01\\x1b[1m']", false),
            "[ 'a\\nb\\tc\\x01\\x1b[1m' ]"
        );
        assert_eq!(escaped("Symbol('a\\nb')", false), "Symbol(a\\nb)");
        assert_eq!(escaped("({ 'k\\n': 1 })", false), "{ k\\n: 1 }");
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

    #[test]
    fn escaping_and_colour_leave_the_visible_text_alone() {
        let source = "({ s: 'x\\t', n: [1, null], e: new Error('m') })";

        assert_eq!(escaped(source, false), strip_codes(&escaped(source, true)));
    }

    /// `text` without the colour sequences this module paints.
    fn strip_codes(text: &str) -> String {
        let mut out = text.to_owned();
        for code in [31, 32, 33, 34, 35, 36, 1, 90, 39, 22] {
            out = out.replace(&format!("\x1b[{code}m"), "");
        }
        out
    }
}
