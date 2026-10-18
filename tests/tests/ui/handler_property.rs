// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A template sets an element's `onclick` property in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_click_runs_the_function_a_template_binds_to_onclick(cx: &mut TestAppContext) {
    let mut h = load(cx, "handler-property");
    assert_eq!(text_of(&mut h, "count"), "0");

    click(&mut h, "bump");
    click(&mut h, "bump");

    assert_eq!(text_of(&mut h, "count"), "2");
}

#[gpui::test]
fn an_onclick_property_that_returns_false_cancels_the_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "handler-property");

    click(&mut h, "cancel");

    assert_eq!(text_of(&mut h, "log"), "prevented:true");
}
