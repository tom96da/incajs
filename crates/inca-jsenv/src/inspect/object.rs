// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Plain objects: keys, own properties, the constructor name and promise state.

use std::fmt::Write;

use rquickjs::promise::PromiseState;
use rquickjs::{Object, Promise, Value, object::Filter};

use super::{Mode, plain, put, quote, settled, unproxied, write_value};
use super::{builtins, url};
use crate::paint::Style;
use crate::quickjs::ObjectExt;

/// A key that is an identifier prints as it is; any other is quoted.
pub(super) fn write_key(out: &mut String, key: &str, bracketed: bool, mode: Mode) {
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
pub(super) enum Slot<'js> {
    Value(Value<'js>),
    /// `[Getter]`, `[Setter]` or `[Getter/Setter]`.
    Accessor(&'static str),
    /// No such property, or a read that threw (the exception is cleared).
    Missing,
}

/// Reads an own property's descriptor, so an accessor is named and never
/// called.
pub(super) fn slot<'js>(object: &Object<'js>, key: &str) -> Slot<'js> {
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
pub(super) fn write_slot(out: &mut String, slot: Slot<'_>, depth: usize, mode: Mode) {
    match slot {
        Slot::Value(item) => write_value(out, &item, depth + 1, mode),
        Slot::Accessor(text) => put(out, mode, Style::Cyan, text),
        Slot::Missing => put(out, mode, Style::Grey, "undefined"),
    }
}

pub(super) fn write_object(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) {
    let Some(object) = value.as_object() else {
        put(out, mode, Style::Cyan, "[Object]");
        return;
    };
    if builtins::write(out, object, depth, mode) || url::write(out, value, depth, mode) {
        return;
    }
    let promise = value.as_promise();
    // The label of a prototype-less object is a tag in brackets.
    let (label, tag) = match (constructor_name(object), promise.is_some()) {
        (None, true) => ("Promise: null prototype".to_owned(), true),
        (None, false) if is_module_namespace(object) => ("Module: null prototype".to_owned(), true),
        (None, false) => ("Object: null prototype".to_owned(), true),
        (Some(name), true) if name != "Promise" => (format!("{name} [Promise]"), false),
        (Some(name), _) => (name, false),
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
    if let Some(promise) = promise {
        write_promise_state(&mut body, promise, depth, mode);
        entries += 1;
    }
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

/// Appends the state of `promise`: `<pending>`, its value, or `<rejected>` and its reason.
pub(super) fn write_promise_state(
    out: &mut String,
    promise: &Promise<'_>,
    depth: usize,
    mode: Mode,
) {
    match promise.state() {
        PromiseState::Pending => put(out, mode, Style::Cyan, "<pending>"),
        PromiseState::Resolved => {
            for value in promise.result::<Value>().into_iter().flatten() {
                write_value(out, &value, depth + 1, mode);
            }
        }
        PromiseState::Rejected => {
            put(out, mode, Style::Cyan, "<rejected>");
            out.push(' ');
            // `result` throws the reason, so `catch` takes it back.
            drop(promise.result::<Value>());
            write_value(out, &promise.ctx().catch(), depth + 1, mode);
        }
    }
}

/// The name of the first constructor on the prototype chain that the object
/// is an instance of. `None` for an object with no prototype. When no
/// constructor qualifies, `Object <name of the prototype>`. Only own data
/// properties are read. A constructor that is a `Proxy` reads as its target,
/// and a `Proxy` on the chain reads as `Object`, so no getter or trap runs.
pub(super) fn constructor_name(object: &Object<'_>) -> Option<String> {
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
pub(super) fn detached(object: &Object<'_>) -> String {
    if object.keys::<String>().next().is_none() {
        "[Object: null prototype] {}".to_owned()
    } else {
        "[Object: null prototype]".to_owned()
    }
}

/// The string held by an own data property, or `None`.
pub(super) fn own_string(object: &Object<'_>, key: &str) -> Option<String> {
    let desc = settled(object.ctx(), object.get_own_property_descriptor(key)).flatten()?;
    desc.value.as_string()?.to_string().ok()
}

/// Whether the object is a module namespace, which has no prototype and the
/// tag `Module`.
pub(super) fn is_module_namespace(object: &Object<'_>) -> bool {
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
pub(super) fn prototype_of<'js>(object: &Object<'js>) -> Option<Object<'js>> {
    let prototype = object.get_prototype();
    if object.ctx().has_exception() {
        drop(object.ctx().catch());
    }
    prototype
}

/// Whether the data property `constructor.prototype` is on the object's chain.
pub(super) fn is_instance<'js>(object: &Object<'js>, constructor: &Object<'js>) -> bool {
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
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::inspect::{line, pin};
    use crate::tests::{colored, counted, cyan, rendered, with_realm, yellow};
    use crate::{Engine, compile_async_function};

    #[test]
    fn a_namespace_with_an_uninitialized_export_prints_and_leaves_nothing_pending() {
        with_realm(|ctx| {
            pin(&ctx).unwrap();
            // The module throws before `x` is initialized, so reading `x` throws.
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

    #[rstest]
    #[case::constructor_assigns_field(
        "class Foo { constructor() { this.a = 1; } }; new Foo()",
        "Foo { a: 1 }"
    )]
    #[case::empty_class("class Foo {}; new Foo()", "Foo {}")]
    #[case::subclass("class A {}; class B extends A { a = 1 }; new B()", "B { a: 1 }")]
    #[case::function_constructor("function F() { this.a = 1; }; new F()", "F { a: 1 }")]
    #[case::constructor_deleted(
        "const f = new (class Foo { a = 1 })(); delete f.constructor; f",
        "Foo { a: 1 }"
    )]
    #[case::nested_in_array("class Foo {}; ({ x: [new Foo()] })", "{ x: [ Foo {} ] }")]
    #[case::past_nesting_depth(
        "class Foo { a = 1 }; ({ x: { y: { z: new Foo() } } })",
        "{ x: { y: { z: [Foo] } } }"
    )]
    #[case::plain_prototype("Object.create({ a: 1 })", "{}")]
    #[case::prototype_cut_off(
        "class Foo { a = 1 }; Object.setPrototypeOf(Foo.prototype, null); new Foo()",
        "Foo { a: 1 }"
    )]
    #[case::constructor_proxy_with_throwing_get(
        r"
        class Foo { a = 1 };
        Foo.prototype.constructor = new Proxy(Foo, { get() { throw 1; } }); new Foo()
        ",
        "Foo { a: 1 }"
    )]
    #[case::plain_object("({ a: 1 })", "{ a: 1 }")]
    fn an_instance_prints_with_its_class_name(#[case] expression: &str, #[case] expected: &str) {
        assert_eq!(rendered(expression), expected, "{expression}");
    }

    #[test]
    fn a_prototype_less_object_prints_a_tag() {
        assert_eq!(
            rendered("Object.assign(Object.create(null), { a: 1 })"),
            "[Object: null prototype] { a: 1 }"
        );
        assert_eq!(
            rendered("Object.create(null)"),
            "[Object: null prototype] {}"
        );
        assert_eq!(
            rendered("({ x: { y: { z: Object.assign(Object.create(null), { a: 1 }) } } })"),
            "{ x: { y: { z: [Object: null prototype] } } }"
        );
    }

    #[rstest]
    #[case::anonymous_class("new (class { a = 1 })()", "{ a: 1 }")]
    // Foo.prototype has no Foo.prototype on its chain.
    #[case::prototype_itself("class Foo {}; Foo.prototype", "{}")]
    #[case::constructor_is_a_number(
        "const o = { a: 1 }; o.constructor = 5; o",
        "{ a: 1, constructor: 5 }"
    )]
    #[case::constructor_deleted(
        "class Foo { a = 1 }; delete Foo.prototype.constructor; new Foo()",
        "{ a: 1 }"
    )]
    #[case::prototype_constructor_is_a_number(
        "class Foo { a = 1 }; Foo.prototype.constructor = 5; new Foo()",
        "{ a: 1 }"
    )]
    #[case::constructor_getter_throws(
        r"
        class Foo { a = 1 }; Object.defineProperty(Foo.prototype, 'constructor',
        { get() { throw 1; } }); new Foo()
        ",
        "{ a: 1 }"
    )]
    #[case::static_name_getter_throws(
        "class Foo { a = 1; static get name() { throw 1; } }; new Foo()",
        "{ a: 1 }"
    )]
    #[case::proxy_prototype("Object.create(new Proxy({}, {}))", "{}")]
    #[case::name_defined_as_throwing_getter(
        r"
        class Foo { a = 1 };
        Object.defineProperty(Foo, 'name', { get() { throw 1; } }); new Foo()
        ",
        "{ a: 1 }"
    )]
    #[case::bound_function_constructor(
        "class Foo { a = 1 }; Foo.prototype.constructor = (function () {}).bind(); new Foo()",
        "{ a: 1 }"
    )]
    fn a_constructor_that_cannot_name_the_object_falls_back_to_object(
        #[case] expression: &str,
        #[case] expected: &str,
    ) {
        with_realm(|ctx| {
            let value: Value<'_> = ctx.eval(expression).unwrap();
            assert_eq!(line(&[value], false), expected, "{expression}");
            assert!(!ctx.has_exception(), "{expression} left an exception");
        });
    }

    #[rstest]
    #[case::empty(
        "Object.create(Object.create(null))",
        "Object <[Object: null prototype] {}> {}"
    )]
    #[case::own_property(
        "Object.assign(Object.create(Object.create(null)), { a: 1 })",
        "Object <[Object: null prototype] {}> { a: 1 }"
    )]
    #[case::prototype_with_property(
        "Object.create(Object.create(null, { x: { value: 1, enumerable: true } }))",
        "Object <[Object: null prototype]> {}"
    )]
    #[case::two_levels(
        "Object.create(Object.create(Object.create(null)))",
        "Object <Object <[Object: null prototype] {}>> {}"
    )]
    fn an_object_whose_prototypes_name_no_constructor_names_its_prototype(
        #[case] expression: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(rendered(expression), expected, "{expression}");
    }

    #[test]
    fn a_module_namespace_is_tagged_as_a_module() {
        with_realm(|ctx| {
            drop(rquickjs::Module::evaluate(
                ctx.clone(),
                "n",
                "import * as self from 'n'; globalThis.ns = self; export let x = 1;",
            ));
            let value: Value<'_> = ctx.eval("ns").unwrap();
            assert_eq!(line(&[value], false), "[Module: null prototype] { x: 1 }");
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
    fn an_error_prints_its_frames_at_their_source() {
        let engine = Engine::new().unwrap();
        let body = crate::sourcemap::mapped_body("throw new Error('bad edit');");
        engine.with(|ctx| {
            let f = compile_async_function(&ctx, &[], &body, "virtual.js").unwrap();
            ctx.globals().set("f", f).unwrap();
        });
        engine
            .eval::<()>("f().catch((e) => { globalThis.caught = e; });")
            .unwrap();
        assert!(engine.run_jobs().is_empty());

        let out = engine.with(|ctx| {
            let value: Value<'_> = ctx.globals().get("caught").unwrap();
            line(&[value], false)
        });
        assert!(out.contains("src/app.ts:12:5"), "{out}");
        assert!(!out.contains("virtual.js:"), "{out}");
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

    #[rstest]
    #[case::getter("({ get a() { calls++; return 1; } })", "{ a: [Getter] }")]
    #[case::setter("({ set a(v) { calls++; } })", "{ a: [Setter] }")]
    #[case::getter_and_setter(
        "({ get a() { calls++; }, set a(v) { calls++; } })",
        "{ a: [Getter/Setter] }"
    )]
    #[case::defined_getter(
        "Object.defineProperty({}, 'a', { get() { calls++; }, enumerable: true })",
        "{ a: [Getter] }"
    )]
    #[case::undefined_accessors(
        "Object.defineProperty({}, 'a', { set: undefined, get: undefined, enumerable: true })",
        "{ a: undefined }"
    )]
    fn an_accessor_is_named_by_its_kind_and_never_called(
        #[case] source: &str,
        #[case] shown: &str,
    ) {
        assert_eq!(counted(source), (shown.to_owned(), 0), "{source}");
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
                r"
                const a = [1, 2, 3];
                Object.defineProperty(a, 1, { get() { calls++; }, enumerable: true });
                Object.defineProperty(a, 2, { set(v) { calls++; }, enumerable: true }); a
                "
            ),
            ("[ 1, [Getter], [Setter] ]".to_owned(), 0)
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

    #[rstest]
    #[case::resolved("Promise.resolve(1)", "Promise { 1 }")]
    #[case::pending("new Promise(() => {})", "Promise { <pending> }")]
    #[case::rejected(
        r"
        (() => {
            const p = Promise.reject(1);
            p.catch(() => {});
            return p;
        })()
        ",
        "Promise { <rejected> 1 }"
    )]
    #[case::nested_string("Promise.resolve('a')", "Promise { 'a' }")]
    #[case::own_property("Object.assign(Promise.resolve(1), { x: 2 })", "Promise { 1, x: 2 }")]
    #[case::subclass(
        r"
        (() => {
            class P extends Promise {}
            return P.resolve(1);
        })()
        ",
        "P [Promise] { 1 }"
    )]
    #[case::null_prototype(
        "Object.setPrototypeOf(Promise.resolve(1), null)",
        "[Promise: null prototype] { 1 }"
    )]
    #[case::past_the_depth("[[[Promise.resolve(1)]]]", "[ [ [ [Promise] ] ] ]")]
    #[case::prototype_only("Object.create(Promise.prototype)", "Promise {}")]
    fn a_promise_prints_its_state(#[case] expression: &str, #[case] want: &str) {
        assert_eq!(rendered(expression), want);
    }

    #[rstest]
    #[case::pending("new Promise(() => {})", format!("Promise {{ {} }}", cyan("<pending>")))]
    #[case::rejected(
        r"
        (() => {
            const p = Promise.reject(1);
            p.catch(() => {});
            return p;
        })()
        ",
        format!("Promise {{ {} {} }}", cyan("<rejected>"), yellow("1"))
    )]
    fn a_promise_state_marker_is_cyan(#[case] expression: &str, #[case] want: String) {
        assert_eq!(colored(expression), want);
    }
}
