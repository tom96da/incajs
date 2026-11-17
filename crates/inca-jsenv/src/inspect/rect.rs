// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `DOMRect` and `DOMRectReadOnly` values.
//!
//! The text is read from the state of the instance, so printing runs no script code, whatever a
//! script defined on the prototypes.

use rquickjs::Value;

use super::object::{constructor_name, write_key};
use super::{Mode, put, write_value};
use crate::geometry::printed;
use crate::paint::Style;

/// Appends the constructor name, or the interface name when there is none, and the eight values of
/// a rect. Returns false for any other value.
pub(super) fn write(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) -> bool {
    let Some((name, values)) = printed(value) else {
        return false;
    };
    let label = value
        .as_object()
        .and_then(constructor_name)
        .unwrap_or_else(|| name.to_owned());
    if depth >= mode.max_depth {
        put(out, mode, Style::Cyan, &format!("[{label}]"));
        return true;
    }
    out.push_str(&label);
    out.push_str(" { ");
    for (index, (key, number)) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write_key(out, key, false, mode);
        write_value(
            out,
            &Value::new_number(value.ctx().clone(), *number),
            depth + 1,
            mode,
        );
    }
    out.push_str(" }");
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{written_by, yellow};

    /// The lines `source` logs in a realm that has the rect classes installed.
    fn lines(source: &str, color: bool) -> Vec<String> {
        written_by(color, |ctx| {
            crate::vault::create_vault(ctx).unwrap();
            crate::geometry::install(ctx).unwrap();
            ctx.eval::<(), _>(source).unwrap();
        })
        .into_iter()
        .map(|(_, text)| text)
        .collect()
    }

    fn first(source: &str) -> String {
        lines(source, false).remove(0)
    }

    #[rstest]
    #[case::writable(
        "new DOMRect(1, 2, 3, 4)",
        "DOMRect { x: 1, y: 2, width: 3, height: 4, top: 2, right: 4, bottom: 6, left: 1 }"
    )]
    #[case::read_only(
        "new DOMRectReadOnly(1, 2, 3, 4)",
        "DOMRectReadOnly { x: 1, y: 2, width: 3, height: 4, top: 2, right: 4, bottom: 6, left: 1 }"
    )]
    #[case::subclass(
        "new (class R extends DOMRect {})(1, 2, 3, 4)",
        "R { x: 1, y: 2, width: 3, height: 4, top: 2, right: 4, bottom: 6, left: 1 }"
    )]
    #[case::read_only_subclass(
        "new (class Q extends DOMRectReadOnly {})(1, 2, 3, 4)",
        "Q { x: 1, y: 2, width: 3, height: 4, top: 2, right: 4, bottom: 6, left: 1 }"
    )]
    #[case::undefined_constructor(
        "new (class R extends DOMRect { static { R.prototype.constructor = undefined; } })(1, 2, 3, 4)",
        "DOMRect { x: 1, y: 2, width: 3, height: 4, top: 2, right: 4, bottom: 6, left: 1 }"
    )]
    #[case::null_prototype(
        "Object.setPrototypeOf(new DOMRect(1, 2, 3, 4), null)",
        "DOMRect { x: 1, y: 2, width: 3, height: 4, top: 2, right: 4, bottom: 6, left: 1 }"
    )]
    #[case::negative_size(
        "new DOMRect(0, 0, -10, 5)",
        "DOMRect { x: 0, y: 0, width: -10, height: 5, top: 0, right: 0, bottom: 5, left: -10 }"
    )]
    fn a_rect_prints_its_eight_values(#[case] source: &str, #[case] expected: &str) {
        assert_eq!(first(&format!("console.log({source})")), expected);
    }

    #[test]
    fn a_redefined_getter_changes_nothing_printed() {
        assert_eq!(
            first(
                r"
                Object.defineProperty(DOMRect.prototype, 'x', {
                    get() {
                        throw new Error('called');
                    },
                });
                console.log(new DOMRect(1, 2, 3, 4));
                "
            ),
            "DOMRect { x: 1, y: 2, width: 3, height: 4, top: 2, right: 4, bottom: 6, left: 1 }"
        );
    }

    #[test]
    fn a_rect_past_the_depth_limit_prints_its_name() {
        assert_eq!(
            first("console.log({ a: { b: { c: new DOMRect() } } })"),
            "{ a: { b: { c: [DOMRect] } } }"
        );
    }

    #[test]
    fn a_subclass_past_the_depth_limit_prints_its_name() {
        assert_eq!(
            first("console.log({ a: { b: { c: new (class R extends DOMRect {})() } } })"),
            "{ a: { b: { c: [R] } } }"
        );
    }

    #[test]
    fn the_numbers_are_yellow() {
        assert!(lines("console.log(new DOMRect(1, 2, 3, 4))", true)[0].contains(&yellow("3")));
    }
}
