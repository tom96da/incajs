// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `focus()` and `scrollIntoView()` bring a node into view of its scroll
//! container in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, near, text_of};
use gpui::TestAppContext;
use inca_host::harness::Harness;

/// The bounds of the scroll container's padding box, which is the scrollport:
/// the border box inset by the 3px border.
fn scrollport(h: &mut Harness) -> (f32, f32) {
    let b = by_id(&h.snapshot(), "box").bounds.unwrap();
    (b.y + 3.0, b.height - 6.0)
}

fn item_y(h: &mut Harness, index: usize) -> f32 {
    by_id(&h.snapshot(), &format!("item-{index}"))
        .bounds
        .unwrap()
        .y
}

#[gpui::test]
fn focus_centres_the_node_in_the_scrollport(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus_scroll");

    click(&mut h, "focus20");
    h.settle();

    // The padding box and the content box share a centre because the padding is symmetric.
    let (top, height) = scrollport(&mut h);
    near(item_y(&mut h, 20) + 15.0, top + height / 2.0);
}

#[gpui::test]
fn prevent_scroll_leaves_the_scroll_position_at_zero(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus_scroll");

    click(&mut h, "prevent20");
    h.settle();

    assert_eq!(text_of(&mut h, "scrolltop"), "0");
}

#[gpui::test]
fn scroll_into_view_aligns_the_node_top_to_the_scrollport_top(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus_scroll");

    click(&mut h, "into20");

    // The scrollport is the padding box, so the top aligns 3px below the border edge.
    let (top, _) = scrollport(&mut h);
    near(item_y(&mut h, 20), top);
}

#[gpui::test]
fn tab_from_the_first_item_scrolls_the_focused_item_fully_into_view(cx: &mut TestAppContext) {
    let mut h = load(cx, "focus_scroll");
    click(&mut h, "item-0");

    for _ in 0..25 {
        if text_of(&mut h, "focused") == "20" {
            break;
        }
        h.keystrokes("tab");
    }

    assert_eq!(text_of(&mut h, "focused"), "20");
    let (top, height) = scrollport(&mut h);
    let y = item_y(&mut h, 20);
    assert!(
        y >= top && y + 30.0 <= top + height,
        "item 20 at {y} outside {top}..{}",
        top + height
    );
}
