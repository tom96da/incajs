// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders a JS value as the text `console` prints.
//!
//! Modelled on Node's `util.inspect`. `JSON.stringify` throws on a cycle and
//! drops functions, `undefined` and symbols, which are the values someone
//! reaches for `console` to look at.

use std::fmt::Write;

use rquickjs::{Coerced, Ctx, Object, Result as JsResult, Type, Value, object::Filter};

use crate::paint::{Style, paint};
use crate::quickjs::{ObjectExt, ValueExt};
use crate::sourcemap::remap_stack;

mod builtins;

pub(crate) use builtins::pin;

/// Nesting depth after which `[Array]` or `[Object]` is printed. It also bounds a cycle.
const MAX_DEPTH: usize = 3;

/// The depth cap of `%o`, which matches Node's depth 4.
const DETAILED_DEPTH: usize = 5;

/// Entries of one array, `Map` or `Set` printed before `... N more items`. A
/// run of array holes is one entry.
const MAX_ENTRIES: usize = 100;

/// The value of a read, or `None` after clearing the exception it threw.
pub(crate) fn settled<T>(ctx: &Ctx<'_>, result: JsResult<T>) -> Option<T> {
    if result.is_err() {
        drop(ctx.catch());
    }
    result.ok()
}

pub(crate) fn line(values: &[Value<'_>], color: bool) -> String {
    let mode = Mode::new(color, false);
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
    render_quoted(value, Mode::new(color, false))
}

/// As [`quoted`], with the depth cap of `%o` and the non-enumerable own
/// properties shown in square brackets.
pub(crate) fn detailed(value: &Value<'_>, color: bool) -> String {
    render_quoted(
        value,
        Mode {
            max_depth: DETAILED_DEPTH,
            hidden: true,
            ..Mode::new(color, false)
        },
    )
}

/// As [`quoted`], with every control character in the value's text spelled
/// as an escape sequence, so the result is a single line.
pub(crate) fn quoted_escaped(value: &Value<'_>, color: bool) -> String {
    render_quoted(value, Mode::new(color, true))
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
    /// Nesting depth after which `[Array]` or `[Object]` is printed.
    max_depth: usize,
    /// Whether non-enumerable own string keys are printed as `[key]: value`.
    hidden: bool,
}

impl Mode {
    const fn new(color: bool, escape: bool) -> Self {
        Self {
            color,
            escape,
            max_depth: MAX_DEPTH,
            hidden: false,
        }
    }
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

/// The value itself, or for a `Proxy` the target it finally forwards to
/// (`None` when a link of the chain is revoked). A proxy's target exists
/// before the proxy does, so a chain cannot loop.
pub(crate) fn unproxied<'js>(value: &Value<'js>) -> Option<Value<'js>> {
    let mut current = value.clone();
    loop {
        if !current.is_proxy() {
            return Some(current);
        }
        let target = settled(value.ctx(), current.proxy_target())?;
        current = target.into_value();
    }
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

/// A symbol has no string coercion (`String(sym)` throws), so this reads the
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
        let stack = remap_stack(ctx, stack.trim_end());
        for frame in stack.split('\n') {
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
    if depth >= mode.max_depth {
        put(out, mode, Style::Cyan, "[Array]");
        return;
    }
    let object = array.as_object();
    let ctx = object.ctx();
    // A length above `i32::MAX` is a float, so `Array::len` cannot read it.
    let length = settled(ctx, object.get::<_, f64>("length")).unwrap_or(0.0);
    if length < 1.0 && !mode.hidden {
        out.push_str("[]");
        return;
    }

    out.push_str("[ ");
    let mut written = false;
    let mut entries = 0;
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
            entries += 1;
            next = index;
            if entries == MAX_ENTRIES {
                break;
            }
        }
        separate(out, &mut written);
        write_slot(out, slot(object, &index.to_string()), depth, mode);
        entries += 1;
        next = index + 1.0;
        if entries == MAX_ENTRIES {
            break;
        }
    }
    let rest = length - next;
    if rest > 0.0 {
        if entries == MAX_ENTRIES {
            write_more(out, &mut written, rest, mode);
        } else {
            write_holes(out, &mut written, rest, mode);
        }
    }
    if mode.hidden {
        separate(out, &mut written);
        out.push_str("[length]: ");
        put(out, mode, Style::Yellow, &length.to_string());
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
    put(
        out,
        mode,
        Style::Grey,
        &format!("<{}>", items(count, "empty")),
    );
}

/// `... N more items`, closing an array cut at [`MAX_ENTRIES`].
fn write_more(out: &mut String, written: &mut bool, count: f64, mode: Mode) {
    separate(out, written);
    plain(out, mode, &format!("... {}", items(count, "more")));
}

/// `N <kind> item` or `N <kind> items`.
#[allow(clippy::float_cmp)] // JS lengths arrive as f64 and the counts are whole numbers
fn items(count: f64, kind: &str) -> String {
    let plural = if count == 1.0 { "" } else { "s" };
    format!("{count} {kind} item{plural}")
}

/// `text` in single quotes.
pub(crate) fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "\\'"))
}

