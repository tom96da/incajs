// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `<style>` blocks and stylesheet imports of a real `.vue` app applied by the host.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, load, near};
use gpui::TestAppContext;

fn width_of(cx: &mut TestAppContext, id: &str) -> f32 {
    let mut h = load(cx, "style-sfc");
    let snap = h.snapshot();
    by_id(&snap, id).bounds.unwrap().width
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
