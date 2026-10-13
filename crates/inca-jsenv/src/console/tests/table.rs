// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn dir_quotes_a_string_and_ignores_options_and_extras() {
    assert_eq!(
        texts(
            "console.dir('s'); console.dir({ a: 'b' }, { depth: 0 }); console.dir(1, 2, 3); console.dir();"
        ),
        ["'s'", "{ a: 'b' }", "1", "undefined"]
    );
}

#[test]
fn table_draws_through_the_console() {
    assert_eq!(
        texts("console.table([{ a: 1 }]);"),
        ["┌─────────┬───┐\n│ (index) │ a │\n├─────────┼───┤\n│ 0       │ 1 │\n└─────────┴───┘"]
    );
}

#[test]
fn table_takes_a_columns_filter() {
    let drawn = &texts("console.table([{ a: 1, b: 2 }], ['b']);")[0];

    assert!(drawn.contains("│ b │"), "{drawn}");
    assert!(!drawn.contains("│ a │"), "{drawn}");
}

#[test]
fn table_falls_back_to_log_for_a_non_object() {
    assert_eq!(
        texts("console.table(5); console.table('s'); console.table(null); console.table();"),
        ["5", "s", "null", "undefined"]
    );
}

#[test]
fn table_writes_at_log_level_behind_the_guides() {
    let written = lines_written_by("console.group('g'); console.table([]);");

    assert_eq!(written[1].0, Level::Log);
    assert_eq!(
        written[1].1,
        "│ ┌─────────┐\n│ │ (index) │\n│ ├─────────┤\n│ └─────────┘"
    );
}

#[test]
fn table_escapes_control_characters_and_backslashes_in_cells() {
    let drawn = &texts(r"console.table([{ 'k\n': 'a\nb\tc\\d\x01', e: '\u0085' }]);")[0];

    assert_eq!(drawn.lines().count(), 5, "{drawn}");
    assert!(drawn.contains(r"│ k\n "), "{drawn}");
    assert!(drawn.contains(r"'a\nb\tc\\d\x01'"), "{drawn}");
    assert!(drawn.contains(r"'\x85'"), "{drawn}");
}

#[test]
fn table_treats_date_map_set_and_error_as_not_tabular() {
    assert_eq!(
        texts(
            r"
            console.table(new Date(0)); console.table(new Map([[1, 2]]));
            console.table(new Set([1])); console.table(new Error('x')); console.table({});
            "
        )
        .iter()
        .map(|t| t.lines().next().unwrap().to_owned())
        .collect::<Vec<_>>(),
        [
            "1970-01-01T00:00:00.000Z",
            "Map(1) { 1 => 2 }",
            "Set(1) { 1 }",
            "Error: x",
            "{}"
        ]
    );
}

#[test]
fn table_puts_non_tabular_rows_in_the_values_column() {
    let drawn = &texts("console.table([new Date(0), new Error('x')]);")[0];

    assert!(drawn.contains("Values"), "{drawn}");
    assert!(
        drawn.contains("│ 0       │ 1970-01-01T00:00:00.000Z "),
        "{drawn}"
    );
    assert!(drawn.contains("Error: x"), "{drawn}");
    assert!(drawn.contains(r"Error: x\n    at "), "{drawn}");
    assert_eq!(drawn.lines().count(), 6, "{drawn}");
}

#[test]
fn table_shows_date_regexp_map_and_set_rows_as_one_line_values() {
    let drawn = &texts(
        "console.table([new Date(0), new Date(NaN), /a\\nb/g, new Map([['k', { v: 'x' }]]), new Set([1, 'y'])]);",
    )[0];

    assert_eq!(
        drawn,
        concat!(
            "┌─────────┬──────────────────────────────┐\n",
            "│ (index) │ Values                       │\n",
            "├─────────┼──────────────────────────────┤\n",
            "│ 0       │ 1970-01-01T00:00:00.000Z     │\n",
            "│ 1       │ Invalid Date                 │\n",
            "│ 2       │ /a\\nb/g                      │\n",
            "│ 3       │ Map(1) { 'k' => { v: 'x' } } │\n",
            "│ 4       │ Set(2) { 1, 'y' }            │\n",
            "└─────────┴──────────────────────────────┘",
        )
    );
}

