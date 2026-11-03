// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Arrays: indexed entries, holes, and the cut after [`MAX_ENTRIES`].

use rquickjs::{Value, object::Filter};

use super::object::{slot, write_slot};
use super::{MAX_ENTRIES, Mode, plain, put, settled};
use crate::paint::Style;

pub(super) fn write_array(out: &mut String, value: &Value<'_>, depth: usize, mode: Mode) {
    let Some(array) = value.as_array() else {
        put(out, mode, Style::Cyan, "[Array]");
        return;
    };
    if depth >= mode.max_depth {
        put(out, mode, Style::Cyan, "[Array]");
        return;
    }
    let object = array.as_object();
    let ctx = object.ctx();
    // A length above `i32::MAX` is a float, so `Array::len` cannot read it.
    let length = settled(ctx, object.get::<_, f64>("length")).unwrap_or(0.0);
    if length < 1.0 && !mode.hidden {
        out.push_str("[]");
        return;
    }

    out.push_str("[ ");
    let mut written = false;
    let mut entries = 0;
    let mut next = 0.0;
    // Only the present indices are visited, in ascending order, so a hole run
    // of any size costs one step.
    for key in object.own_keys::<String>(Filter::new().string()) {
        let Some(index) = settled(ctx, key).as_deref().and_then(array_index) else {
            continue;
        };
        if index >= length {
            continue;
        }
        if index > next {
            write_holes(out, &mut written, index - next, mode);
            entries += 1;
            next = index;
            if entries == MAX_ENTRIES {
                break;
            }
        }
        separate(out, &mut written);
        write_slot(out, slot(object, &index.to_string()), depth, mode);
        entries += 1;
        next = index + 1.0;
        if entries == MAX_ENTRIES {
            break;
        }
    }
    let rest = length - next;
    if rest > 0.0 {
        if entries == MAX_ENTRIES {
            write_more(out, &mut written, rest, mode);
        } else {
            write_holes(out, &mut written, rest, mode);
        }
    }
    if mode.hidden {
        separate(out, &mut written);
        out.push_str("[length]: ");
        put(out, mode, Style::Yellow, &length.to_string());
    }
    out.push_str(" ]");
}

/// The index an array key names, or `None` for any other key.
pub(super) fn array_index(key: &str) -> Option<f64> {
    let index: u32 = key.parse().ok()?;
    (index != u32::MAX && index.to_string() == key).then_some(f64::from(index))
}

/// Starts the next entry of an array.
pub(super) fn separate(out: &mut String, written: &mut bool) {
    if *written {
        out.push_str(", ");
    }
    *written = true;
}

pub(super) fn write_holes(out: &mut String, written: &mut bool, count: f64, mode: Mode) {
    separate(out, written);
    put(
        out,
        mode,
        Style::Grey,
        &format!("<{}>", items(count, "empty")),
    );
}

/// `... N more items`, closing an array cut at [`MAX_ENTRIES`].
pub(super) fn write_more(out: &mut String, written: &mut bool, count: f64, mode: Mode) {
    separate(out, written);
    plain(out, mode, &format!("... {}", items(count, "more")));
}

