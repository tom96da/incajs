// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `console.table`.

use rquickjs::{Coerced, Type, Value};
use unicode_width::UnicodeWidthStr;

use crate::inspect;
use crate::paint::{Style, paint};

const INDEX: &str = "(index)";
const VALUES: &str = "Values";

type Entries<'js> = Vec<(String, Value<'js>)>;

/// `plain` sets widths; `shown` is printed.
#[derive(Default)]
struct Cell {
    plain: String,
    shown: String,
}

impl Cell {
    fn plain(text: String) -> Self {
        Self {
            shown: text.clone(),
            plain: text,
        }
    }
}

/// Draws `data` as a table, or `None` when it is not tabular.
///
/// An array, or an object with at least one own enumerable key, is tabular.
/// `columns`, when an array, picks and orders the columns shown. Cell values
/// are colored when `color` is set.
pub(super) fn render(data: &Value<'_>, columns: Option<&Value<'_>>, color: bool) -> Option<String> {
    let rows: Vec<(String, Value<'_>, Option<Entries<'_>>)> = entries(data)?
        .into_iter()
        .map(|(index, row)| {
            let own = entries(&row);
            (index, row, own)
        })
        .collect();

    let mut keys: Vec<String> = Vec::new();
    for key in rows.iter().filter_map(|(_, _, own)| own.as_ref()).flatten() {
        if !keys.contains(&key.0) {
            keys.push(key.0.clone());
        }
    }
    if let Some(picked) = columns.and_then(strings) {
        keys.clear();
        for key in picked {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    let has_values = rows.iter().any(|(_, _, own)| own.is_none());

    let mut header = vec![Cell::plain(INDEX.to_owned())];
    header.extend(keys.iter().map(|key| Cell::plain(escape(key))));
    if has_values {
        header.push(Cell::plain(VALUES.to_owned()));
    }

    let body: Vec<Vec<Cell>> = rows
        .iter()
        .map(|(index, row, own)| {
            let mut cells = vec![Cell::plain(escape(index))];
            for key in &keys {
                let cell = own
                    .as_ref()
                    .and_then(|own| own.iter().find(|(k, _)| k == key))
                    .map(|(_, value)| cell(value, color));
                cells.push(cell.unwrap_or_default());
            }
            if has_values {
                cells.push(if own.is_none() {
                    cell(row, color)
                } else {
                    Cell::default()
                });
            }
            cells
        })
        .collect();

    Some(draw(&header, &body))
}

fn entries<'js>(value: &Value<'js>) -> Option<Entries<'js>> {
    let value = &inspect::unproxied(value)?;
    if !matches!(value.type_of(), Type::Array | Type::Object) {
        return None;
    }
    let object = value.as_object()?;
    let ctx = object.ctx();
    let entries: Entries<'js> = object
        .keys::<String>()
        .filter_map(|key| {
            let key = inspect::settled(ctx, key)?;
            let item = inspect::settled(ctx, object.get::<_, Value<'_>>(key.as_str()))?;
            Some((key, item))
        })
        .collect();
    (value.is_array() || !entries.is_empty()).then_some(entries)
}

fn cell(value: &Value<'_>, color: bool) -> Cell {
    if let Some(text) = value.as_string().and_then(|text| text.to_string().ok()) {
        let plain = inspect::quote(&escape(&text));
        return Cell {
            shown: paint(color, Style::Green, &plain),
            plain,
        };
    }
    let plain = inspect::quoted_escaped(value, false);
    let shown = if color {
        inspect::quoted_escaped(value, true)
    } else {
        plain.clone()
    };
    Cell { plain, shown }
}

/// Spells a backslash and every control character as an escape sequence, so
/// a cell stays on one line.
fn escape(text: &str) -> String {
    inspect::escape_controls(&text.replace('\\', "\\\\"))
}

fn strings(value: &Value<'_>) -> Option<Vec<String>> {
    let value = &inspect::unproxied(value)?;
    let array = value.as_array()?;
    let ctx = array.ctx();
    Some(
        array
            .iter::<Coerced<String>>()
            .filter_map(|item| inspect::settled(ctx, item))
            .map(|item| item.0)
            .collect(),
    )
}

fn draw(header: &[Cell], body: &[Vec<Cell>]) -> String {
    let widths: Vec<usize> = (0..header.len())
        .map(|column| {
            body.iter()
                .map(|row| row[column].plain.width())
                .chain([header[column].plain.width()])
                .max()
                .unwrap_or(0)
                + 2
        })
        .collect();

    let rule = |left: &str, mid: &str, right: &str| {
        let bars: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
        format!("{left}{}{right}", bars.join(mid))
    };
    let row = |cells: &[Cell]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| {
                format!(
                    " {}{}",
                    cell.shown,
                    " ".repeat(width - 1 - cell.plain.width())
                )
            })
            .collect();
        format!("│{}│", padded.join("│"))
    };

    let mut lines = vec![rule("┌", "┬", "┐"), row(header), rule("├", "┼", "┤")];
    lines.extend(body.iter().map(|cells| row(cells)));
    lines.push(rule("└", "┴", "┘"));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::console::Level;
    use crate::tests::{colored_texts, drawn_both_ways, lines_written_by, texts, with_realm};

    /// Renders `data` (and `columns`, when given) as `console.table` would.
    fn table(data: &str, columns: Option<&str>) -> Option<String> {
        with_realm(|ctx| {
            let data: Value<'_> = ctx.eval(data).unwrap();
            let columns: Option<Value<'_>> = columns.map(|c| ctx.eval(c).unwrap());
            let drawn = render(&data, columns.as_ref(), false);
            assert!(!ctx.has_exception(), "a read left an exception");
            drawn
        })
    }

    #[test]
    fn an_array_of_objects_gets_a_column_per_key() {
        assert_eq!(
            table("[{a: 1, b: 'x'}, {a: 2}]", None).unwrap(),
            concat!(
                "┌─────────┬───┬─────┐\n",
                "│ (index) │ a │ b   │\n",
                "├─────────┼───┼─────┤\n",
                "│ 0       │ 1 │ 'x' │\n",
                "│ 1       │ 2 │     │\n",
                "└─────────┴───┴─────┘",
            )
        );
    }

    #[test]
    fn an_array_of_arrays_numbers_its_columns() {
        assert_eq!(
            table("[[1, 2], [3, 4]]", None).unwrap(),
            concat!(
                "┌─────────┬───┬───┐\n",
                "│ (index) │ 0 │ 1 │\n",
                "├─────────┼───┼───┤\n",
                "│ 0       │ 1 │ 2 │\n",
                "│ 1       │ 3 │ 4 │\n",
                "└─────────┴───┴───┘",
            )
        );
    }

    #[test]
    fn an_object_of_objects_indexes_rows_by_key() {
        assert_eq!(
            table("({x: {n: 1}, y: {n: 2}})", None).unwrap(),
            concat!(
                "┌─────────┬───┐\n",
                "│ (index) │ n │\n",
                "├─────────┼───┤\n",
                "│ x       │ 1 │\n",
                "│ y       │ 2 │\n",
                "└─────────┴───┘",
            )
        );
    }

    #[test]
    fn primitive_rows_fill_a_values_column() {
        assert_eq!(
            table("['a', 2]", None).unwrap(),
            concat!(
                "┌─────────┬────────┐\n",
                "│ (index) │ Values │\n",
                "├─────────┼────────┤\n",
                "│ 0       │ 'a'    │\n",
                "│ 1       │ 2      │\n",
                "└─────────┴────────┘",
            )
        );
    }

    #[test]
    fn mixed_rows_use_both_the_key_and_values_columns() {
        assert_eq!(
            table("[{a: 1}, 5]", None).unwrap(),
            concat!(
                "┌─────────┬───┬────────┐\n",
                "│ (index) │ a │ Values │\n",
                "├─────────┼───┼────────┤\n",
                "│ 0       │ 1 │        │\n",
                "│ 1       │   │ 5      │\n",
                "└─────────┴───┴────────┘",
            )
        );
    }

    #[test]
    fn the_columns_argument_picks_and_orders_columns() {
        assert_eq!(
            table("[{a: 1, b: 2, c: 3}]", Some("['c', 'a', 'zz']")).unwrap(),
            concat!(
                "┌─────────┬───┬───┬────┐\n",
                "│ (index) │ c │ a │ zz │\n",
                "├─────────┼───┼───┼────┤\n",
                "│ 0       │ 3 │ 1 │    │\n",
                "└─────────┴───┴───┴────┘",
            )
        );
    }

    #[test]
    fn a_columns_argument_that_is_not_an_array_is_ignored() {
        assert_eq!(table("[{a: 1}]", Some("'a'")), table("[{a: 1}]", None),);
    }

    #[test]
    fn an_empty_array_draws_only_the_index_header() {
        assert_eq!(
            table("[]", None).unwrap(),
            concat!(
                "┌─────────┐\n",
                "│ (index) │\n",
                "├─────────┤\n",
                "└─────────┘",
            )
        );
    }

    #[test]
    fn a_non_object_is_not_tabular() {
        assert_eq!(table("1", None), None);
        assert_eq!(table("'text'", None), None);
        assert_eq!(table("null", None), None);
        assert_eq!(table("undefined", None), None);
        assert_eq!(table("(() => {})", None), None);
    }

    #[test]
    fn wide_characters_are_measured_by_display_width() {
        assert_eq!(
            table("[{k: '日本'}, {k: 'ab'}]", None).unwrap(),
            concat!(
                "┌─────────┬────────┐\n",
                "│ (index) │ k      │\n",
                "├─────────┼────────┤\n",
                "│ 0       │ '日本' │\n",
                "│ 1       │ 'ab'   │\n",
                "└─────────┴────────┘",
            )
        );
    }

    #[test]
    fn a_throwing_property_is_left_out_and_leaves_nothing_pending() {
        assert_eq!(
            table("[{ get a() { throw 1; }, b: 2 }]", None).unwrap(),
            concat!(
                "┌─────────┬───┐\n",
                "│ (index) │ b │\n",
                "├─────────┼───┤\n",
                "│ 0       │ 2 │\n",
                "└─────────┴───┘",
            )
        );
    }

    #[test]
    fn a_throwing_row_is_left_out_and_leaves_nothing_pending() {
        let drawn = table(
            "Object.defineProperty([{ a: 1 }, { a: 2 }], 0, { get() { throw 1; } })",
            None,
        )
        .unwrap();

        assert!(drawn.contains("│ 1       │ 2 │"), "{drawn}");
        assert!(!drawn.contains("│ 0 "), "{drawn}");
    }

    #[test]
    fn a_proxy_is_drawn_as_its_target() {
        let plain = table("[{ a: 1 }, [2]]", None).unwrap();

        assert_eq!(
            table("[new Proxy({ a: 1 }, {}), new Proxy([2], {})]", None).unwrap(),
            plain
        );
        assert_eq!(
            table("new Proxy([{ a: 1 }, [2]], {})", None).unwrap(),
            plain
        );
    }

    #[test]
    fn a_proxy_row_calls_no_trap() {
        with_realm(|ctx| {
            ctx.eval::<(), _>(
                "globalThis.calls = 0; globalThis.spy = (t) => new Proxy(t, new Proxy({}, { get() { calls++; } }));",
            )
            .unwrap();
            let data: Value<'_> = ctx.eval("spy([spy({ a: 1 }), spy({}), spy([2])])").unwrap();

            assert!(render(&data, None, false).is_some());
            assert_eq!(ctx.eval::<i32, _>("calls").unwrap(), 0);
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn a_proxy_of_an_array_picks_the_columns() {
        let plain = table("[{ a: 1, b: 2 }]", Some("['b']")).unwrap();

        assert_eq!(
            table("[{ a: 1, b: 2 }]", Some("new Proxy(['b'], {})")).unwrap(),
            plain
        );
        assert!(!plain.contains(" a "), "{plain}");
    }

    #[test]
    fn a_revoked_proxy_is_not_tabular() {
        with_realm(|ctx| {
            let data: Value<'_> = ctx
                .eval("const r = Proxy.revocable([], {}); r.revoke(); r.proxy")
                .unwrap();

            assert!(render(&data, None, false).is_none());
            assert!(!ctx.has_exception());
        });
    }

    #[test]
    fn a_column_that_cannot_be_stringified_is_skipped() {
        let drawn = table("[{ a: 1 }]", Some("['a', { toString() { throw 1; } }]")).unwrap();

        assert!(drawn.contains("│ a │"), "{drawn}");
        assert_eq!(drawn.lines().count(), 5, "{drawn}");
    }

    #[test]
    fn nested_values_in_cells_are_inspected() {
        let drawn = table("[{a: [1, 2], b: {c: 1}}]", None).unwrap();

        assert!(drawn.contains("[ 1, 2 ]"), "{drawn}");
        assert!(drawn.contains("{ c: 1 }"), "{drawn}");
    }

    #[test]
    fn dir_quotes_a_string() {
        assert_eq!(texts("console.dir('s');"), ["'s'"]);
    }

    #[test]
    fn dir_prints_only_its_first_argument() {
        assert_eq!(
            texts("console.dir({ a: 'b' }, { depth: 0 }); console.dir(1, 2, 3); console.dir();"),
            ["{ a: 'b' }", "1", "undefined"]
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
    fn table_escapes_a_single_quote_in_a_string_cell() {
        let drawn = &texts(r#"console.table([{ a: "it's" }]);"#)[0];

        assert!(drawn.contains(r"'it\'s'"), "{drawn}");
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
    fn a_colored_table_of_the_types_has_the_layout_of_the_plain_one() {
        drawn_both_ways("[new Date(0), /x/g, new Map([['日本', [1]]]), new Set(['a\\nb'])]");
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
    fn a_table_cell_holding_an_escape_sequence_shows_it_as_text() {
        let (plain, colored) = drawn_both_ways(r"[{ k: ['\x1b[1mx'] }]");

        assert!(plain.contains(r"[ '\x1b[1mx' ]"), "{plain}");
        assert!(colored.contains(r"\x1b[1mx"), "{colored}");
    }

    #[test]
    fn a_table_with_a_wide_and_a_colored_cell_stays_aligned() {
        drawn_both_ways(r"[{ a: '日本', b: [1, 'x'], c: null, d: undefined, e: () => 1 }]");
    }
}
