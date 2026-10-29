// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a script may use before the engine stops it, and the vault keys that
//! let an error report name the limit.

use std::cell::Cell;
use std::time::{Duration, Instant};

use rquickjs::{Ctx, FromJs, Result as JsResult};

use crate::vault::vault;

/// The largest stack limit the engine takes. `QuickJS` reads a larger value as
/// no limit.
const MAX_STACK: usize = 16 * 1024 * 1024;
/// The vault key of the time limit, in milliseconds.
const TIME_LIMIT: &str = "timeLimit";
/// The vault key that is `true` while the engine has a memory limit.
const MEMORY_LIMITED: &str = "memoryLimited";

thread_local! {
    /// Whether the interrupt handler stopped the latest call on this thread. The handler has no
    /// [`Ctx`], so the vault cannot carry it.
    static STOPPED: Cell<bool> = const { Cell::new(false) };
}

/// What an [`Engine`](crate::Engine) allows a script. A field left at `0` sets
/// no limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    /// Bytes of heap. An allocation past the limit throws
    /// `InternalError: out of memory`.
    pub memory: usize,
    /// Bytes of native stack, at most 16 MiB. `0` keeps the engine's default of
    /// 1 MiB. Deeper recursion throws `RangeError: Maximum call stack size
    /// exceeded`. The thread that runs the engine needs a larger stack than the
    /// limit.
    pub stack: usize,
    /// How long one call into the engine may run: `eval`, `with`,
    /// `start_module`, `poll_module` or `run_jobs`. A call that outlives it is
    /// stopped with an error that `catch` and `finally` blocks do not see.
    pub time: Duration,
}

impl Limits {
    /// The stack size to give the engine, or `None` to keep its default.
    pub(crate) fn stack_limit(self) -> Option<usize> {
        (self.stack != 0).then(|| self.stack.min(MAX_STACK))
    }
}

/// The end of a call that starts at `now`, or `None` when there is none: a zero
/// time limit, or one too long for an [`Instant`].
fn deadline(now: Instant, time: Duration) -> Option<Instant> {
    if time.is_zero() {
        None
    } else {
        now.checked_add(time)
    }
}

/// The milliseconds of `time`, at most `u32::MAX`.
fn time_limit_ms(time: Duration) -> u32 {
    u32::try_from(time.as_millis()).unwrap_or(u32::MAX)
}

/// Tells the interrupt handler when the running call is out of time.
pub(crate) struct Watchdog {
    time: Duration,
    deadline: Cell<Option<Instant>>,
    /// Whether the deadline passed during the running call.
    tripped: Cell<bool>,
}

impl Watchdog {
    pub(crate) fn new(time: Duration) -> Self {
        Self {
            time,
            deadline: Cell::new(None),
            tripped: Cell::new(false),
        }
    }

    /// Starts the time of a call into the engine.
    pub(crate) fn arm(&self) {
        self.deadline.set(deadline(Instant::now(), self.time));
        self.tripped.set(false);
        STOPPED.set(false);
    }

    /// The answer of the interrupt handler: whether the running call is out of
    /// time.
    pub(crate) fn expired(&self) -> bool {
        let expired = self
            .deadline
            .get()
            .is_some_and(|deadline| Instant::now() >= deadline);
        if expired {
            self.tripped.set(true);
            STOPPED.set(true);
        }
        expired
    }

    /// Whether the running call ran out of time.
    pub(crate) fn tripped(&self) -> bool {
        self.tripped.get()
    }
}

/// Records the limits in the vault of `ctx`'s realm, where an error report
/// reads them.
pub(crate) fn record_limits(ctx: &Ctx<'_>, limits: Limits) -> JsResult<()> {
    let vault = vault(ctx)?;
    if !limits.time.is_zero() {
        vault.set(TIME_LIMIT, time_limit_ms(limits.time))?;
    }
    if limits.memory != 0 {
        vault.set(MEMORY_LIMITED, true)?;
    }
    Ok(())
}

/// The value under `key` in the vault, or `None` when the realm has no vault or
/// the key.
fn recorded<'js, T: FromJs<'js>>(ctx: &Ctx<'js>, key: &str) -> Option<T> {
    vault(ctx).ok()?.get::<_, Option<T>>(key).ok()?
}

