// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Wheel scrolling of a fixed-size container in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, load, near, text_of};
use gpui::TestAppContext;
use inca_gpui::NodeId;
use inca_host::harness::Harness;

fn y(h: &mut Harness, id: &str) -> f32 {
    by_id(&h.snapshot(), id).bounds.unwrap().y
}

fn node(h: &mut Harness, id: &str) -> NodeId {
    by_id(&h.snapshot(), id).id
}

#[gpui::test]
fn scrolling_down_moves_the_children_up_by_the_delta(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let (first, second) = (y(&mut h, "first"), y(&mut h, "second"));
    let box_ = node(&mut h, "box");

    h.scroll(box_, 0.0, -30.0);

    near(y(&mut h, "first"), first - 30.0);
    near(y(&mut h, "second"), second - 30.0);
}

#[gpui::test]
fn scrolling_back_restores_the_children(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let first = y(&mut h, "first");
    let box_ = node(&mut h, "box");

    h.scroll(box_, 0.0, -30.0);
    h.scroll(box_, 0.0, 30.0);

    near(y(&mut h, "first"), first);
}

#[gpui::test]
fn scrolling_stops_at_the_end_of_the_content(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let first = y(&mut h, "first");
    let box_ = node(&mut h, "box");

    h.scroll(box_, 0.0, -500.0);

    // 180 of content in a 100 box leaves 80 to scroll.
    near(y(&mut h, "first"), first - 80.0);
}

#[gpui::test]
fn a_wheel_handler_sees_the_scroll_delta(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    assert_eq!(text_of(&mut h, "seen"), "none");
    let box_ = node(&mut h, "box");

    h.scroll(box_, 0.0, -30.0);

    assert_eq!(text_of(&mut h, "seen"), "30");
}

#[gpui::test]
fn prevent_default_on_wheel_keeps_the_container_from_scrolling(cx: &mut TestAppContext) {
    let mut h = load(cx, "scroll");
    let first = y(&mut h, "locked-first");
    let locked = node(&mut h, "locked");

    h.scroll(locked, 0.0, -30.0);

    near(y(&mut h, "locked-first"), first);
}
