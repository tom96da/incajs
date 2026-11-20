// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `:hover` and `:focus` rules applied through a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{hover, load, near, width};
use gpui::TestAppContext;

#[gpui::test]
fn hovering_a_box_without_listeners_applies_its_hover_rule_until_unhover(cx: &mut TestAppContext) {
    let mut h = load(cx, "visual-hover");
    near(width(&mut h, "box"), 40.0);

    hover(&mut h, "box");
    near(width(&mut h, "box"), 60.0);

    h.unhover();
    near(width(&mut h, "box"), 40.0);
}

#[gpui::test]
fn tab_applies_the_focus_rule_to_the_input(cx: &mut TestAppContext) {
    let mut h = load(cx, "visual-focus");

    h.keystrokes("tab");

    near(width(&mut h, "first"), 60.0);
}

#[gpui::test]
fn the_next_tab_removes_the_focus_rule_from_the_input(cx: &mut TestAppContext) {
    let mut h = load(cx, "visual-focus");
    h.keystrokes("tab");

    h.keystrokes("tab");

    near(width(&mut h, "first"), 40.0);
}