/// The text of the error `QuickJS` throws when the handler stops a call, with
/// the time limit named. `None` for any other error, for an error thrown while
/// no stop happened, and for an engine with no time limit.
pub(crate) fn interrupted(ctx: &Ctx<'_>, name: &str, message: &str) -> Option<String> {
    if !STOPPED.get() || name != "InternalError" || message != "interrupted" {
        return None;
    }
    let ms = recorded::<u32>(ctx, TIME_LIMIT)?;
    Some(format!(
        "{name}: {message}. the script ran longer than {ms} ms"
    ))
}

/// The text of a thrown `null` on an engine with a memory limit, where the
/// engine throws `null` when it cannot allocate an error. `None` without a
/// memory limit.
pub(crate) fn null_under_memory_limit(ctx: &Ctx<'_>) -> Option<String> {
    recorded::<bool>(ctx, MEMORY_LIMITED)?
        .then(|| "null. the engine also throws null when it runs out of memory".to_owned())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use rstest::rstest;

    use super::*;
    use crate::tests::with_realm;
    use crate::vault::create_vault;

    const MIB: usize = 1024 * 1024;

    #[rstest]
    #[case::zero(0, None)]
    #[case::one_mib(MIB, Some(MIB))]
    #[case::the_cap(16 * MIB, Some(16 * MIB))]
    #[case::above_the_cap(16 * MIB + 1, Some(16 * MIB))]
    #[case::the_largest(usize::MAX, Some(16 * MIB))]
    fn the_stack_limit_is_capped_at_16_mib(#[case] stack: usize, #[case] expected: Option<usize>) {
        let limits = Limits {
            stack,
            ..Limits::default()
        };

        assert_eq!(limits.stack_limit(), expected);
    }

    #[rstest]
    #[case::zero(Duration::ZERO, None)]
    #[case::one_second(Duration::from_secs(1), Some(Duration::from_secs(1)))]
    #[case::too_long_for_an_instant(Duration::MAX, None)]
    fn a_deadline_is_the_start_plus_the_time_when_there_is_one(
        #[case] time: Duration,
        #[case] after: Option<Duration>,
    ) {
        let now = Instant::now();

        assert_eq!(deadline(now, time), after.map(|after| now + after));
    }

    #[rstest]
    #[case::milliseconds(Duration::from_millis(50), 50)]
    #[case::seconds(Duration::from_secs(10), 10_000)]
    #[case::just_above_a_u32(Duration::from_millis(1 << 32), u32::MAX)]
    fn the_time_limit_is_recorded_in_milliseconds(#[case] time: Duration, #[case] expected: u32) {
        assert_eq!(time_limit_ms(time), expected);
    }

    fn expired_watchdog() -> Watchdog {
        let watchdog = Watchdog::new(Duration::from_millis(50));
        watchdog.arm();
        watchdog.deadline.set(Some(Instant::now()));
        watchdog
    }

    fn timed_realm(ctx: &Ctx<'_>) {
        create_vault(ctx).unwrap();
        let limits = Limits {
            time: Duration::from_millis(50),
            ..Limits::default()
        };
        record_limits(ctx, limits).unwrap();
    }

    #[test]
    fn an_interrupt_error_is_named_once_the_handler_stopped_the_call() {
        with_realm(|ctx| {
            timed_realm(&ctx);
            assert!(expired_watchdog().expired());

            let text = interrupted(&ctx, "InternalError", "interrupted");

            assert_eq!(
                text.as_deref(),
                Some("InternalError: interrupted. the script ran longer than 50 ms")
            );
        });
    }

    #[test]
    fn an_interrupt_error_before_any_stop_is_not_named() {
        with_realm(|ctx| {
            timed_realm(&ctx);
            Watchdog::new(Duration::from_millis(50)).arm();

            assert_eq!(interrupted(&ctx, "InternalError", "interrupted"), None);
        });
    }

    #[test]
    fn arming_forgets_the_stop_of_the_call_before() {
        with_realm(|ctx| {
            timed_realm(&ctx);
            let watchdog = expired_watchdog();
            assert!(watchdog.expired());

            watchdog.arm();

            assert_eq!(interrupted(&ctx, "InternalError", "interrupted"), None);
        });
    }

    #[rstest]
    #[case::another_name("RangeError", "interrupted")]
    #[case::another_message("InternalError", "out of memory")]
    fn a_stop_names_only_the_interrupt_error(#[case] name: &str, #[case] message: &str) {
        with_realm(|ctx| {
            timed_realm(&ctx);
            assert!(expired_watchdog().expired());

            assert_eq!(interrupted(&ctx, name, message), None);
        });
    }
}
