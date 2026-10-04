// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A listener attached by the render a click causes first runs on the
//! next click.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_listener_attached_by_a_click_skips_that_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "timing");

    click(&mut h, "plain-button");

    assert_eq!(text_of(&mut h, "state"), "true 0");
}

#[gpui::test]
fn the_cycle_repeats_after_the_listener_is_removed(cx: &mut TestAppContext) {
    let mut h = load(cx, "timing");

    for _ in 0..3 {
        click(&mut h, "plain-button");
    }
    assert_eq!(text_of(&mut h, "state"), "true 1");

    click(&mut h, "plain-button");
    assert_eq!(text_of(&mut h, "state"), "false 2");
}

#[gpui::test]
fn a_once_listener_attached_by_a_click_skips_that_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "timing");

    click(&mut h, "once-button");
    assert_eq!(text_of(&mut h, "once-state"), "true 0");

    click(&mut h, "once-button");
    assert_eq!(text_of(&mut h, "once-state"), "false 1");
}

#[gpui::test]
fn a_once_listener_attached_again_skips_the_click_that_attaches_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "timing");
    for _ in 0..2 {
        click(&mut h, "once-button");
    }

    click(&mut h, "once-button");
    assert_eq!(text_of(&mut h, "once-state"), "true 1");

    click(&mut h, "once-button");
    assert_eq!(text_of(&mut h, "once-state"), "false 2");
}
