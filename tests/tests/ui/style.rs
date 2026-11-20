// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The styles a real `.vue` app applies: `<style>` elements and blocks, stylesheet imports and the `style` member.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, near};
use gpui::TestAppContext;
use inca_host::harness::Harness;

fn widths(h: &mut Harness) -> (f32, f32) {
    let snap = h.snapshot();
    (
        by_id(&snap, "a").bounds.unwrap().width,
        by_id(&snap, "b").bounds.unwrap().width,
    )
}

fn width_of(cx: &mut TestAppContext, id: &str) -> f32 {
    let mut h = load(cx, "style-sfc");
    let snap = h.snapshot();
    by_id(&snap, id).bounds.unwrap().width
}

#[gpui::test]
fn an_id_rule_wins_over_a_class_rule_by_specificity(cx: &mut TestAppContext) {
    let mut h = load(cx, "style-element");

    let (a, b) = widths(&mut h);
    near(a, 40.0);
    near(b, 60.0);
}

#[gpui::test]
fn removing_the_style_element_returns_the_boxes_to_their_content_width(cx: &mut TestAppContext) {
    let mut h = load(cx, "style-element");
    click(&mut h, "remove");

    let (a, b) = widths(&mut h);
    near(a, 0.0);
    near(b, 0.0);
}

#[gpui::test]
fn an_inline_width_set_through_the_style_member_is_drawn(cx: &mut TestAppContext) {
    let mut h = load(cx, "style-member");
    click(&mut h, "probe");

    let snap = h.snapshot();
    near(by_id(&snap, "box").bounds.unwrap().width, 40.0);
}

#[gpui::test]
fn a_plain_block_sizes_a_box(cx: &mut TestAppContext) {
    near(width_of(cx, "plain"), 30.0);
}

#[gpui::test]
fn a_scoped_block_sizes_its_own_box(cx: &mut TestAppContext) {
    near(width_of(cx, "scoped"), 40.0);
}

#[gpui::test]
fn a_scoped_block_leaves_the_same_class_in_a_child_component_alone(cx: &mut TestAppContext) {
    near(width_of(cx, "child"), 0.0);
}

#[gpui::test]
fn a_css_import_with_at_import_sizes_a_box(cx: &mut TestAppContext) {
    near(width_of(cx, "imported"), 50.0);
}

#[gpui::test]
fn the_later_of_two_sheets_at_equal_specificity_wins(cx: &mut TestAppContext) {
    near(width_of(cx, "order"), 22.0);
}

#[gpui::test]
fn a_module_block_class_sizes_a_box(cx: &mut TestAppContext) {
    near(width_of(cx, "module"), 70.0);
}
