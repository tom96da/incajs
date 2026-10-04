// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `.once` and handler arrays in real `.vue` apps.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn click_once_runs_a_single_time_across_several_clicks(cx: &mut TestAppContext) {
    let mut h = load(cx, "once");
    assert_eq!(text_of(&mut h, "count"), "0");

    click(&mut h, "box");
    click(&mut h, "box");
    click(&mut h, "box");

    assert_eq!(text_of(&mut h, "count"), "1");
}

#[gpui::test]
fn an_array_of_handlers_runs_every_handler_in_order(cx: &mut TestAppContext) {
    let mut h = load(cx, "handlers");

    click(&mut h, "all");

    assert_eq!(text_of(&mut h, "log"), "a b parent");
}

#[gpui::test]
fn stop_immediate_propagation_skips_the_later_handlers_and_the_parent(cx: &mut TestAppContext) {
    let mut h = load(cx, "handlers");

    click(&mut h, "halted");

    assert_eq!(text_of(&mut h, "log"), "a halt");
}