#[test]
fn table_cells_hold_the_four_types_inspected() {
    let drawn =
        &texts("console.table([{ d: new Date(0), m: new Map([[1, 2]]), s: new Set() }]);")[0];

    assert!(drawn.contains("│ 1970-01-01T00:00:00.000Z │"), "{drawn}");
    assert!(drawn.contains("│ Map(1) { 1 => 2 } │"), "{drawn}");
    assert!(drawn.contains("│ Set(0) {} │"), "{drawn}");
}

#[test]
fn table_cells_escape_control_characters_inside_a_collection() {
    let drawn = &texts("console.table([{ m: new Map([['a\\nb', 'c\\td']]) }]);")[0];

    assert_eq!(drawn.lines().count(), 5, "{drawn}");
    assert!(drawn.contains(r"Map(1) { 'a\nb' => 'c\td' }"), "{drawn}");
}

#[test]
fn dir_dirxml_and_the_specifiers_print_the_four_types() {
    assert_eq!(
        texts(
            r"
            console.dir(new Map([['a', new Set(['b'])]])); console.dirxml(new Date(0));
            console.log('%o|%O', /x/g, new Set([1]));
            console.log(new Map([[1, 2]]), new Date(NaN));
            "
        ),
        [
            "Map(1) { 'a' => Set(1) { 'b' } }",
            "1970-01-01T00:00:00.000Z",
            "/x/g|Set(1) { 1 }",
            "Map(1) { 1 => 2 } Invalid Date",
        ]
    );
}

#[test]
fn an_error_property_holding_the_types_prints_them() {
    assert_eq!(
        texts("console.log({ e: Object.assign(new Error('x'), { when: new Date(0) }), m: new Map() });")[0]
            .lines()
            .next()
            .unwrap(),
        "{ e: Error: x"
    );
    assert_eq!(
        texts("const e = new Error('x'); e.stack = ''; console.log([e, new Set([1])]);"),
        ["[ Error: x, Set(1) { 1 } ]"]
    );
}

#[test]
fn hostile_values_print_without_leaving_an_exception() {
    assert_eq!(
        texts(
            r"
            const hostile = new Date(0); hostile.valueOf = () => { throw 1; };
            hostile.toISOString = () => { throw 2; }; console.log(hostile);
            const m = new Map([[1, 2]]); m.entries = () => { throw 3; };
            m[Symbol.iterator] = () => { throw 4; }; console.log(m);
            console.log(new Proxy(new Map(), {}));
            const r = Proxy.revocable(new Set(), {}); r.revoke(); console.log(r.proxy);
            console.log(new (class S extends Set {})([1]));
            const c = new Map(); c.set('self', c); console.log(c);
            console.log(new Set(Array.from({ length: 100000 }, (_, i) => i)).size);
            try { null.x; } catch (e) { console.log('caught'); }
            "
        ),
        [
            "1970-01-01T00:00:00.000Z",
            "Map(1) { 1 => 2 }",
            "Map(0) {}",
            "<Revoked Proxy>",
            "Set(1) { 1 }",
            "Map(1) { 'self' => Map(1) { 'self' => Map(1) { 'self' => [Map] } } }",
            "100000",
            "caught"
        ]
    );
}

#[test]
fn a_colored_table_of_the_types_has_the_layout_of_the_plain_one() {
    drawn_both_ways("[new Date(0), /x/g, new Map([['日本', [1]]]), new Set(['a\\nb'])]");
}

#[test]
fn color_off_prints_the_types_without_an_escape_sequence() {
    let source = r"
        console.log(new Date(0), new Date(NaN), /a/g, new Map([[1, 'x']]), new Set());
        console.dir(new Map()); console.table([new Date(0)]);
        ";

    assert!(texts(source).iter().all(|t| !t.contains('\x1b')));
    assert_eq!(
        colored_texts(source)
            .iter()
            .map(|t| without_known_codes(t))
            .collect::<Vec<_>>(),
        texts(source)
    );
}

