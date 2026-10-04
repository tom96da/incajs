// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders a JS value as the text `console` prints.
//!
//! Modelled on Node's `util.inspect` rather than `JSON.stringify`, which is
//! the wrong tool here: it throws on a cycle, and silently drops functions,
//! `undefined` and symbols — exactly the values someone reaches for `console`
//! to look at.

use std::fmt::Write;

use rquickjs::{Coerced, Ctx, Object, Result as JsResult, Type, Value, object::Filter, qjs};

use crate::paint::{Style, paint};

mod builtins;

pub(crate) use builtins::pin;

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

/// The target a `Proxy` forwards to, or `None` for a revoked one. Reading it
/// runs no trap.
fn proxy_target<'js>(proxy: &Value<'js>) -> Option<Value<'js>> {
    let ctx = proxy.ctx();
    // SAFETY: `proxy` is a live proxy in `ctx`. The call returns a new
    // reference to the target, or the exception value (which owns nothing)
    // after throwing for a revoked proxy. `Value::from_raw` takes over the
    // reference, so it is released exactly once, when the value drops.
    let target = unsafe {
        Value::from_raw(
            ctx.clone(),
            qjs::JS_GetProxyTarget(ctx.as_raw().as_ptr(), proxy.as_raw()),
        )
    };
    if target.is_exception() {
        drop(ctx.catch());
        return None;
    }
    Some(target)
}

/// The value itself, or for a `Proxy` the target it finally forwards to
/// (`None` when a link of the chain is revoked). A proxy's target exists
/// before the proxy does, so a chain cannot loop.
pub(crate) fn unproxied<'js>(value: &Value<'js>) -> Option<Value<'js>> {
    let mut current = value.clone();
    while current.is_proxy() {
        current = proxy_target(&current)?;
    }
    Some(current)
}

