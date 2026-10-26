// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::float_cmp, clippy::unwrap_used)]

use std::time::Duration;

use inca_bridge::test_support::TIMERS_JS;

use super::*;

#[gpui::test]
fn advance_clock_runs_a_due_timer_before_it_returns(cx: &mut TestAppContext) {
    let mut h = Harness::load(
        cx,
        TEST_ENTRY_PATH,
        &[
            TIMERS_JS,
            r"
            globalThis.ticks = 0;
            setTimeout(() => {
                globalThis.ticks += 1;
            }, 100);
            ",
        ]
        .concat(),
    );

    h.advance_clock(Duration::from_millis(99));
    assert_eq!(eval_f64(&h, "globalThis.ticks"), 0.0);
    h.advance_clock(Duration::from_millis(1));
    assert_eq!(eval_f64(&h, "globalThis.ticks"), 1.0);
}
