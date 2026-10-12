// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tab focus brings a node into view of its scroll container in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, text_of};
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
