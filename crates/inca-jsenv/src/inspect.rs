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

/// Nesting depth after which `[Array]` or `[Object]` is printed. It also bounds a cycle.
const MAX_DEPTH: usize = 3;

/// The value of a read, or `None` after clearing the exception it threw.
pub(crate) fn settled<T>(ctx: &Ctx<'_>, result: JsResult<T>) -> Option<T> {
    if result.is_err() {
        drop(ctx.catch());
    }
    result.ok()
}

/// Formats one `console` call's arguments into the line it prints.
pub(crate) fn line(values: &[Value<'_>]) -> String {
    let mut out = String::new();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        write_value(&mut out, value, 0);
    }
    out
}

/// Renders one value as a nested one: a string is quoted, as in `dir` and
/// `table` cells, and anything else reads as it does in [`line`].
pub(crate) fn quoted(value: &Value<'_>) -> String {
    let mut out = String::new();
    let depth = usize::from(value.type_of() == Type::String);
    write_value(&mut out, value, depth);
    out
}

/// A top-level string prints bare (`console.log('a')` gives `a`); one nested
/// in an array or object is quoted, so `['a']` doesn't read as `[a]`.
fn write_value(out: &mut String, value: &Value<'_>, depth: usize) {
    match value.type_of() {
        Type::String => {
            let text = value.as_string().and_then(|s| s.to_string().ok());
            match (text, depth) {
                (Some(text), 0) => out.push_str(&text),
                (Some(text), _) => {
                    let _ = write!(out, "'{}'", text.replace('\'', "\\'"));
                }
                (None, _) => out.push_str("[String]"),
            }
        }
        Type::Undefined | Type::Uninitialized => out.push_str("undefined"),
        Type::Null => out.push_str("null"),
        Type::Bool => out.push_str(if value.as_bool() == Some(true) {
            "true"
        } else {
            "false"
        }),
        Type::Int | Type::Float => match value.as_number() {
            // Rust's own `Display` for `f64` spells these `inf`/`-inf`.
            Some(number) if number.is_infinite() => {
                out.push_str(if number.is_sign_positive() {
                    "Infinity"
                } else {
                    "-Infinity"
                });
            }
            Some(number) => {
                let _ = write!(out, "{number}");
            }
            None => out.push_str("NaN"),
        },
        Type::BigInt => write_coerced(out, value, "[BigInt]", "n"),
        Type::Symbol => write_symbol(out, value),
        Type::Exception => write_error(out, value),
        Type::Function | Type::Constructor => write_function(out, value),
        Type::Array => write_array(out, value, depth),
        Type::Object | Type::Promise | Type::Proxy | Type::Module | Type::Unknown => {
            write_object(out, value, depth);
        }
    }
}

/// Falls back to the value's own string coercion, for the types with no
/// structure worth walking.
fn write_coerced(out: &mut String, value: &Value<'_>, fallback: &str, suffix: &str) {
    match settled(value.ctx(), value.get::<Coerced<String>>()) {
        Some(text) => {
            let _ = write!(out, "{}{suffix}", text.0);
        }
        None => out.push_str(fallback),
    }
}

/// A symbol has no string coercion — `String(sym)` throws — so this reads the
/// description the way `Symbol.prototype.toString` would render it.
fn write_symbol(out: &mut String, value: &Value<'_>) {
    let description = value
        .as_symbol()
        .and_then(|symbol| symbol.description().ok())
        .and_then(|description| description.get::<Option<String>>().ok().flatten())
        .unwrap_or_default();
    let _ = write!(out, "Symbol({description})");
}

/// `name: message`, then the stack. `QuickJS`'s `stack` carries frames only.
fn write_error(out: &mut String, value: &Value<'_>) {
    let Some(object) = value.as_object() else {
        out.push_str("[Error]");
        return;
    };

    let ctx = object.ctx();
    let name = settled(ctx, object.get::<_, Option<String>>("name"))
        .flatten()
        .unwrap_or_else(|| "Error".to_owned());
    match settled(ctx, object.get::<_, Option<String>>("message")).flatten() {
        Some(message) if !message.is_empty() => {
            let _ = write!(out, "{name}: {message}");
        }
        _ => out.push_str(&name),
    }

    if let Some(Some(stack)) = settled(ctx, object.get::<_, Option<String>>("stack"))
        && !stack.trim().is_empty()
    {
        let _ = write!(out, "\n{}", stack.trim_end());
    }
}

fn write_function(out: &mut String, value: &Value<'_>) {
    let name = value
        .as_object()
        .and_then(|object| settled(object.ctx(), object.get::<_, Option<String>>("name")).flatten())
        .filter(|name| !name.is_empty());
    match name {
        Some(name) => {
            let _ = write!(out, "[Function: {name}]");
        }
        None => out.push_str("[Function (anonymous)]"),
    }
}

fn write_array(out: &mut String, value: &Value<'_>, depth: usize) {
    let Some(array) = value.as_array() else {
        out.push_str("[Array]");
        return;
    };
    if depth >= MAX_DEPTH {
        out.push_str("[Array]");
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
            Some(item) => write_value(out, &item, depth + 1),
            None => out.push_str("[unreadable]"),
        }
    }
    out.push_str(" ]");
}

fn write_object(out: &mut String, value: &Value<'_>, depth: usize) {
    let Some(object) = value.as_object() else {
        out.push_str("[Object]");
        return;
    };
    if depth >= MAX_DEPTH {
        out.push_str("[Object]");
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
        let _ = write!(&mut body, "{key}: ");
        match settled(object.ctx(), object.get::<_, Value<'_>>(key.as_str())) {
            Some(item) => write_value(&mut body, &item, depth + 1),
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
            line(&[value])
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

            assert_eq!(quoted(&text), "'it\\'s'");
            assert_eq!(quoted(&object), "{ a: 'b' }");
            assert_eq!(quoted(&number), "7");
        });
    }

    #[test]
    fn several_arguments_are_space_separated() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let out = context.with(|ctx| {
            let values: Vec<Value<'_>> = ctx.eval("['a', 1, { k: true }]").unwrap();
            line(&values)
        });

        assert_eq!(out, "a 1 { k: true }");
    }
}
