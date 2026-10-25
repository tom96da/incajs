// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Text input in real `.vue` apps.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn typing_into_a_v_model_input_updates_the_model(cx: &mut TestAppContext) {
    let mut h = load(cx, "input");
    click(&mut h, "field");

    h.keystrokes("h i");

    assert_eq!(text_of(&mut h, "mirror"), "hi");
}

#[gpui::test]
fn a_model_change_rewrites_the_input(cx: &mut TestAppContext) {
    let mut h = load(cx, "input");
    click(&mut h, "field");
    h.keystrokes("h i");
    click(&mut h, "clear");
    click(&mut h, "field");

    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "mirror"), "x");
}
