// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Clicks real `.vue` apps and checks that reactive state reaches the tree.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, near, text, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_ref_renders_its_initial_value(cx: &mut TestAppContext) {
    let snap = load(cx, "counter").snapshot();

    assert_eq!(text(by_id(&snap, "button")), "Count: 0");
}

#[gpui::test]
fn a_click_increments_the_rendered_ref(cx: &mut TestAppContext) {
    let mut h = load(cx, "counter");
    let button = by_id(&h.snapshot(), "button").id;

    h.click(button);

    assert_eq!(text_of(&mut h, "button"), "Count: 1");
}

#[gpui::test]
fn clicking_twice_increments_twice(cx: &mut TestAppContext) {
    let mut h = load(cx, "counter");
    let button = by_id(&h.snapshot(), "button").id;

    h.click(button);
    h.click(button);

    assert_eq!(text_of(&mut h, "button"), "Count: 2");
}

#[gpui::test]
fn a_click_keeps_the_button_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "counter");
    let button = by_id(&h.snapshot(), "button").id;

    h.click(button);

    assert_eq!(by_id(&h.snapshot(), "button").id, button);
}

#[gpui::test]
fn a_bound_style_resizes_the_box_on_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "resizer");
    let before = by_id(&h.snapshot(), "box").bounds.unwrap();
    let id = by_id(&h.snapshot(), "box").id;
    near(before.width, 100.0);
    near(before.height, 40.0);

    h.click(id);

    let after = by_id(&h.snapshot(), "box").bounds.unwrap();
    near(after.width, 200.0);
    near(after.height, 40.0);
    near(after.x, before.x);
    near(after.y, before.y);
}

#[gpui::test]
fn clicking_the_resized_box_again_restores_its_width(cx: &mut TestAppContext) {
    let mut h = load(cx, "resizer");
    let id = by_id(&h.snapshot(), "box").id;

    h.click(id);
    h.click(id);

    near(by_id(&h.snapshot(), "box").bounds.unwrap().width, 100.0);
}

#[gpui::test]
fn a_bound_style_changes_the_height_on_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "taller");
    let before = by_id(&h.snapshot(), "box").bounds.unwrap();
    near(before.height, 40.0);

    click(&mut h, "box");
    let tall = by_id(&h.snapshot(), "box").bounds.unwrap();
    click(&mut h, "box");
    let back = by_id(&h.snapshot(), "box").bounds.unwrap();

    near(tall.height, 80.0);
    near(tall.width, 100.0);
    near(back.height, 40.0);
}
