// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

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

/// `Set(total) { 0, 1, ..., shown - 1 tail }`.
fn set_text(total: usize, shown: usize, tail: &str) -> String {
    let items: Vec<String> = (0..shown).map(|i| i.to_string()).collect();
    format!("Set({total}) {{ {}{tail} }}", items.join(", "))
}

/// `Map(total) { 0 => 0, ..., shown - 1 => shown - 1 tail }`.
fn map_text(total: usize, shown: usize, tail: &str) -> String {
    let items: Vec<String> = (0..shown).map(|i| format!("{i} => {i}")).collect();
    format!("Map({total}) {{ {}{tail} }}", items.join(", "))
}

#[test]
fn a_collection_prints_up_to_100_entries() {
    for total in [99, 100] {
        assert_eq!(
            rendered(&format!(
                "new Set(Array.from({{ length: {total} }}, (_, i) => i))"
            )),
            set_text(total, total, "")
        );
        assert_eq!(
            rendered(&format!(
                "new Map(Array.from({{ length: {total} }}, (_, i) => [i, i]))"
            )),
            map_text(total, total, "")
        );
    }
}

#[test]
fn a_collection_past_100_entries_counts_the_rest() {
    for (total, tail) in [
        (101, ", ... 1 more item"),
        (102, ", ... 2 more items"),
        (100_000, ", ... 99900 more items"),
    ] {
        assert_eq!(
            rendered(&format!(
                "new Set(Array.from({{ length: {total} }}, (_, i) => i))"
            )),
            set_text(total, 100, tail),
            "{total}"
        );
        assert_eq!(
            rendered(&format!(
                "new Map(Array.from({{ length: {total} }}, (_, i) => [i, i]))"
            )),
            map_text(total, 100, tail),
            "{total}"
        );
    }
}

#[test]
fn a_nested_collection_is_capped_on_its_own() {
    let inner = set_text(101, 100, ", ... 1 more item");
    assert_eq!(
        rendered("[new Set(Array.from({ length: 101 }, (_, i) => i)), 7]"),
        format!("[ {inner}, 7 ]")
    );
    assert_eq!(
        rendered("new Map([['k', new Set(Array.from({ length: 101 }, (_, i) => i))]])"),
        format!("Map(1) {{ 'k' => {inner} }}")
    );
}

#[test]
fn the_collection_count_is_uncolored() {
    assert!(
        colored("new Set(Array.from({ length: 101 }, (_, i) => i))")
            .ends_with(", ... 1 more item }")
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
        let setup = format!("globalThis.d = new Date(0); globalThis.bad = new Date(NaN); {tamper}");
        assert_eq!(
            after(&setup, "[d, bad]", false),
            "[ 1970-01-01T00:00:00.000Z, Invalid Date ]",
            "{tamper}"
        );
    }
}

#[test]
fn a_date_ignores_its_own_overrides() {
    let date = r"
        const d = new Date(0);
        d.toISOString = () => 'own'; d.valueOf = () => { throw 1; };
        d.getTime = () => { throw 2; }; Object.defineProperty(d, Symbol.toPrimitive, { value() { throw 3; } }); d
        ";
    assert_eq!(rendered(date), "1970-01-01T00:00:00.000Z");
}

#[test]
fn replacing_the_collection_and_regexp_machinery_changes_nothing() {
    let setup = r"
        globalThis.m = new Map([[1, 2]]); globalThis.s = new Set([1]);
        globalThis.r = /x/g; Map.prototype.forEach = () => { throw 1; };
        Map.prototype.entries = () => { throw 2; };
        Map.prototype[Symbol.iterator] = () => { throw 3; };
        Set.prototype.forEach = () => { throw 4; };
        Set.prototype.values = () => { throw 5; };
        Set.prototype[Symbol.iterator] = () => { throw 6; };
        Object.defineProperty(Map.prototype, 'size', { get() { throw 7; } });
        Object.defineProperty(Set.prototype, 'size', { get() { throw 8; } });
        Object.defineProperty(RegExp.prototype, 'source', { get() { throw 9; } });
        RegExp.prototype.toString = () => 'hijacked';
        globalThis.Map = globalThis.Set = globalThis.RegExp = undefined;
        ";
    assert_eq!(after(setup, "m", false), "Map(1) { 1 => 2 }");
    assert_eq!(after(setup, "s", false), "Set(1) { 1 }");
    assert_eq!(after(setup, "r", false), "/x/g");
}

#[test]
fn a_collections_own_overrides_are_ignored() {
    assert_eq!(
        rendered(
            r"
            const m = new Map([[1, 2]]); m.forEach = () => { throw 1; };
            m.entries = () => { throw 2; }; m[Symbol.iterator] = () => { throw 3; };
            Object.defineProperty(m, 'size', { value: 99 }); m
            "
        ),
        "Map(1) { 1 => 2 }"
    );
    assert_eq!(
        rendered(
            r"
            const s = new Set([1]); s.forEach = () => { throw 1; };
            s.values = () => { throw 2; }; s[Symbol.iterator] = () => { throw 3; }; s
            "
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

#[test]
fn the_types_are_colored_whole_with_their_contents_inside() {
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
fn the_types_nest_colored_inside_a_collection() {
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
fn the_types_print_the_same_text_with_color_on_or_off() {
    let source =
        "[new Date(0), new Date(NaN), /a\\/b/gi, new Map([['k', new Set([1, 'v'])]]), new Set()]";

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
            r"
            globalThis.calls = 0; globalThis.r = /a/gy;
            Object.defineProperty(r, '{key}', {{ get() {{ calls++; throw 1; }} }});
            "
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
    assert_eq!(rendered("Object.create(Map.prototype)"), "Map {}");
    assert_eq!(rendered("Object.create(Date.prototype)"), "Date {}");
    assert_eq!(rendered("Object.create(Set.prototype)"), "Set {}");
    assert_eq!(rendered("Object.create(RegExp.prototype)"), "RegExp {}");
}
