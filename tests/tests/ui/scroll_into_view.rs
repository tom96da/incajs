// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `scrollIntoView` and the scroll that `focus()` makes, read as the offset of
//! an item inside its container. The container is 100 tall with ten items of
//! 30, so the range ends at 200.

#![allow(clippy::unwrap_used)]

use super::common::{click, left_in, load, near, settled, top_in};
use gpui::TestAppContext;

#[gpui::test]
fn scroll_into_view_without_options_aligns_the_start(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-default");

    near(top_in(&mut h, "item-5", "port"), 0.0);
}

#[gpui::test]
fn scroll_into_view_with_block_start_aligns_the_start(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-start");

    near(top_in(&mut h, "item-5", "port"), 0.0);
}

#[gpui::test]
fn scroll_into_view_with_block_center_centers_the_item(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-center");

    near(top_in(&mut h, "item-5", "port"), 35.0);
}

#[gpui::test]
fn scroll_into_view_with_block_end_aligns_the_end(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-end");

    near(top_in(&mut h, "item-5", "port"), 70.0);
}

#[gpui::test]
fn nearest_aligns_the_end_of_an_item_below_the_view(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-nearest");

    near(top_in(&mut h, "item-5", "port"), 70.0);
}

#[gpui::test]
fn nearest_aligns_the_start_of_an_item_above_the_view(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "jump-200");
    click(&mut h, "si-nearest-2");

    near(top_in(&mut h, "item-2", "port"), 0.0);
}

#[gpui::test]
fn nearest_leaves_a_fully_visible_item_in_place(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-nearest-1");

    near(top_in(&mut h, "item-1", "port"), 30.0);
}

#[gpui::test]
fn scroll_into_view_stops_at_the_end_of_the_range(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-start-9");

    near(top_in(&mut h, "item-9", "port"), 70.0);
}

#[gpui::test]
fn scroll_into_view_on_a_node_appended_in_the_same_call_reaches_the_range_end(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "append-reveal");
    // the frame that records the box of the appended node, then the frame whose reveal uses it
    settled(&mut h, 2);

    near(top_in(&mut h, "extra", "port"), 70.0);
}

#[gpui::test]
fn focus_scrolls_a_node_outside_the_view_to_the_center(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "focus-5");

    near(top_in(&mut h, "item-5", "port"), 35.0);
}

#[gpui::test]
fn focus_moves_a_partly_visible_node_to_the_nearest_edge(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "jump-10");
    click(&mut h, "focus-3");

    near(top_in(&mut h, "item-3", "port"), 70.0);
}

#[gpui::test]
fn focus_moves_a_node_partly_above_the_view_to_the_start(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "jump-20");
    click(&mut h, "focus-0");

    near(top_in(&mut h, "item-0", "port"), 0.0);
}

#[gpui::test]
fn focus_leaves_a_fully_visible_node_in_place(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "focus-1");

    near(top_in(&mut h, "item-1", "port"), 30.0);
}

#[gpui::test]
fn focus_with_prevent_scroll_keeps_the_offset(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "focus-5-prevent");

    near(top_in(&mut h, "item-5", "port"), 150.0);
}

#[gpui::test]
fn scroll_into_view_with_inline_center_centers_horizontally(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-inline-center");

    near(left_in(&mut h, "h-5", "hport"), 10.0);
}

#[gpui::test]
fn scroll_into_view_with_inline_end_aligns_the_right_edge(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-inline-end");

    near(left_in(&mut h, "h-5", "hport"), 20.0);
}

#[gpui::test]
fn scroll_into_view_true_aligns_the_start(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-true");

    near(top_in(&mut h, "item-5", "port"), 0.0);
}

#[gpui::test]
fn scroll_into_view_false_aligns_the_end(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll-into-view");
    click(&mut h, "si-false");

    near(top_in(&mut h, "item-5", "port"), 70.0);
}
