// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A `template` element stays out of the window and keeps its content through a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{click, has_id, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_template_with_a_child_draws_only_the_div_beside_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "template");

    let snap = h.snapshot();

    assert!(has_id(&snap, "shown"));
    assert!(!has_id(&snap, "own"));
}

#[gpui::test]
fn the_content_of_a_template_is_readable_after_several_frames(cx: &mut TestAppContext) {
    let mut h = load(cx, "template");
    for _ in 0..3 {
        h.settle();
    }

    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "result"), "shown");
}
