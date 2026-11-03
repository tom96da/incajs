// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rules of a `<style>` element applied through a real `.vue` app.

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