/// `N <kind> item` or `N <kind> items`.
#[allow(clippy::float_cmp)] // JS lengths arrive as f64 and the counts are whole numbers
pub(super) fn items(count: f64, kind: &str) -> String {
    let plural = if count == 1.0 { "" } else { "s" };
    format!("{count} {kind} item{plural}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{colored, counted, escaped, green, grey, rendered, yellow};

    #[rstest]
    #[case::middle("[1, , 3]", "[ 1, <1 empty item>, 3 ]")]
    #[case::leading("[, 1]", "[ <1 empty item>, 1 ]")]
    #[case::trailing("[1, , ]", "[ 1, <1 empty item> ]")]
    #[case::several_runs("[1, , , 4, , 6]", "[ 1, <2 empty items>, 4, <1 empty item>, 6 ]")]
    #[case::only_holes("new Array(3)", "[ <3 empty items> ]")]
    #[case::around_undefined(
        "[undefined, , undefined]",
        "[ undefined, <1 empty item>, undefined ]"
    )]
    fn array_holes_print_as_counted_empty_items(#[case] source: &str, #[case] shown: &str) {
        assert_eq!(rendered(source), shown, "{source}");
    }

    #[test]
    fn array_holes_print_grey_when_colored() {
        assert_eq!(
            colored("[1, , 3]"),
            format!(
                "[ {}, {}, {} ]",
                yellow("1"),
                grey("<1 empty item>"),
                yellow("3")
            )
        );
    }

    /// `[ 0, 1, ..., count - 1 ]` as `console` prints it, `tail` after the last.
    fn numbers(count: usize, tail: &str) -> String {
        let items: Vec<String> = (0..count).map(|i| i.to_string()).collect();
        format!("[ {}{tail} ]", items.join(", "))
    }

    #[rstest]
    #[case::below_cap(99)]
    #[case::at_cap(100)]
    fn an_array_prints_up_to_100_entries(#[case] length: usize) {
        assert_eq!(
            rendered(&format!("Array.from({{ length: {length} }}, (_, i) => i)")),
            numbers(length, "")
        );
    }

    #[rstest]
    #[case::one_more(101, ", ... 1 more item")]
    #[case::two_more(102, ", ... 2 more items")]
    #[case::many_more(1000, ", ... 900 more items")]
    fn an_array_past_100_entries_counts_the_rest(#[case] length: usize, #[case] tail: &str) {
        assert_eq!(
            rendered(&format!("Array.from({{ length: {length} }}, (_, i) => i)")),
            numbers(100, tail),
            "{length}"
        );
    }

    #[test]
    fn the_array_cap_leaves_the_text_of_each_entry_alone() {
        assert_eq!(
            colored("Array.from({ length: 101 }, () => 'a')")
                .matches(&green("'a'"))
                .count(),
            100
        );
        assert!(
            colored("Array.from({ length: 101 }, () => 1)").ends_with(", ... 1 more item ]"),
            "the count is uncolored"
        );
        assert_eq!(
            escaped("Array.from({ length: 101 }, () => 1)", true),
            colored("Array.from({ length: 101 }, () => 1)")
        );
    }

    #[test]
    fn the_cap_applies_to_a_nested_array_on_its_own() {
        let inner = numbers(100, ", ... 1 more item");
        assert_eq!(
            rendered("[Array.from({ length: 101 }, (_, i) => i), 7]"),
            format!("[ {inner}, 7 ]")
        );
        assert_eq!(
            rendered("({ a: Array.from({ length: 101 }, (_, i) => i) })"),
            format!("{{ a: {inner} }}")
        );
    }

    #[test]
    fn a_hole_run_is_one_entry_toward_the_cap() {
        let dense = |count: usize| {
            let items: Vec<String> = (0..count).map(|i| i.to_string()).collect();
            items.join(", ") + ", "
        };
        // 99 items, then a hole run and one item: 101 entries.
        let source = "const a = Array.from({ length: 99 }, (_, i) => i); a[100] = 'x'; a";
        assert_eq!(
            rendered(source),
            format!("[ {}<1 empty item>, ... 1 more item ]", dense(99))
        );
        // 98 items, a hole run and one item: exactly 100 entries.
        let source = "const a = Array.from({ length: 98 }, (_, i) => i); a[99] = 'x'; a";
        assert_eq!(
            rendered(source),
            format!("[ {}<1 empty item>, 'x' ]", dense(98))
        );
        // 100 items, then a trailing run of holes.
        let source = "const a = Array.from({ length: 100 }, (_, i) => i); a.length = 105; a";
        assert_eq!(rendered(source), numbers(100, ", ... 5 more items"));
        // 99 items, then a trailing run of holes: the run is the 100th entry.
        let source = "const a = Array.from({ length: 99 }, (_, i) => i); a.length = 105; a";
        assert_eq!(
            rendered(source),
            format!("[ {}<6 empty items> ]", dense(99))
        );
    }

    #[test]
    fn a_hole_run_that_fills_the_cap_leaves_the_next_item_in_the_count() {
        // 99 items, a hole run that is the 100th entry, then 2 more items.
        let source = "const a = Array.from({ length: 99 }, (_, i) => i); a[100] = 1; a[101] = 2; a";
        let out = rendered(source);
        assert!(
            out.ends_with(", <1 empty item>, ... 2 more items ]"),
            "{out}"
        );
        assert_eq!(out.matches(", ").count(), 100);
    }

    #[test]
    fn an_array_with_extra_keys_still_stops_at_the_cap() {
        assert_eq!(
            rendered("const a = Array.from({ length: 101 }, (_, i) => i); a.extra = 1; a"),
            numbers(100, ", ... 1 more item")
        );
    }

    #[test]
    fn a_huge_array_is_cut_and_its_tail_counted() {
        assert_eq!(
            rendered("const a = Array.from({ length: 100 }, (_, i) => i); a[4e9] = 1; a"),
            numbers(100, ", ... 3999999901 more items")
        );
    }

    #[rstest]
    #[case::hole_run_then_element("const a = []; a[1e9] = 1; a", "[ <1000000000 empty items>, 1 ]")]
    #[case::max_length("new Array(4294967295)", "[ <4294967295 empty items> ]")]
    #[case::length_assigned("const a = []; a.length = 2 ** 31; a", "[ <2147483648 empty items> ]")]
    #[case::highest_index(
        "const a = []; a[4294967294] = 1; a",
        "[ <4294967294 empty items>, 1 ]"
    )]
    #[case::elements_set_out_of_order(
        "const a = []; a[2e9] = 'y'; a[5e8] = 'x'; a",
        "[ <500000000 empty items>, 'x', <1499999999 empty items>, 'y' ]"
    )]
    #[case::small_indexes_out_of_order(
        "const a = []; a[5] = 1; a[2] = 2; a",
        "[ <2 empty items>, 2, <2 empty items>, 1 ]"
    )]
    #[case::length_past_elements(
        "const a = [1, 2, 3]; a.length = 2 ** 32 - 1; a",
        "[ 1, 2, 3, <4294967292 empty items> ]"
    )]
    #[case::getter_among_holes(
        "const a = []; Object.defineProperty(a, 1, { get() { calls++; } }); a[4] = 1; a",
        "[ <1 empty item>, [Getter], <2 empty items>, 1 ]"
    )]
    #[case::past_nesting_depth(
        "({ a: { b: { c: (() => { const a = []; a[1e9] = 1; return a; })() } } })",
        "{ a: { b: { c: [Array] } } }"
    )]
    fn a_sparse_array_costs_its_present_elements_and_not_its_length(
        #[case] source: &str,
        #[case] shown: &str,
    ) {
        let started = std::time::Instant::now();
        assert_eq!(counted(source), (shown.to_owned(), 0), "{source}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
}
