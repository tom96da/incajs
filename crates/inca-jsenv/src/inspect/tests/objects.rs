// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use crate::{Engine, compile_async_function};

#[test]
fn a_namespace_with_an_uninitialized_export_prints_and_leaves_nothing_pending() {
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
fn an_instance_prints_with_its_class_name() {
    let cases = [
        (
            "class Foo { constructor() { this.a = 1; } }; new Foo()",
            "Foo { a: 1 }",
        ),
        ("class Foo {}; new Foo()", "Foo {}"),
        (
            "class A {}; class B extends A { a = 1 }; new B()",
            "B { a: 1 }",
        ),
        ("function F() { this.a = 1; }; new F()", "F { a: 1 }"),
        (
            "const f = new (class Foo { a = 1 })(); delete f.constructor; f",
            "Foo { a: 1 }",
        ),
        ("class Foo {}; ({ x: [new Foo()] })", "{ x: [ Foo {} ] }"),
        (
            "class Foo { a = 1 }; ({ x: { y: { z: new Foo() } } })",
            "{ x: { y: { z: [Foo] } } }",
        ),
        ("Object.create({ a: 1 })", "{}"),
        (
            "class Foo { a = 1 }; Object.setPrototypeOf(Foo.prototype, null); new Foo()",
            "Foo { a: 1 }",
        ),
        (
            r"
            class Foo { a = 1 };
            Foo.prototype.constructor = new Proxy(Foo, { get() { throw 1; } }); new Foo()
            ",
            "Foo { a: 1 }",
        ),
        ("({ a: 1 })", "{ a: 1 }"),
    ];
    for (expression, expected) in cases {
        assert_eq!(rendered(expression), expected, "{expression}");
    }
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

#[test]
fn a_constructor_that_cannot_name_the_object_falls_back_to_object() {
    let cases = [
        // Anonymous class.
        ("new (class { a = 1 })()", "{ a: 1 }"),
        // Foo.prototype has no Foo.prototype on its chain.
        ("class Foo {}; Foo.prototype", "{}"),
        (
            "const o = { a: 1 }; o.constructor = 5; o",
            "{ a: 1, constructor: 5 }",
        ),
        (
            "class Foo { a = 1 }; delete Foo.prototype.constructor; new Foo()",
            "{ a: 1 }",
        ),
        (
            "class Foo { a = 1 }; Foo.prototype.constructor = 5; new Foo()",
            "{ a: 1 }",
        ),
        (
            r"
            class Foo { a = 1 }; Object.defineProperty(Foo.prototype, 'constructor',
            { get() { throw 1; } }); new Foo()
            ",
            "{ a: 1 }",
        ),
        (
            "class Foo { a = 1; static get name() { throw 1; } }; new Foo()",
            "{ a: 1 }",
        ),
        ("Object.create(new Proxy({}, {}))", "{}"),
        (
            r"
            class Foo { a = 1 };
            Object.defineProperty(Foo, 'name', { get() { throw 1; } }); new Foo()
            ",
            "{ a: 1 }",
        ),
        (
            "class Foo { a = 1 }; Foo.prototype.constructor = (function () {}).bind(); new Foo()",
            "{ a: 1 }",
        ),
    ];
    for (expression, expected) in cases {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let value: Value<'_> = ctx.eval(expression).unwrap();
            assert_eq!(line(&[value], false), expected, "{expression}");
            assert!(!ctx.has_exception(), "{expression} left an exception");
        });
    }
}

#[test]
fn an_object_whose_prototypes_name_no_constructor_names_its_prototype() {
    let cases = [
        (
            "Object.create(Object.create(null))",
            "Object <[Object: null prototype] {}> {}",
        ),
        (
            "Object.assign(Object.create(Object.create(null)), { a: 1 })",
            "Object <[Object: null prototype] {}> { a: 1 }",
        ),
        (
            "Object.create(Object.create(null, { x: { value: 1, enumerable: true } }))",
            "Object <[Object: null prototype]> {}",
        ),
        (
            "Object.create(Object.create(Object.create(null)))",
            "Object <Object <[Object: null prototype] {}>> {}",
        ),
    ];
    for (expression, expected) in cases {
        assert_eq!(rendered(expression), expected, "{expression}");
    }
}

#[test]
fn a_module_namespace_is_tagged_as_a_module() {
    let runtime = Runtime::new().unwrap();
    let context = Context::full(&runtime).unwrap();
    context.with(|ctx| {
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
