// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Focus through `tabindex` in a real `.vue` app.

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_click_focuses_a_node_with_tabindex(cx: &mut TestAppContext) {
    let mut h = load(cx, "tabindex");

    click(&mut h, "target");

    assert_eq!(text_of(&mut h, "log"), "target:focus");
}

#[gpui::test]
fn focus_on_a_node_without_tabindex_does_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "tabindex");

    click(&mut h, "focus-bare");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn a_press_on_an_empty_area_blurs_the_focused_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "tabindex");
    click(&mut h, "target");

    click(&mut h, "empty");

    assert_eq!(text_of(&mut h, "log"), "target:focus target:blur");
}

#[gpui::test]
fn changing_tabindex_to_a_negative_number_keeps_the_node_focusable(cx: &mut TestAppContext) {
    let mut h = load(cx, "tabindex");
    click(&mut h, "target");

    click(&mut h, "negative");
    click(&mut h, "empty");
    click(&mut h, "target");

    assert_eq!(
        text_of(&mut h, "log"),
        "target:focus target:blur target:focus"
    );
}

#[gpui::test]
fn removing_tabindex_blurs_the_node_and_stops_click_focus(cx: &mut TestAppContext) {
    let mut h = load(cx, "tabindex");
    click(&mut h, "target");

    click(&mut h, "remove");
    click(&mut h, "target");

    assert_eq!(text_of(&mut h, "log"), "target:focus target:blur");
}
