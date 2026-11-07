// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Markup assigned to `innerHTML` reaches the window through a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, near};
use gpui::TestAppContext;

#[gpui::test]
fn a_style_attribute_in_assigned_markup_sizes_the_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "inner-html");

    click(&mut h, "fill");

    let made = by_id(&h.snapshot(), "made").bounds.unwrap();
    near(made.width, 80.0);
}
