// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bubbling and `stop` in real `.vue` apps.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_click_runs_the_child_handler_before_the_parent_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "bubble");

    click(&mut h, "inner");

    assert_eq!(text_of(&mut h, "log"), "inner outer");
}

#[gpui::test]
fn a_click_on_the_parent_alone_skips_the_child_handler(cx: &mut TestAppContext) {
    let mut h = load(cx, "bubble");
    let id = by_id(&h.snapshot(), "outer").id;

    // The center of `outer` lies right of `inner`.
    h.click(id);

    assert_eq!(text_of(&mut h, "log"), "outer");
}

#[gpui::test]
fn stop_propagation_keeps_the_parent_handler_from_running(cx: &mut TestAppContext) {
    let mut h = load(cx, "stop");

    click(&mut h, "inner");

    assert_eq!(text_of(&mut h, "log"), "inner");
}
