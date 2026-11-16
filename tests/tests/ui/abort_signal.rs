// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `AbortController` and `AbortSignal` in a real `.vue` app.

use std::time::Duration;

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn onabort_of_a_signal_runs_in_the_app(cx: &mut TestAppContext) {
    let mut h = load(cx, "abort-signal");

    click(&mut h, "abort");

    assert_eq!(text_of(&mut h, "handled"), "onabort");
}

#[gpui::test]
fn a_timeout_signal_aborts_after_the_clock_advances(cx: &mut TestAppContext) {
    let mut h = load(cx, "abort-signal");
    assert_eq!(text_of(&mut h, "timed-out"), "waiting");

    h.advance_clock(Duration::from_millis(60));
    h.settle();

    assert_eq!(text_of(&mut h, "timed-out"), "timeout");
}

#[gpui::test]
fn a_click_listener_with_a_signal_stops_after_the_abort(cx: &mut TestAppContext) {
    let mut h = load(cx, "abort-signal");
    click(&mut h, "counted");
    assert_eq!(text_of(&mut h, "clicks"), "1");

    click(&mut h, "stop");
    click(&mut h, "counted");

    assert_eq!(text_of(&mut h, "clicks"), "1");
}
