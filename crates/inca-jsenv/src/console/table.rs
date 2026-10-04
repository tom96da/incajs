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
/// are coloured when `color` is set.
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
        let plain = format!("'{}'", escape(&text).replace('\'', "\\'"));
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
#[allow(clippy::unwrap_used)]
mod tests {
    use rquickjs::{Context, Runtime};

    use super::*;

    /// Renders `data` (and `columns`, when given) as `console.table` would.
    fn table(data: &str, columns: Option<&str>) -> Option<String> {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
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
            "┌─────────┬───┬─────┐\n\
             │ (index) │ a │ b   │\n\
             ├─────────┼───┼─────┤\n\
             │ 0       │ 1 │ 'x' │\n\
             │ 1       │ 2 │     │\n\
             └─────────┴───┴─────┘"
        );
    }

    #[test]
    fn an_array_of_arrays_numbers_its_columns() {
        assert_eq!(
            table("[[1, 2], [3, 4]]", None).unwrap(),
            "┌─────────┬───┬───┐\n\
             │ (index) │ 0 │ 1 │\n\
             ├─────────┼───┼───┤\n\
             │ 0       │ 1 │ 2 │\n\
             │ 1       │ 3 │ 4 │\n\
             └─────────┴───┴───┘"
        );
    }

    #[test]
    fn an_object_of_objects_indexes_rows_by_key() {
        assert_eq!(
            table("({x: {n: 1}, y: {n: 2}})", None).unwrap(),
            "┌─────────┬───┐\n\
             │ (index) │ n │\n\
             ├─────────┼───┤\n\
             │ x       │ 1 │\n\
             │ y       │ 2 │\n\
             └─────────┴───┘"
        );
    }

    #[test]
    fn primitive_rows_fill_a_values_column() {
        assert_eq!(
            table("['a', 2]", None).unwrap(),
            "┌─────────┬────────┐\n\
             │ (index) │ Values │\n\
             ├─────────┼────────┤\n\
             │ 0       │ 'a'    │\n\
             │ 1       │ 2      │\n\
             └─────────┴────────┘"
        );
    }

    #[test]
    fn mixed_rows_use_both_the_key_and_values_columns() {
        assert_eq!(
            table("[{a: 1}, 5]", None).unwrap(),
            "┌─────────┬───┬────────┐\n\
             │ (index) │ a │ Values │\n\
             ├─────────┼───┼────────┤\n\
             │ 0       │ 1 │        │\n\
             │ 1       │   │ 5      │\n\
             └─────────┴───┴────────┘"
        );
    }

    #[test]
    fn the_columns_argument_picks_and_orders_columns() {
        assert_eq!(
            table("[{a: 1, b: 2, c: 3}]", Some("['c', 'a', 'zz']")).unwrap(),
            "┌─────────┬───┬───┬────┐\n\
             │ (index) │ c │ a │ zz │\n\
             ├─────────┼───┼───┼────┤\n\
             │ 0       │ 3 │ 1 │    │\n\
             └─────────┴───┴───┴────┘"
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
            "┌─────────┐\n\
             │ (index) │\n\
             ├─────────┤\n\
             └─────────┘"
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
            "┌─────────┬────────┐\n\
             │ (index) │ k      │\n\
             ├─────────┼────────┤\n\
             │ 0       │ '日本' │\n\
             │ 1       │ 'ab'   │\n\
             └─────────┴────────┘"
        );
    }

    #[test]
    fn a_throwing_property_is_left_out_and_leaves_nothing_pending() {
        assert_eq!(
            table("[{ get a() { throw 1; }, b: 2 }]", None).unwrap(),
            "┌─────────┬───┐\n\
             │ (index) │ b │\n\
             ├─────────┼───┤\n\
             │ 0       │ 2 │\n\
             └─────────┴───┘"
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
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
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
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
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
}
