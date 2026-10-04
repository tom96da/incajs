// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Lays out real `.vue` apps and compares box positions and sizes.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, load, near};
use gpui::TestAppContext;

#[gpui::test]
fn a_row_places_children_after_the_padding_with_gaps_between(cx: &mut TestAppContext) {
    let snap = load(cx, "row").snapshot();
    let row = by_id(&snap, "row").bounds.unwrap();
    let a = by_id(&snap, "a").bounds.unwrap();
    let b = by_id(&snap, "b").bounds.unwrap();

    near(row.width, 400.0);
    near(a.width, 50.0);
    near(a.height, 30.0);
    near(b.width, 60.0);
    near(a.x, row.x + 20.0);
    near(a.y, row.y + 20.0);
    near(b.x, a.x + a.width + 10.0);
    near(b.y, a.y);
}

#[gpui::test]
fn a_row_child_with_flex_grow_takes_the_remaining_width(cx: &mut TestAppContext) {
    let snap = load(cx, "row").snapshot();
    let row = by_id(&snap, "row").bounds.unwrap();
    let b = by_id(&snap, "b").bounds.unwrap();
    let c = by_id(&snap, "c").bounds.unwrap();

    near(c.x, b.x + b.width + 10.0);
    near(c.width, 400.0 - 40.0 - 50.0 - 60.0 - 20.0);
    near(c.x + c.width, row.x + row.width - 20.0);
}

#[gpui::test]
fn a_column_stacks_children_below_the_padding_with_gaps(cx: &mut TestAppContext) {
    let snap = load(cx, "column").snapshot();
    let column = by_id(&snap, "column").bounds.unwrap();
    let a = by_id(&snap, "a").bounds.unwrap();
    let b = by_id(&snap, "b").bounds.unwrap();

    near(a.x, column.x + 10.0);
    near(a.y, column.y + 10.0);
    near(b.y, a.y + a.height + 8.0);
    near(b.x, a.x);
}

#[gpui::test]
fn column_children_split_the_free_height_by_flex_grow(cx: &mut TestAppContext) {
    let snap = load(cx, "column").snapshot();
    let column = by_id(&snap, "column").bounds.unwrap();
    let b = by_id(&snap, "b").bounds.unwrap();
    let c = by_id(&snap, "c").bounds.unwrap();

    // 310 - 20 padding - 40 fixed - 16 gaps = 234, split 1:2.
    near(b.height, 78.0);
    near(c.height, 156.0);
    near(c.y, b.y + b.height + 8.0);
    near(c.y + c.height, column.y + column.height - 10.0);
}
