// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Renders a JS value as the text `console` prints.
//!
//! Modelled on Node's `util.inspect`. `JSON.stringify` throws on a cycle and
//! drops functions, `undefined` and symbols, which are the values someone
//! reaches for `console` to look at.

use std::fmt::Write;

use rquickjs::{Coerced, Ctx, Result as JsResult, Type, Value};

use crate::paint::{Style, paint};
use crate::quickjs::ValueExt;
use crate::sourcemap::remap_stack;

mod array;
mod builtins;
mod collections;
mod object;

use array::write_array;
pub(crate) use builtins::pin;
use object::write_object;

/// Nesting depth after which `[Array]` or `[Object]` is printed. It also bounds a cycle.
const MAX_DEPTH: usize = 3;

/// The depth cap of `%o`, which matches Node's depth 4.
const DETAILED_DEPTH: usize = 5;

/// Entries of one array, typed array, `Map` or `Set` printed before
/// `... N more items`. A run of array holes is one entry.
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

/// `text` in single quotes.
pub(crate) fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "\\'"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rquickjs::{Context, Runtime};
    use rstest::rstest;

    use super::*;
    use crate::tests::{colored, counted, cyan, escaped, green, rendered, with_realm, yellow};

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

    #[rstest]
    #[case::null("null", "\x1b[1mnull\x1b[22m")]
    #[case::undefined("undefined", "\x1b[90mundefined\x1b[39m")]
    fn a_nullish_value_takes_its_own_style(#[case] source: &str, #[case] want: &str) {
        assert_eq!(colored(source), want);
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

    /// A `Proxy` around `target` whose handler counts every trap lookup in
    /// `calls`.
    fn spied(target: &str) -> String {
        format!(
            "new Proxy({target}, new Proxy({{}}, {{ get() {{ calls++; return undefined; }} }}))"
        )
    }

    #[rstest]
    #[case::map("new Map([[1, 2]])", "Map(1) { 1 => 2 }")]
    #[case::set("new Set([1])", "Set(1) { 1 }")]
    #[case::date("new Date(0)", "1970-01-01T00:00:00.000Z")]
    #[case::regexp("/a/g", "/a/g")]
    #[case::array("[1, 'a']", "[ 1, 'a' ]")]
    #[case::empty_array("[]", "[]")]
    #[case::object("({ a: 1 })", "{ a: 1 }")]
    #[case::empty_object("({})", "{}")]
    #[case::function("function named() {}", "[Function: named]")]
    #[case::accessors(
        "({ get a() { calls++; }, set b(v) {} })",
        "{ a: [Getter], b: [Setter] }"
    )]
    fn a_proxy_prints_as_its_target(#[case] target: &str, #[case] shown: &str) {
        assert_eq!(counted(&spied(target)), (shown.to_owned(), 0), "{target}");
    }

    #[test]
    fn a_proxy_chain_prints_as_its_last_target() {
        assert_eq!(
            counted(&spied(&spied(&spied("new Map([[1, 2]])")))),
            ("Map(1) { 1 => 2 }".to_owned(), 0)
        );
    }

    #[rstest]
    #[case::map("const r = Proxy.revocable(new Map(), {}); r.revoke(); r.proxy")]
    #[case::object("const r = Proxy.revocable({}, {}); r.revoke(); r.proxy")]
    #[case::function("const r = Proxy.revocable(function () {}, {}); r.revoke(); r.proxy")]
    #[case::wrapped(
        "const r = Proxy.revocable({}, {}); const p = new Proxy(r.proxy, {}); r.revoke(); p"
    )]
    fn a_revoked_proxy_says_so(#[case] source: &str) {
        assert_eq!(rendered(source), "<Revoked Proxy>", "{source}");
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
                concat!(
                    "{ o: { x: 1 }, a: [ [ 2 ] ], m: Map(1) { { x: 1 } => 1970-01-01T00:00:00.000Z }, ",
                    "s: Set(1) { [ 2 ] } }"
                )
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
    fn a_proxy_prints_the_same_in_color() {
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
    fn an_object_inheriting_from_a_proxy_runs_no_trap() {
        assert_eq!(
            counted(&format!("Object.create({})", spied("({ a: 1 })"))),
            ("{}".to_owned(), 0)
        );
    }

    #[test]
    fn a_second_pinning_keeps_the_first_intrinsics() {
        with_realm(|ctx| {
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
    fn an_unpinned_realm_prints_the_four_types_as_objects() {
        with_realm(|ctx| {
            let values: Vec<Value<'_>> = ctx
                .eval("[new Date(0), /x/g, new Map([[1, 2]]), new Set([1])]")
                .unwrap();

            assert_eq!(line(&values, false), "Date {} RegExp {} Map {} Set {}");
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn values_made_before_pin_print_after_it() {
        with_realm(|ctx| {
            let values: Vec<Value<'_>> = ctx
                .eval("[new Date(0), /x/g, new Map([[1, 2]]), new Set([1])]")
                .unwrap();

            pin(&ctx).unwrap();
            assert_eq!(
                line(&values, false),
                "1970-01-01T00:00:00.000Z /x/g Map(1) { 1 => 2 } Set(1) { 1 }"
            );
        });
    }
}
