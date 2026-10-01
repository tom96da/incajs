// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `console.table`.

use std::fmt::Write;

use rquickjs::{Coerced, Type, Value};
use unicode_width::UnicodeWidthStr;

use crate::inspect;

const INDEX: &str = "(index)";
const VALUES: &str = "Values";

type Entries<'js> = Vec<(String, Value<'js>)>;

/// Draws `data` as a box-drawn table, or `None` when it is not tabular and
/// has to be logged as it is.
///
/// Only an array, or an object with at least one own enumerable key, is
/// tabular. Each own key of `data` is a row. A row that is itself tabular
/// contributes its own keys as columns, in first-seen order; any other row
/// fills the `Values` column. `columns`, when it is an array, picks the
/// columns to show and their order, first occurrence winning. Every value is
/// read once.
pub(super) fn render(data: &Value<'_>, columns: Option<&Value<'_>>) -> Option<String> {
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

    let mut header = vec![INDEX.to_owned()];
    header.extend(keys.iter().map(|key| escape(key)));
    if has_values {
        header.push(VALUES.to_owned());
    }

    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|(index, row, own)| {
            let mut cells = vec![escape(index)];
            for key in &keys {
                let cell = own
                    .as_ref()
                    .and_then(|own| own.iter().find(|(k, _)| k == key))
                    .map(|(_, value)| cell(value));
                cells.push(cell.unwrap_or_default());
            }
            if has_values {
                cells.push(if own.is_none() {
                    cell(row)
                } else {
                    String::new()
                });
            }
            cells
        })
        .collect();

    Some(draw(&header, &body))
}

/// The own keys and values of an array, or of an object that has any;
/// `None` for anything else.
fn entries<'js>(value: &Value<'js>) -> Option<Entries<'js>> {
    if !matches!(value.type_of(), Type::Array | Type::Object) {
        return None;
    }
    let object = value.as_object()?;
    let entries: Entries<'js> = object
        .keys::<String>()
        .flatten()
        .filter_map(|key| {
            let item = object.get::<_, Value<'_>>(key.as_str()).ok()?;
            Some((key, item))
        })
        .collect();
    (value.is_array() || !entries.is_empty()).then_some(entries)
}

/// One cell's text on a single line.
fn cell(value: &Value<'_>) -> String {
    match value.as_string().and_then(|text| text.to_string().ok()) {
        Some(text) => format!("'{}'", escape(&text).replace('\'', "\\'")),
        None => escape_controls(&inspect::quoted(value)),
    }
}

/// Spells a backslash and every control character as an escape sequence, so
/// a cell stays on one line.
fn escape(text: &str) -> String {
    escape_controls(&text.replace('\\', "\\\\"))
}

fn escape_controls(text: &str) -> String {
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

/// The elements of an array, each as a string.
fn strings(value: &Value<'_>) -> Option<Vec<String>> {
    let array = value.as_array()?;
    Some(
        array
            .iter::<Coerced<String>>()
            .flatten()
            .map(|item| item.0)
            .collect(),
    )
}

fn draw(header: &[String], body: &[Vec<String>]) -> String {
    let widths: Vec<usize> = (0..header.len())
        .map(|column| {
            body.iter()
                .map(|row| row[column].width())
                .chain([header[column].width()])
                .max()
                .unwrap_or(0)
                + 2
        })
        .collect();

    let rule = |left: &str, mid: &str, right: &str| {
        let bars: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
        format!("{left}{}{right}", bars.join(mid))
    };
    let row = |cells: &[String]| {
        let padded: Vec<String> = cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!(" {cell}{}", " ".repeat(width - 1 - cell.width())))
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
            render(&data, columns.as_ref())
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
    fn nested_values_in_cells_are_inspected() {
        let drawn = table("[{a: [1, 2], b: {c: 1}}]", None).unwrap();

        assert!(drawn.contains("[ 1, 2 ]"), "{drawn}");
        assert!(drawn.contains("{ c: 1 }"), "{drawn}");
    }
}
