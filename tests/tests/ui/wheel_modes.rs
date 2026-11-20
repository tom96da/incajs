// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The line wheel and the Shift wheel in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{load, near, node_id, y};
use gpui::{Modifiers, ScrollDelta, TestAppContext, point};

#[gpui::test]
fn a_line_wheel_down_moves_the_content_up(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-modes");
    let before = y(&mut h, "first");
    let id = node_id(&mut h, "box");

    h.scroll_with(id, ScrollDelta::Lines(point(0.0, -1.0)), Modifiers::none());

    assert!(y(&mut h, "first") < before);
}

#[gpui::test]
fn a_shift_wheel_scrolls_the_vertical_axis(cx: &mut TestAppContext) {
    let mut h = load(cx, "wheel-modes");
    let before = y(&mut h, "cell");
    let id = node_id(&mut h, "grid");

    h.scroll_with(
        id,
        ScrollDelta::Pixels(point(gpui::px(0.0), gpui::px(-30.0))),
        Modifiers::shift(),
    );

    near(before - y(&mut h, "cell"), 30.0);
}
