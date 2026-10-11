// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Box geometry read through template refs in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn the_rect_is_the_bounds_the_node_is_drawn_at(cx: &mut TestAppContext) {
    let mut h = load(cx, "measure");
    click(&mut h, "measure");

    let rect: serde_json::Value = serde_json::from_str(&text_of(&mut h, "rect")).unwrap();
    let drawn = by_id(&h.snapshot(), "inner").bounds.unwrap();
    for (key, expected) in [
        ("x", drawn.x),
        ("y", drawn.y),
        ("width", drawn.width),
        ("height", drawn.height),
    ] {
        let actual = rect[key].as_f64().unwrap();
        assert!(
            (actual - f64::from(expected)).abs() < 0.01,
            "{key}: {actual} is not {expected}"
        );
    }
}

#[gpui::test]
fn offset_sizes_and_position_are_the_declared_box(cx: &mut TestAppContext) {
    let mut h = load(cx, "measure");
    click(&mut h, "measure");

    // Width and height are the declared 50x30. Left and top are the 10px padding of `outer`.
    assert_eq!(text_of(&mut h, "offset"), "50,30,10,10");
}

#[gpui::test]
fn client_size_excludes_the_border_on_both_sides(cx: &mut TestAppContext) {
    let mut h = load(cx, "measure");
    click(&mut h, "measure");

    assert_eq!(text_of(&mut h, "client"), "46,26");
}

#[gpui::test]
fn offset_parent_is_the_containing_ref(cx: &mut TestAppContext) {
    let mut h = load(cx, "measure");
    click(&mut h, "measure");

    assert_eq!(text_of(&mut h, "parent"), "true");
}

#[gpui::test]
fn a_read_during_mount_sees_zeros_and_a_null_offset_parent(cx: &mut TestAppContext) {
    let mut h = load(cx, "measure");

    assert_eq!(text_of(&mut h, "mounted"), "0,0,null");
}

#[gpui::test]
fn a_click_at_the_centre_reads_the_offset_inside_the_box(cx: &mut TestAppContext) {
    let mut h = load(cx, "offset");
    click(&mut h, "box");

    // The 100x50 box sits at (10,10) with a 2px border, so its centre is
    // client (60,35) and offset (60-12, 35-12).
    assert_eq!(text_of(&mut h, "result"), "48,23,60,35");
}
