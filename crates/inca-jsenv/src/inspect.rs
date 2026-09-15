// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders a JS value as the text `console` prints.
//!
//! Modelled on Node's `util.inspect` rather than `JSON.stringify`, which is
//! the wrong tool here: it throws on a cycle, and silently drops functions,
//! `undefined` and symbols — exactly the values someone reaches for `console`
//! to look at.

use std::fmt::Write;

use rquickjs::{Coerced, Type, Value};

/// How far into nested arrays and objects to descend before printing
/// `[Array]`/`[Object]` instead. Also what keeps a cycle from recursing
/// forever, since nothing here tracks values already seen.
const MAX_DEPTH: usize = 3;

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
    match value.get::<Coerced<String>>() {
        Ok(text) => {
            let _ = write!(out, "{}{suffix}", text.0);
        }
        Err(_) => out.push_str(fallback),
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

/// Prints `name: message`, then the stack under it when there is one.
///
/// `QuickJS` puts only the frames in `error.stack`, unlike V8 — reporting the
/// stack alone would lose the message, which is the part a reader needs
/// first.
fn write_error(out: &mut String, value: &Value<'_>) {
    let Some(object) = value.as_object() else {
        out.push_str("[Error]");
        return;
    };

    let name = object
        .get::<_, Option<String>>("name")
        .ok()
        .flatten()
        .unwrap_or_else(|| "Error".to_owned());
    match object.get::<_, Option<String>>("message").ok().flatten() {
        Some(message) if !message.is_empty() => {
            let _ = write!(out, "{name}: {message}");
        }
        _ => out.push_str(&name),
    }

    if let Ok(Some(stack)) = object.get::<_, Option<String>>("stack")
        && !stack.trim().is_empty()
    {
        let _ = write!(out, "\n{}", stack.trim_end());
    }
}

fn write_function(out: &mut String, value: &Value<'_>) {
    let name = value
        .as_object()
        .and_then(|object| object.get::<_, Option<String>>("name").ok().flatten())
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
        match item {
            Ok(item) => write_value(out, &item, depth + 1),
            Err(_) => out.push_str("[unreadable]"),
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
    for key in object.keys::<String>().flatten() {
        if entries > 0 {
            body.push_str(", ");
        }
        let _ = write!(&mut body, "{key}: ");
        match object.get::<_, Value<'_>>(key.as_str()) {
            Ok(item) => write_value(&mut body, &item, depth + 1),
            Err(_) => body.push_str("[unreadable]"),
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