/// A key that is an identifier prints as it is; any other is quoted.
fn write_key(out: &mut String, key: &str, bracketed: bool, mode: Mode) {
    if bracketed {
        out.push('[');
    }
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
    out.push_str(if bracketed { "]: " } else { ": " });
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
    let Some(Some(desc)) = settled(object.ctx(), object.get_own_property_descriptor(key)) else {
        return Slot::Missing;
    };
    match (
        desc.is_accessor(),
        desc.getter.is_undefined(),
        desc.setter.is_undefined(),
    ) {
        (true, false, false) => Slot::Accessor("[Getter/Setter]"),
        (true, false, true) => Slot::Accessor("[Getter]"),
        (true, true, false) => Slot::Accessor("[Setter]"),
        // A data property, or an accessor with neither function, whose value
        // is `undefined`.
        _ => Slot::Value(desc.value),
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
    // The label of a prototype-less object is a tag in brackets.
    let (label, tag) = match constructor_name(object) {
        None if is_module_namespace(object) => ("Module: null prototype".to_owned(), true),
        None => ("Object: null prototype".to_owned(), true),
        Some(name) => (name, false),
    };
    if depth >= mode.max_depth {
        put(out, mode, Style::Cyan, &format!("[{label}]"));
        return;
    }
    let prefix = match (tag, label.as_str()) {
        (true, _) => format!("[{label}] "),
        (false, "Object") => String::new(),
        _ => format!("{label} "),
    };

    let mut entries = 0;
    let mut body = String::new();
    let keys = if mode.hidden {
        object.own_keys::<String>(Filter::new().string())
    } else {
        object.keys::<String>()
    };
    for key in keys {
        let Ok(key) = key else {
            drop(object.ctx().catch());
            continue;
        };
        let shown = mode.hidden
            && settled(object.ctx(), object.get_own_property_descriptor(&key[..]))
                .flatten()
                .is_some_and(|desc| !desc.is_enumerable());
        if entries > 0 {
            body.push_str(", ");
        }
        write_key(&mut body, &key, shown, mode);
        write_slot(&mut body, slot(object, &key), depth, mode);
        entries += 1;
    }

    if entries == 0 {
        let _ = write!(out, "{prefix}{{}}");
    } else {
        let _ = write!(out, "{prefix}{{ {body} }}");
    }
}

/// The name of the first constructor on the prototype chain that the object
/// is an instance of. `None` for an object with no prototype. When no
/// constructor qualifies, `Object <name of the prototype>`. Only own data
/// properties are read. A constructor that is a `Proxy` reads as its target,
/// and a `Proxy` on the chain reads as `Object`, so no getter or trap runs.
fn constructor_name(object: &Object<'_>) -> Option<String> {
    let first = prototype_of(object)?;
    let mut current = first.clone();
    loop {
        if current.as_value().is_proxy() {
            return Some("Object".to_owned());
        }
        let own = settled(
            object.ctx(),
            current.get_own_property_descriptor("constructor"),
        )
        .flatten();
        if let Some(constructor) = own
            .and_then(|desc| unproxied(&desc.value))
            .and_then(Value::into_object)
            && constructor.is_function()
            && let Some(name) = own_string(&constructor, "name").filter(|name| !name.is_empty())
            && is_instance(object, &constructor)
        {
            return Some(name);
        }
        match prototype_of(&current) {
            Some(next) => current = next,
            None => break,
        }
    }
    let inner = constructor_name(&first).unwrap_or_else(|| detached(&first));
    Some(format!("Object <{inner}>"))
}

/// A prototype-less object as a prototype label: `[Object: null prototype]`,
/// then `{}` when it has no enumerable keys.
fn detached(object: &Object<'_>) -> String {
    if object.keys::<String>().next().is_none() {
        "[Object: null prototype] {}".to_owned()
    } else {
        "[Object: null prototype]".to_owned()
    }
}

/// The string held by an own data property, or `None`.
fn own_string(object: &Object<'_>, key: &str) -> Option<String> {
    let desc = settled(object.ctx(), object.get_own_property_descriptor(key)).flatten()?;
    desc.value.as_string()?.to_string().ok()
}

/// Whether the object is a module namespace, which has no prototype and the
/// tag `Module`.
fn is_module_namespace(object: &Object<'_>) -> bool {
    let ctx = object.ctx();
    let Ok(symbol) = ctx.globals().get::<_, Object<'_>>("Symbol") else {
        drop(ctx.catch());
        return false;
    };
    let Some(Some(tag)) = settled(ctx, symbol.get_own_property_descriptor("toStringTag")) else {
        return false;
    };
    object.get_prototype().is_none()
        && settled(ctx, object.get_own_property_descriptor(tag.value))
            .flatten()
            .and_then(|desc| desc.value.as_string()?.to_string().ok())
            .is_some_and(|tag| tag == "Module")
}

/// The prototype, clearing the exception a `Proxy` prototype's trap threw.
fn prototype_of<'js>(object: &Object<'js>) -> Option<Object<'js>> {
    let prototype = object.get_prototype();
    if object.ctx().has_exception() {
        drop(object.ctx().catch());
    }
    prototype
}

/// Whether the data property `constructor.prototype` is on the object's chain.
fn is_instance<'js>(object: &Object<'js>, constructor: &Object<'js>) -> bool {
    let ctx = object.ctx();
    let Some(target) = settled(ctx, constructor.get_own_property_descriptor("prototype"))
        .flatten()
        .and_then(|desc| desc.value.into_object())
    else {
        return false;
    };
    let mut current = prototype_of(object);
    while let Some(next) = current {
        if next == target {
            return true;
        }
        if next.as_value().is_proxy() {
            return false;
        }
        current = prototype_of(&next);
    }
    false
}

#[cfg(test)]
mod tests;