/// A top-level string prints bare (`console.log('a')` gives `a`); one nested
/// in an array or object is quoted, so `['a']` doesn't read as `[a]`.
fn write_value(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) {
    let Some(value) = &unproxied(value) else {
        put(out, mode, Style::Cyan, "<Revoked Proxy>");
        return;
    };
    match value.type_of() {
        Type::String => {
            let text = value.as_string().and_then(|s| s.to_string().ok());
            match (text, depth) {
                (Some(text), 0) => plain(out, mode, &text),
                (Some(text), _) => {
                    put(out, mode, Style::Green, &quote(&text));
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
    let object = array.as_object();
    let ctx = object.ctx();
    // A length above `i32::MAX` is a float, so `Array::len` cannot read it.
    let length = settled(ctx, object.get::<_, f64>("length")).unwrap_or(0.0);
    if length < 1.0 {
        out.push_str("[]");
        return;
    }

    out.push_str("[ ");
    let mut written = false;
    let mut next = 0.0;
    // Only the present indices are visited, in ascending order, so a hole run
    // of any size costs one step.
    for key in object.own_keys::<String>(Filter::new().string()) {
        let Some(index) = settled(ctx, key).as_deref().and_then(array_index) else {
            continue;
        };
        if index >= length {
            continue;
        }
        if index > next {
            write_holes(out, &mut written, index - next, mode);
        }
        separate(out, &mut written);
        write_slot(out, slot(object, &index.to_string()), depth, mode);
        next = index + 1.0;
    }
    if length > next {
        write_holes(out, &mut written, length - next, mode);
    }
    out.push_str(" ]");
}

/// The index an array key names, or `None` for any other key.
fn array_index(key: &str) -> Option<f64> {
    let index: u32 = key.parse().ok()?;
    (index != u32::MAX && index.to_string() == key).then_some(f64::from(index))
}

/// Starts the next entry of an array.
fn separate(out: &mut String, written: &mut bool) {
    if *written {
        out.push_str(", ");
    }
    *written = true;
}

fn write_holes(out: &mut String, written: &mut bool, count: f64, mode: Mode) {
    separate(out, written);
    let count = count.to_string();
    let plural = if count == "1" { "" } else { "s" };
    put(
        out,
        mode,
        Style::Grey,
        &format!("<{count} empty item{plural}>"),
    );
}

/// `text` in single quotes.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "\\'"))
}

/// A key that is an identifier prints as it is; any other is quoted.
fn write_key(out: &mut String, key: &str, mode: Mode) {
    let mut chars = key.chars();
    let bare = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if bare {
        plain(out, mode, key);
    } else {
        put(out, mode, Style::Green, &quote(key));
    }
    out.push_str(": ");
}

/// What an own property holds.
enum Slot<'js> {
    Value(Value<'js>),
    /// `[Getter]`, `[Setter]` or `[Getter/Setter]`.
    Accessor(&'static str),
    /// No such property, or a read that threw (the exception is cleared).
    Missing,
}

/// Reads an own property's descriptor, so an accessor is named and never
/// called.
fn slot<'js>(object: &Object<'js>, key: &str) -> Slot<'js> {
    let ctx = object.ctx();
    // The atom comes from a JS string, so a non-ASCII key finds its own
    // property.
    let Some(name) = settled(ctx, rquickjs::String::from_str(ctx.clone(), key)) else {
        return Slot::Missing;
    };
    let raw_ctx = ctx.as_raw().as_ptr();
    let mut desc = std::mem::MaybeUninit::<qjs::JSPropertyDescriptor>::uninit();
    // SAFETY: `desc` is only read after a positive result, which fills it.
    // The atom is released right after the call.
    let found = unsafe {
        let atom = qjs::JS_ValueToAtom(raw_ctx, name.as_value().as_raw());
        if atom == qjs::JS_ATOM_NULL {
            drop(ctx.catch());
            return Slot::Missing;
        }
        let found =
            qjs::JS_GetOwnProperty(raw_ctx, desc.as_mut_ptr(), object.as_value().as_raw(), atom);
        qjs::JS_FreeAtom(raw_ctx, atom);
        found
    };
    if found < 0 {
        drop(ctx.catch());
    }
    if found <= 0 {
        return Slot::Missing;
    }
    // SAFETY: a positive result filled `desc` with three owned values.
    // `Value::from_raw` takes each over, so each is released once.
    let (flags, value, getter, setter) = unsafe {
        let desc = desc.assume_init();
        (
            desc.flags,
            Value::from_raw(ctx.clone(), desc.value),
            Value::from_raw(ctx.clone(), desc.getter),
            Value::from_raw(ctx.clone(), desc.setter),
        )
    };
    let accessor = flags & qjs::JS_PROP_GETSET.cast_signed() != 0;
    match (accessor, getter.is_undefined(), setter.is_undefined()) {
        (true, false, false) => Slot::Accessor("[Getter/Setter]"),
        (true, false, true) => Slot::Accessor("[Getter]"),
        (true, true, false) => Slot::Accessor("[Setter]"),
        // A data property, or an accessor with neither function, whose value
        // is `undefined`.
        _ => Slot::Value(value),
    }
}

/// Appends what an own property holds. `undefined` stands for a missing one.
fn write_slot(out: &mut String, slot: Slot<'_>, depth: usize, mode: Mode) {
    match slot {
        Slot::Value(item) => write_value(out, &item, depth + 1, mode),
        Slot::Accessor(text) => put(out, mode, Style::Cyan, text),
        Slot::Missing => put(out, mode, Style::Grey, "undefined"),
    }
}

fn write_object(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) {
    if builtins::write(out, value, depth, mode) {
        return;
    }
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
        write_key(&mut body, &key, mode);
        write_slot(&mut body, slot(object, &key), depth, mode);
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
            pin(&ctx).unwrap();
            let value: Value<'_> = ctx.eval(expression).unwrap();
            let out = line(&[value], false);
            assert!(!ctx.has_exception(), "a read left an exception pending");
            out
        })
    }

    #[test]
    fn a_module_namespace_with_an_uninitialised_export_prints_and_leaves_nothing_pending() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            pin(&ctx).unwrap();
            // The module throws before `x` is initialised, so reading `x` throws.
            drop(rquickjs::Module::evaluate(
                ctx.clone(),
                "m",
                "import * as self from 'm'; globalThis.ns = self; throw 1; export let x = 1;",
            ));
            drop(ctx.catch());
            let value: Value<'_> = ctx.eval("ns").unwrap();
            let _ = line(&[value], false);

            assert!(!ctx.has_exception(), "a read left an exception pending");
        });
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

    /// Renders `expression` with a `calls` counter in scope, and returns the
    /// text and the counter.
    fn counted(expression: &str) -> (String, i32) {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            pin(&ctx).unwrap();
            ctx.eval::<(), _>("globalThis.calls = 0;").unwrap();
            let value: Value<'_> = ctx.eval(expression).unwrap();
            let text = line(&[value], false);
            assert!(!ctx.has_exception());
            (text, ctx.eval("calls").unwrap())
        })
    }

    #[test]
    fn an_accessor_is_named_by_its_kind_and_never_called() {
        for (source, shown) in [
            ("({ get a() { calls++; return 1; } })", "{ a: [Getter] }"),
            ("({ set a(v) { calls++; } })", "{ a: [Setter] }"),
            (
                "({ get a() { calls++; }, set a(v) { calls++; } })",
                "{ a: [Getter/Setter] }",
            ),
            (
                "Object.defineProperty({}, 'a', { get() { calls++; }, enumerable: true })",
                "{ a: [Getter] }",
            ),
            (
                "Object.defineProperty({}, 'a', { set: undefined, get: undefined, enumerable: true })",
                "{ a: undefined }",
            ),
        ] {
            assert_eq!(counted(source), (shown.to_owned(), 0), "{source}");
        }
    }

    #[test]
    fn accessors_sit_among_data_properties_in_order() {
        assert_eq!(
            rendered("({ x: 1, get y() { return 2; }, z: 'z', set w(v) {} })"),
            "{ x: 1, y: [Getter], z: 'z', w: [Setter] }"
        );
    }

    #[test]
    fn an_accessor_nests_in_objects_arrays_and_collections() {
        assert_eq!(
            rendered("({ a: { b: { get c() { return 1; } } } })"),
            "{ a: { b: { c: [Getter] } } }"
        );
        assert_eq!(
            rendered("[{ get a() { return 1; } }]"),
            "[ { a: [Getter] } ]"
        );
        assert_eq!(
            rendered("new Map([[1, { get a() { return 1; } }]])"),
            "Map(1) { 1 => { a: [Getter] } }"
        );
        assert_eq!(
            rendered("new Set([{ set a(v) {} }])"),
            "Set(1) { { a: [Setter] } }"
        );
    }

    #[test]
    fn an_accessor_past_the_depth_limit_is_not_reached() {
        assert_eq!(
            counted("({ a: { b: { c: { get d() { calls++; } } } } })"),
            ("{ a: { b: { c: [Object] } } }".to_owned(), 0)
        );
    }

    #[test]
    fn an_array_element_accessor_is_named_and_not_called() {
        assert_eq!(
            counted(
                "const a = [1, 2, 3]; \
                 Object.defineProperty(a, 1, { get() { calls++; }, enumerable: true }); \
                 Object.defineProperty(a, 2, { set(v) { calls++; }, enumerable: true }); a"
            ),
            ("[ 1, [Getter], [Setter] ]".to_owned(), 0)
        );
    }

    #[test]
    fn array_holes_print_as_counted_empty_items() {
        for (source, shown) in [
            ("[1, , 3]", "[ 1, <1 empty item>, 3 ]"),
            ("[, 1]", "[ <1 empty item>, 1 ]"),
            ("[1, , ]", "[ 1, <1 empty item> ]"),
            (
                "[1, , , 4, , 6]",
                "[ 1, <2 empty items>, 4, <1 empty item>, 6 ]",
            ),
            ("new Array(3)", "[ <3 empty items> ]"),
            (
                "[undefined, , undefined]",
                "[ undefined, <1 empty item>, undefined ]",
            ),
        ] {
            assert_eq!(rendered(source), shown, "{source}");
        }
        assert_eq!(
            colored("[1, , 3]"),
            format!(
                "[ {}, {}, {} ]",
                yellow("1"),
                grey("<1 empty item>"),
                yellow("3")
            )
        );
    }

    #[test]
    fn a_sparse_array_costs_its_present_elements_and_not_its_length() {
        let started = std::time::Instant::now();
        for (source, shown) in [
            (
                "const a = []; a[1e9] = 1; a",
                "[ <1000000000 empty items>, 1 ]",
            ),
            ("new Array(4294967295)", "[ <4294967295 empty items> ]"),
            (
                "const a = []; a.length = 2 ** 31; a",
                "[ <2147483648 empty items> ]",
            ),
            (
                "const a = []; a[4294967294] = 1; a",
                "[ <4294967294 empty items>, 1 ]",
            ),
            (
                "const a = []; a[2e9] = 'y'; a[5e8] = 'x'; a",
                "[ <500000000 empty items>, 'x', <1499999999 empty items>, 'y' ]",
            ),
            (
                "const a = []; a[5] = 1; a[2] = 2; a",
                "[ <2 empty items>, 2, <2 empty items>, 1 ]",
            ),
            (
                "const a = [1, 2, 3]; a.length = 2 ** 32 - 1; a",
                "[ 1, 2, 3, <4294967292 empty items> ]",
            ),
            (
                "const a = []; Object.defineProperty(a, 1, { get() { calls++; } }); a[4] = 1; a",
                "[ <1 empty item>, [Getter], <2 empty items>, 1 ]",
            ),
            (
                "({ a: { b: { c: (() => { const a = []; a[1e9] = 1; return a; })() } } })",
                "{ a: { b: { c: [Array] } } }",
            ),
        ] {
            assert_eq!(counted(source), (shown.to_owned(), 0), "{source}");
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_class_getter_lives_on_the_prototype_and_is_neither_shown_nor_run() {
        assert_eq!(
            counted("new (class { x = 1; get y() { calls++; return 2; } })()"),
            ("{ x: 1 }".to_owned(), 0)
        );
        assert_eq!(
            counted("Object.create({ get a() { calls++; } })"),
            ("{}".to_owned(), 0)
        );
    }

    #[test]
    fn a_class_instance_accessor_defined_on_the_instance_is_named() {
        assert_eq!(
            counted(
                "const o = new (class { constructor() { \
                 Object.defineProperty(this, 'v', { get: () => { calls++; }, enumerable: true }); } })(); o"
            ),
            ("{ v: [Getter] }".to_owned(), 0)
        );
    }

    #[test]
    fn a_hidden_accessor_stays_hidden() {
        assert_eq!(
            counted("Object.defineProperty({ a: 1 }, 'h', { get() { calls++; } })"),
            ("{ a: 1 }".to_owned(), 0)
        );
    }

    #[test]
    fn a_symbol_keyed_accessor_stays_unlisted_and_is_not_run() {
        assert_eq!(
            counted("({ get [Symbol('s')]() { calls++; } })"),
            ("{}".to_owned(), 0)
        );
    }

    #[test]
    fn a_non_ascii_key_finds_its_own_accessor() {
        assert_eq!(
            rendered("({ '\u{c3}\u{a9}': 2, '\u{e9}': 1 })"),
            "{ '\u{c3}\u{a9}': 2, '\u{e9}': 1 }"
        );
        assert_eq!(
            counted("({ '\u{c3}\u{a9}': 2, get '\u{e9}'() { calls++; } })"),
            ("{ '\u{c3}\u{a9}': 2, '\u{e9}': [Getter] }".to_owned(), 0)
        );
    }

    #[test]
    fn a_numeric_and_an_odd_key_find_their_accessor() {
        assert_eq!(
            counted("({ get 1() { calls++; }, get 'a b'() { calls++; }, get 'é'() { calls++; } })"),
            (
                "{ '1': [Getter], 'a b': [Getter], 'é': [Getter] }".to_owned(),
                0
            )
        );
    }

    #[test]
    fn a_data_property_holding_a_function_is_not_an_accessor() {
        assert_eq!(rendered("({ f() {} })"), "{ f: [Function: f] }");
    }

    #[test]
    fn accessor_text_has_the_same_words_with_colour_on_or_off() {
        let source = "({ get a() { return 1; }, set b(v) {}, get c() { return 1; }, set c(v) {} })";

        assert_eq!(rendered(source), strip_codes(&colored(source)));
    }

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
            pin(&ctx).unwrap();
            let value: Value<'_> = ctx.eval(expression).unwrap();
            let out = line(&[value], true);
            assert!(!ctx.has_exception(), "a read left an exception pending");
            out
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
    fn an_identifier_key_is_uncoloured_and_a_quoted_key_is_green() {
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
    fn empty_containers_are_uncoloured() {
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
            pin(&ctx).unwrap();
            let value: Value<'_> = ctx.eval(expression).unwrap();
            let out = quoted_escaped(&value, color);
            assert!(!ctx.has_exception(), "a read left an exception pending");
            out
        })
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

    #[test]
    fn escaping_and_colour_leave_the_visible_text_alone() {
        let source = "({ s: 'x\\t', n: [1, null], e: new Error('m') })";

        assert_eq!(escaped(source, false), strip_codes(&escaped(source, true)));
    }

    /// Pins the intrinsics, runs `setup`, then renders `expression`.
    fn after(setup: &str, expression: &str, color: bool) -> String {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            pin(&ctx).unwrap();
            ctx.eval::<(), _>(setup).unwrap();
            let value: Value<'_> = ctx.eval(expression).unwrap();
            let out = line(&[value], color);
            assert!(!ctx.has_exception(), "a read left an exception pending");
            out
        })
    }

    fn magenta(text: &str) -> String {
        format!("\x1b[35m{text}\x1b[39m")
    }

    fn red(text: &str) -> String {
        format!("\x1b[31m{text}\x1b[39m")
    }

    #[test]
    fn a_date_prints_its_iso_string() {
        assert_eq!(rendered("new Date(0)"), "1970-01-01T00:00:00.000Z");
        assert_eq!(rendered("new Date(-1)"), "1969-12-31T23:59:59.999Z");
        assert_eq!(
            rendered("new Date(Date.UTC(2024, 1, 29, 13, 5, 9, 7))"),
            "2024-02-29T13:05:09.007Z"
        );
    }

    #[test]
    fn a_date_outside_four_digit_years_keeps_the_engines_signed_form() {
        assert_eq!(rendered("new Date(8.64e15)"), "+275760-09-13T00:00:00.000Z");
        assert_eq!(
            rendered("new Date(-8.64e15)"),
            "-271821-04-20T00:00:00.000Z"
        );
        assert_eq!(
            rendered("new Date(Date.UTC(12345, 0, 1))"),
            "+012345-01-01T00:00:00.000Z"
        );
        assert_eq!(
            rendered("new Date(-62198755200000)"),
            "-000001-01-01T00:00:00.000Z"
        );
    }

    #[test]
    fn an_invalid_date_says_so() {
        assert_eq!(rendered("new Date(NaN)"), "Invalid Date");
        assert_eq!(rendered("new Date('nope')"), "Invalid Date");
        assert_eq!(rendered("new Date(8.64e15 + 1)"), "Invalid Date");
    }

    #[test]
    fn a_date_prints_nothing_but_its_time() {
        assert_eq!(
            rendered("Object.assign(new Date(0), { x: 1 })"),
            "1970-01-01T00:00:00.000Z",
            "Node adds the own properties after the time"
        );
    }

    #[test]
    fn a_regexp_prints_its_source_and_flags() {
        assert_eq!(rendered("/a\\/b/gi"), "/a\\/b/gi");
        assert_eq!(rendered("/x/"), "/x/");
        assert_eq!(rendered("new RegExp('')"), "/(?:)/");
        assert_eq!(rendered("new RegExp('\\n', 'dgimsuy')"), "/\\n/dgimsuy");
        assert_eq!(
            rendered("const r = /x/g; r.lastIndex = 3; r"),
            "/x/g",
            "lastIndex is not part of the text"
        );
        assert_eq!(
            rendered("Object.assign(/x/, { k: 1 })"),
            "/x/",
            "Node adds the own properties after the pattern"
        );
    }

    #[test]
    fn a_map_prints_its_size_and_entries() {
        assert_eq!(
            rendered("new Map([['a', 1], ['b', { c: 2 }]])"),
            "Map(2) { 'a' => 1, 'b' => { c: 2 } }"
        );
        assert_eq!(
            rendered("new Map([[{ a: 1 }, [1]]])"),
            "Map(1) { { a: 1 } => [ 1 ] }"
        );
        assert_eq!(rendered("new Map([[1, 2], [1, 3]])"), "Map(1) { 1 => 3 }");
        assert_eq!(rendered("new Map()"), "Map(0) {}");
    }

    #[test]
    fn a_set_prints_its_size_and_members() {
        assert_eq!(rendered("new Set([1, 2])"), "Set(2) { 1, 2 }");
        assert_eq!(rendered("new Set(['x', ['y']])"), "Set(2) { 'x', [ 'y' ] }");
        assert_eq!(rendered("new Set([1, 1])"), "Set(1) { 1 }");
        assert_eq!(rendered("new Set()"), "Set(0) {}");
    }

    #[test]
    fn collections_follow_the_insertion_order_and_hold_any_value() {
        assert_eq!(
            rendered("new Map([[null, undefined], [NaN, 1n], [Symbol('s'), true]])"),
            "Map(3) { null => undefined, NaN => 1n, Symbol(s) => true }"
        );
        assert_eq!(
            rendered("new Set([new Date(0), /x/g, new Map(), new Set()])"),
            "Set(4) { 1970-01-01T00:00:00.000Z, /x/g, Map(0) {}, Set(0) {} }"
        );
    }

    #[test]
    fn the_four_types_nest_in_arrays_and_objects() {
        assert_eq!(
            rendered("[new Date(0), /x/g, new Map([[1, 2]]), new Set([1])]"),
            "[ 1970-01-01T00:00:00.000Z, /x/g, Map(1) { 1 => 2 }, Set(1) { 1 } ]"
        );
        assert_eq!(
            rendered("({ d: new Date(0), r: /x/, m: new Map(), s: new Set([1]) })"),
            "{ d: 1970-01-01T00:00:00.000Z, r: /x/, m: Map(0) {}, s: Set(1) { 1 } }"
        );
    }

    #[test]
    fn a_collection_stops_at_the_depth_limit() {
        assert_eq!(
            rendered("({ a: { b: { c: new Map([[1, 2]]) } } })"),
            "{ a: { b: { c: [Map] } } }"
        );
        assert_eq!(rendered("[[[new Set([1])]]]"), "[ [ [ [Set] ] ] ]");
        assert_eq!(
            rendered("({ a: { b: new Map([[1, { c: 1 }]]) } })"),
            "{ a: { b: Map(1) { 1 => [Object] } } }"
        );
        assert_eq!(
            rendered("new Map([[1, new Map([[2, new Map([[3, new Map([[4, 5]])]])]])]])"),
            "Map(1) { 1 => Map(1) { 2 => Map(1) { 3 => [Map] } } }"
        );
    }

    #[test]
    fn a_date_and_a_regexp_have_no_depth_limit() {
        assert_eq!(
            rendered("({ a: { b: { c: new Date(0), d: /x/ } } })"),
            "{ a: { b: { c: 1970-01-01T00:00:00.000Z, d: /x/ } } }"
        );
    }

    #[test]
    fn a_collection_that_holds_itself_ends_at_the_depth_limit() {
        assert_eq!(
            rendered("const m = new Map(); m.set('self', m); m"),
            "Map(1) { 'self' => Map(1) { 'self' => Map(1) { 'self' => [Map] } } }",
            "Node marks the cycle instead"
        );
        assert_eq!(
            rendered("const s = new Set(); s.add(s); s"),
            "Set(1) { Set(1) { Set(1) { [Set] } } }"
        );
    }

    #[test]
    fn a_large_collection_prints_every_entry() {
        let out = rendered("new Set(Array.from({ length: 100000 }, (_, i) => i))");

        assert!(out.starts_with("Set(100000) { 0, 1, 2, "), "{}", &out[..40]);
        assert!(out.ends_with(", 99998, 99999 }"));
        let out = rendered("new Map(Array.from({ length: 100000 }, (_, i) => [i, i]))");

        assert!(
            out.starts_with("Map(100000) { 0 => 0, 1 => 1, "),
            "{}",
            &out[..40]
        );
    }

    #[test]
    fn replacing_the_date_machinery_does_not_change_a_date() {
        for tamper in [
            "Date.prototype.toISOString = () => 'hijacked';",
            "Date.prototype.getTime = () => { throw new Error('no'); };",
            "Date.prototype.valueOf = () => { throw new Error('no'); };",
            "Object.defineProperty(Date.prototype, Symbol.toPrimitive, { value() { throw 1; } });",
            "globalThis.Date = function () {};",
            "Object.defineProperty(globalThis, 'Date', { get() { throw 1; } });",
        ] {
            let setup =
                format!("globalThis.d = new Date(0); globalThis.bad = new Date(NaN); {tamper}");
            assert_eq!(
                after(&setup, "[d, bad]", false),
                "[ 1970-01-01T00:00:00.000Z, Invalid Date ]",
                "{tamper}"
            );
        }
    }

    #[test]
    fn a_date_ignores_its_own_overrides() {
        let date = "const d = new Date(0); \
                    d.toISOString = () => 'own'; d.valueOf = () => { throw 1; }; \
                    d.getTime = () => { throw 2; }; Object.defineProperty(d, Symbol.toPrimitive, { value() { throw 3; } }); d";
        assert_eq!(rendered(date), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn replacing_the_collection_and_regexp_machinery_changes_nothing() {
        let setup = "globalThis.m = new Map([[1, 2]]); globalThis.s = new Set([1]); \
                     globalThis.r = /x/g; Map.prototype.forEach = () => { throw 1; }; \
                     Map.prototype.entries = () => { throw 2; }; \
                     Map.prototype[Symbol.iterator] = () => { throw 3; }; \
                     Set.prototype.forEach = () => { throw 4; }; \
                     Set.prototype.values = () => { throw 5; }; \
                     Set.prototype[Symbol.iterator] = () => { throw 6; }; \
                     Object.defineProperty(Map.prototype, 'size', { get() { throw 7; } }); \
                     Object.defineProperty(Set.prototype, 'size', { get() { throw 8; } }); \
                     Object.defineProperty(RegExp.prototype, 'source', { get() { throw 9; } }); \
                     RegExp.prototype.toString = () => 'hijacked'; \
                     globalThis.Map = globalThis.Set = globalThis.RegExp = undefined;";
        assert_eq!(after(setup, "m", false), "Map(1) { 1 => 2 }");
        assert_eq!(after(setup, "s", false), "Set(1) { 1 }");
        assert_eq!(after(setup, "r", false), "/x/g");
    }

    #[test]
    fn a_collections_own_overrides_are_ignored() {
        assert_eq!(
            rendered(
                "const m = new Map([[1, 2]]); m.forEach = () => { throw 1; }; \
                 m.entries = () => { throw 2; }; m[Symbol.iterator] = () => { throw 3; }; \
                 Object.defineProperty(m, 'size', { value: 99 }); m"
            ),
            "Map(1) { 1 => 2 }"
        );
        assert_eq!(
            rendered(
                "const s = new Set([1]); s.forEach = () => { throw 1; }; \
                 s.values = () => { throw 2; }; s[Symbol.iterator] = () => { throw 3; }; s"
            ),
            "Set(1) { 1 }"
        );
    }

    #[test]
    fn a_subclass_prints_as_its_builtin() {
        assert_eq!(
            rendered("new (class A extends Map {})([[1, 2]])"),
            "Map(1) { 1 => 2 }",
            "Node prefixes the subclass name"
        );
        assert_eq!(
            rendered("new (class B extends Set {})([1])"),
            "Set(1) { 1 }"
        );
        assert_eq!(
            rendered("new (class C extends Date {})(0)"),
            "1970-01-01T00:00:00.000Z"
        );
        assert_eq!(
            rendered("new (class D extends RegExp {})('x', 'g')"),
            "/x/g"
        );
    }

    #[test]
    fn a_null_prototype_collection_still_prints() {
        assert_eq!(
            rendered("const m = new Map([[1, 2]]); Object.setPrototypeOf(m, null); m"),
            "Map(1) { 1 => 2 }"
        );
        assert_eq!(
            rendered("const d = new Date(0); Object.setPrototypeOf(d, null); d"),
            "1970-01-01T00:00:00.000Z"
        );
    }

    /// A `Proxy` around `target` whose handler counts every trap lookup in
    /// `calls`.
    fn spied(target: &str) -> String {
        format!(
            "new Proxy({target}, new Proxy({{}}, {{ get() {{ calls++; return undefined; }} }}))"
        )
    }

    #[test]
    fn a_proxy_prints_as_its_target() {
        for (target, shown) in [
            ("new Map([[1, 2]])", "Map(1) { 1 => 2 }"),
            ("new Set([1])", "Set(1) { 1 }"),
            ("new Date(0)", "1970-01-01T00:00:00.000Z"),
            ("/a/g", "/a/g"),
            ("[1, 'a']", "[ 1, 'a' ]"),
            ("[]", "[]"),
            ("({ a: 1 })", "{ a: 1 }"),
            ("({})", "{}"),
            ("function named() {}", "[Function: named]"),
            (
                "({ get a() { calls++; }, set b(v) {} })",
                "{ a: [Getter], b: [Setter] }",
            ),
        ] {
            assert_eq!(counted(&spied(target)), (shown.to_owned(), 0), "{target}");
        }
    }

    #[test]
    fn a_proxy_chain_prints_as_its_last_target() {
        assert_eq!(
            counted(&spied(&spied(&spied("new Map([[1, 2]])")))),
            ("Map(1) { 1 => 2 }".to_owned(), 0)
        );
    }

    #[test]
    fn a_revoked_proxy_says_so() {
        for source in [
            "const r = Proxy.revocable(new Map(), {}); r.revoke(); r.proxy",
            "const r = Proxy.revocable({}, {}); r.revoke(); r.proxy",
            "const r = Proxy.revocable(function () {}, {}); r.revoke(); r.proxy",
            "const r = Proxy.revocable({}, {}); const p = new Proxy(r.proxy, {}); r.revoke(); p",
        ] {
            assert_eq!(rendered(source), "<Revoked Proxy>", "{source}");
        }
    }

    #[test]
    fn a_proxy_nests_in_every_container() {
        assert_eq!(
            counted(&format!(
                "({{ o: {0}, a: [{1}], m: new Map([[{0}, {2}]]), s: new Set([{1}]) }})",
                spied("({ x: 1 })"),
                spied("[2]"),
                spied("new Date(0)"),
            )),
            (
                "{ o: { x: 1 }, a: [ [ 2 ] ], m: Map(1) { { x: 1 } => 1970-01-01T00:00:00.000Z }, \
                 s: Set(1) { [ 2 ] } }"
                    .to_owned(),
                0
            )
        );
        assert_eq!(
            rendered("({ o: new Proxy({ x: 1 }, {}), a: [new Proxy([2], {})] })"),
            "{ o: { x: 1 }, a: [ [ 2 ] ] }"
        );
        assert_eq!(
            rendered("new Map([[new Proxy({ k: 1 }, {}), new Proxy(new Set([1]), {})]])"),
            "Map(1) { { k: 1 } => Set(1) { 1 } }"
        );
        assert_eq!(
            rendered("new Set([new Proxy(new Map(), {})])"),
            "Set(1) { Map(0) {} }"
        );
        assert_eq!(
            rendered(
                "({ r: (() => { const r = Proxy.revocable({}, {}); r.revoke(); return r.proxy; })() })"
            ),
            "{ r: <Revoked Proxy> }"
        );
    }

    #[test]
    fn a_proxy_counts_toward_the_depth_limit_as_its_target_does() {
        assert_eq!(
            rendered("({ a: { b: { c: new Proxy({ d: 1 }, {}) } } })"),
            "{ a: { b: { c: [Object] } } }"
        );
        assert_eq!(rendered("[[[new Proxy([1], {})]]]"), "[ [ [ [Array] ] ] ]");
        assert_eq!(
            rendered("({ a: { b: { c: new Proxy(new Map(), {}) } } })"),
            "{ a: { b: { c: [Map] } } }"
        );
        assert_eq!(
            rendered("({ a: { b: { c: new Proxy(function f() {}, {}) } } })"),
            "{ a: { b: { c: [Function: f] } } }"
        );
    }

    #[test]
    fn a_proxy_prints_the_same_in_colour() {
        assert_eq!(
            colored("new Proxy(new Map([['a', 1]]), {})"),
            colored("new Map([['a', 1]])")
        );
        assert_eq!(
            colored("new Proxy({ k: 'v' }, {})"),
            format!("{{ k: {} }}", green("'v'"))
        );
        assert_eq!(
            colored("const r = Proxy.revocable({}, {}); r.revoke(); r.proxy"),
            cyan("<Revoked Proxy>")
        );
        assert_eq!(
            colored("new Proxy(function f() {}, {})"),
            cyan("[Function: f]")
        );
    }

    #[test]
    fn a_proxy_prints_through_quoted_and_escaped_alike() {
        assert_eq!(
            escaped("new Proxy({ 'k\\n': 'v\\n' }, {})", false),
            "{ 'k\\n': 'v\\n' }"
        );
    }

    #[test]
    fn an_object_inheriting_from_a_proxy_runs_no_trap() {
        assert_eq!(
            counted(&format!("Object.create({})", spied("({ a: 1 })"))),
            ("{}".to_owned(), 0)
        );
    }

    #[test]
    fn a_regexp_is_read_without_running_any_of_its_accessors() {
        for key in [
            "global",
            "ignoreCase",
            "multiline",
            "dotAll",
            "unicode",
            "unicodeSets",
            "sticky",
            "hasIndices",
            "source",
        ] {
            let setup = format!(
                "globalThis.calls = 0; globalThis.r = /a/gy; \
                 Object.defineProperty(r, '{key}', {{ get() {{ calls++; throw 1; }} }});"
            );
            let runtime = Runtime::new().unwrap();
            let context = Context::full(&runtime).unwrap();
            context.with(|ctx| {
                pin(&ctx).unwrap();
                ctx.eval::<(), _>(setup.as_str()).unwrap();
                let value: Value<'_> = ctx.eval("r").unwrap();

                assert_eq!(line(&[value], false), "/a/gy", "{key}");
                assert_eq!(ctx.eval::<i32, _>("calls").unwrap(), 0, "{key}");
            });
        }
    }

    #[test]
    fn a_regexp_lists_its_flags_in_the_standard_order() {
        assert_eq!(rendered("new RegExp('x', 'ysmigdu')"), "/x/dgimsuy");
        assert_eq!(rendered("new RegExp('x', 'yvsmigd')"), "/x/dgimsvy");
    }

    #[test]
    fn a_collection_is_not_mistaken_for_a_plain_object() {
        assert_eq!(rendered("Object.create(Map.prototype)"), "{}");
        assert_eq!(rendered("Object.create(Date.prototype)"), "{}");
        assert_eq!(rendered("Object.create(Set.prototype)"), "{}");
        assert_eq!(rendered("Object.create(RegExp.prototype)"), "{}");
    }

    #[test]
    fn the_types_are_coloured_whole_with_their_contents_inside() {
        assert_eq!(colored("new Date(0)"), magenta("1970-01-01T00:00:00.000Z"));
        assert_eq!(colored("new Date(NaN)"), magenta("Invalid Date"));
        assert_eq!(colored("/a/gi"), red("/a/gi"));
        assert_eq!(
            colored("new Map([['a', 1], ['b', { c: 2 }]])"),
            format!(
                "Map(2) {{ {} => {}, {} => {{ c: {} }} }}",
                green("'a'"),
                yellow("1"),
                green("'b'"),
                yellow("2")
            )
        );
        assert_eq!(
            colored("new Set([1, 'x'])"),
            format!("Set(2) {{ {}, {} }}", yellow("1"), green("'x'"))
        );
        assert_eq!(colored("new Map()"), "Map(0) {}");
        assert_eq!(colored("new Set()"), "Set(0) {}");
    }

    #[test]
    fn collection_placeholders_are_cyan() {
        assert_eq!(
            colored("({ a: { b: { c: new Map([[1, 2]]) } } })"),
            format!("{{ a: {{ b: {{ c: {} }} }} }}", cyan("[Map]"))
        );
        assert_eq!(
            colored("[[[new Set([1])]]]"),
            format!("[ [ [ {} ] ] ]", cyan("[Set]"))
        );
    }

    #[test]
    fn the_types_nest_coloured_inside_a_collection() {
        assert_eq!(
            colored("new Map([[new Date(0), /x/]])"),
            format!(
                "Map(1) {{ {} => {} }}",
                magenta("1970-01-01T00:00:00.000Z"),
                red("/x/")
            )
        );
    }

    #[test]
    fn the_types_print_the_same_text_with_colour_on_or_off() {
        let source = "[new Date(0), new Date(NaN), /a\\/b/gi, new Map([['k', new Set([1, 'v'])]]), new Set()]";

        assert_eq!(rendered(source), strip_codes(&colored(source)));
        assert!(!rendered(source).contains('\x1b'));
        assert_eq!(after("", source, true), colored(source));
    }

    #[test]
    fn collection_text_holding_a_line_break_is_closed_on_each_line() {
        assert_eq!(
            colored("new Set(['a\\nb'])"),
            "Set(1) { \x1b[32m'a\x1b[39m\n\x1b[32mb'\x1b[39m }"
        );
        assert_eq!(
            colored("new Map([['k\\nz', 1]])"),
            "Map(1) { \x1b[32m'k\x1b[39m\n\x1b[32mz'\x1b[39m => \x1b[33m1\x1b[39m }"
        );
    }

    #[test]
    fn escaped_collections_stay_on_one_line() {
        assert_eq!(
            escaped("new Map([['a\\nb', 'c\\td']])", false),
            "Map(1) { 'a\\nb' => 'c\\td' }"
        );
        assert_eq!(
            escaped("new Set(['x\\ny'])", true),
            "Set(1) { \x1b[32m'x\\ny'\x1b[39m }"
        );
        assert_eq!(escaped("new RegExp('\\n')", false), "/\\n/");
        assert_eq!(escaped("new Date(0)", false), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn quoted_prints_the_four_types_like_line() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            pin(&ctx).unwrap();
            let date: Value<'_> = ctx.eval("new Date(0)").unwrap();
            let map: Value<'_> = ctx.eval("new Map([['a', 1]])").unwrap();

            assert_eq!(quoted(&date, false), "1970-01-01T00:00:00.000Z");
            assert_eq!(quoted(&map, false), "Map(1) { 'a' => 1 }");
            assert_eq!(quoted(&map, true), colored("new Map([['a', 1]])"));
        });
    }

    #[test]
    fn the_first_pinning_in_a_runtime_is_kept() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            pin(&ctx).unwrap();
            ctx.eval::<(), _>("Date.prototype.toISOString = () => 'late';")
                .unwrap();
            pin(&ctx).unwrap();
            let value: Value<'_> = ctx.eval("new Date(0)").unwrap();

            assert_eq!(line(&[value], false), "1970-01-01T00:00:00.000Z");
        });
    }

    #[test]
    fn pinning_reports_a_realm_without_the_intrinsics() {
        let runtime = Runtime::new().unwrap();
        let context = Context::custom::<rquickjs::context::intrinsic::Eval>(&runtime).unwrap();
        context.with(|ctx| {
            assert!(pin(&ctx).is_err());
            let thrown = format!("{:?}", ctx.catch());
            assert!(thrown.contains("Date is not defined"), "{thrown}");
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn an_unpinned_realm_prints_the_four_types_as_objects_and_can_be_pinned_after() {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let values: Vec<Value<'_>> = ctx
                .eval("[new Date(0), /x/g, new Map([[1, 2]]), new Set([1])]")
                .unwrap();

            assert_eq!(line(&values, false), "{} {} {} {}");
            assert!(!ctx.has_exception());
            pin(&ctx).unwrap();
            assert_eq!(
                line(&values, false),
                "1970-01-01T00:00:00.000Z /x/g Map(1) { 1 => 2 } Set(1) { 1 }"
            );
        });
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
