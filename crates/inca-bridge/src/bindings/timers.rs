// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::rc::Rc;

use rquickjs::Result as JsResult;

use super::Natives;
use super::convert::{JsDelay, JsTimerId};

/// Sets `startTimer` and `stopTimer`, which queue and drop timers in the
/// host's timers. `startTimer(delay, repeat)` returns the new id, or
/// `undefined` after the last id.
pub(super) fn install_timers(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("startTimer", |host| {
        let timers = Rc::clone(&host.borrow().timers);
        move |JsDelay(delay), repeat: bool| timers.start(delay, repeat)
    })?;
    n.set("stopTimer", |host| {
        let timers = Rc::clone(&host.borrow().timers);
        move |JsTimerId(id)| timers.stop(id)
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::engine_with_bindings;

    #[rstest]
    #[case::start_fractional_delay("__inca_native__.startTimer(0.5, false);")]
    #[case::start_number_repeat("__inca_native__.startTimer(0, 1);")]
    #[case::stop_fractional_id("__inca_native__.stopTimer(1.5);")]
    fn a_malformed_timer_argument_throws_a_type_error(#[case] call: &str) {
        let (engine, host) = engine_with_bindings();

        let thrown: String = engine
            .eval(&format!(
                r"
                try {{
                    {call}
                    'none';
                }} catch (error) {{
                    error.name;
                }}
                "
            ))
            .unwrap();

        assert_eq!(thrown, "TypeError", "{call}");
        assert!(host.borrow().timers.next_due().is_none(), "{call}");
    }
}