#[test]
fn table_reads_each_getter_once() {
    let lines = texts(
        r"
        console.table([{ get a() { console.count('g'); return 1; } },
        { get a() { console.count('h'); return 2; } }], ['a', 'a']);
        ",
    );

    assert_eq!(lines[..2], ["g: 1", "h: 1"]);
    assert_eq!(lines.len(), 3);
}

#[test]
fn table_ignores_a_getter_that_changes_the_group_depth() {
    let lines = texts("console.table([{ get a() { console.group('x'); return 1; } }]);");

    assert_eq!(lines[0], "▼ x");
    assert!(lines[1].starts_with("│ ┌"), "{}", lines[1]);
}

#[test]
fn table_dedupes_repeated_columns() {
    assert_eq!(
        texts("console.table([{ a: 1 }], ['a', 'a']);"),
        texts("console.table([{ a: 1 }], ['a']);")
    );
}

#[test]
fn a_colored_table_has_the_layout_of_the_plain_one() {
    let source =
        "console.group('g'); console.table([{ a: 1, b: 'x' }, { a: [1, 'y'], c: null }, 5, 'z']);";
    let plain = texts(source);
    let colored = colored_texts(source);

    assert_eq!(colored.len(), plain.len());
    assert_eq!(strip(&colored[1]), plain[1]);
    assert!(colored[1].contains("\x1b[33m1\x1b[39m"), "{}", colored[1]);
    assert!(colored[1].contains("\x1b[32m'x'\x1b[39m"), "{}", colored[1]);
}

#[test]
fn a_colored_table_cell_is_padded_by_its_visible_width() {
    let drawn = &colored_texts("console.table([{ k: 'ab' }, { k: 1 }]);")[0];

    assert_eq!(
        drawn.lines().nth(3).unwrap(),
        "│ 0       │ \x1b[32m'ab'\x1b[39m │"
    );
    assert_eq!(
        drawn.lines().nth(4).unwrap(),
        "│ 1       │ \x1b[33m1\x1b[39m    │"
    );
}

#[test]
fn a_colored_table_leaves_headers_and_rules_uncolored() {
    let drawn = &colored_texts("console.table([{ a: 1 }]);")[0];
    let lines: Vec<&str> = drawn.lines().collect();

    assert_eq!(lines[0], "┌─────────┬───┐");
    assert_eq!(lines[1], "│ (index) │ a │");
    assert_eq!(lines[2], "├─────────┼───┤");
}

#[test]
fn a_colored_table_escapes_control_characters_like_the_plain_one() {
    let source = r"console.table([{ k: ['a\nb', 1] }]);";

    assert_eq!(strip(&colored_texts(source)[0]), texts(source)[0]);
}

#[test]
fn a_table_cell_holding_an_escape_sequence_shows_it_as_text() {
    let (plain, colored) = drawn_both_ways(r"[{ k: ['\x1b[1mx'] }]");

    assert!(plain.contains(r"[ '\x1b[1mx' ]"), "{plain}");
    assert!(colored.contains(r"\x1b[1mx"), "{colored}");
}

#[test]
fn a_table_cell_holding_a_nested_color_code_stays_inert() {
    let (plain, _) = drawn_both_ways(r"[{ k: { v: '\x1b[31m' }, 'z\x1b[0m': 1 }]");

    assert!(plain.contains(r"'\x1b[31m'"), "{plain}");
    assert!(plain.contains(r"z\x1b[0m"), "{plain}");
}

#[test]
fn a_table_cell_with_a_tab_or_newline_stays_on_one_line() {
    let (plain, _) = drawn_both_ways(r"[{ k: ['a\tb', 'c\nd'], 'e\tf': 1 }, 'x\ty']");

    assert_eq!(plain.lines().count(), 6, "{plain}");
    assert!(plain.contains(r"'a\tb'"), "{plain}");
    assert!(plain.contains(r"'c\nd'"), "{plain}");
    assert!(plain.contains(r"e\tf"), "{plain}");
}

#[test]
fn a_table_with_a_wide_and_a_colored_cell_stays_aligned() {
    drawn_both_ways(r"[{ a: '日本', b: [1, 'x'], c: null, d: undefined, e: () => 1 }]");
}
