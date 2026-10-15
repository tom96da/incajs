// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// A `Proxy` around `target` whose handler counts every trap lookup in
/// `calls`.
fn spied(target: &str) -> String {
    format!("new Proxy({target}, new Proxy({{}}, {{ get() {{ calls++; return undefined; }} }}))")
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

        assert_eq!(line(&values, false), "Date {} RegExp {} Map {} Set {}");
        assert!(!ctx.has_exception());
        pin(&ctx).unwrap();
        assert_eq!(
            line(&values, false),
            "1970-01-01T00:00:00.000Z /x/g Map(1) { 1 => 2 } Set(1) { 1 }"
        );
    });
}
