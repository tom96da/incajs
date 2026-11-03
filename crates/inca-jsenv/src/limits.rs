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
    use std::thread;
    use std::time::Duration;

    use rstest::rstest;

    use super::*;
    use crate::tests::with_realm;
    use crate::vault::create_vault;
    use crate::{Engine, EngineError};

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

    /// Spins for 5 s, so an engine that stops nothing ends the test by its failure within 5 s.
    const SPINS: &str = r"
        const end = Date.now() + 5000;
        while (Date.now() < end) {}
    ";
    /// Runs past one poll of the interrupt handler (every 10 000 steps).
    const LOOPS: &str = r"
        let n = 0;
        for (let i = 0; i < 30000; i += 1) {
            n += 1;
        }
    ";
    /// The same loop, run by a job.
    const LOOPS_IN_A_JOB: &str = r"
        Promise.resolve().then(() => {
            let n = 0;
            for (let i = 0; i < 30000; i += 1) {
                n += 1;
            }
        });
    ";
    /// Queues two jobs per job and spins 1 ms in each, so only a stop of the drain ends it before
    /// the cap of 5 s. The jobs queued at the stop must not run.
    const FANS_OUT: &str = r"
        const end = Date.now() + 5000;
        const spawn = () => {
            if (Date.now() >= end) {
                return;
            }
            const start = Date.now();
            while (Date.now() - start < 1) {}
            Promise.resolve().then(spawn);
            Promise.resolve().then(spawn);
        };
        spawn();
    ";
    const WAITS_FOREVER: &str = r"
        await new Promise(() => {});
    ";
    const FILLS_MEMORY: &str = r"
        const keep = [];
        for (let i = 0; i < 16; i += 1) {
            keep.push(new ArrayBuffer(8 * 1024 * 1024));
        }
    ";
    /// The number of calls that nest before the stack limit throws.
    const RECURSES: &str = r"
        (() => {
            let depth = 0;
            function recurse() {
                depth += 1;
                recurse();
            }
            try {
                recurse();
            } catch {}
            return depth;
        })()
    ";
    /// The time limit of the stop tests: far above what a loaded machine needs for the work that
    /// must finish, far below the 5 s of what must be stopped.
    const LIMIT_MS: u64 = 300;
    /// The wait that outlasts the limit.
    const PASSED: Duration = Duration::from_millis(2 * LIMIT_MS);

    fn stopped() -> String {
        format!("InternalError: interrupted. the script ran longer than {LIMIT_MS} ms")
    }

    fn engine_with(limits: Limits) -> Engine {
        Engine::builder().limits(limits).build().unwrap()
    }

    fn timed(millis: u64) -> Limits {
        Limits {
            time: Duration::from_millis(millis),
            ..Limits::default()
        }
    }

    fn memory_of(mib: usize) -> Limits {
        Limits {
            memory: mib * MIB,
            ..Limits::default()
        }
    }

    #[test]
    fn a_call_past_the_time_limit_is_stopped_with_the_limit_named() {
        let engine = engine_with(timed(LIMIT_MS));

        let err = engine.eval::<()>(SPINS).unwrap_err();

        assert_eq!(err.message(), stopped());
    }

    #[test]
    fn a_stopped_call_keeps_its_frames() {
        let engine = engine_with(timed(LIMIT_MS));

        let err = engine.eval::<()>(SPINS).unwrap_err();

        assert!(err.stack().unwrap_or_default().contains("eval_script"));
    }

    #[test]
    fn a_call_through_with_is_stopped_at_the_time_limit() {
        let engine = engine_with(timed(LIMIT_MS));

        let message = engine.with(|ctx| {
            let err = ctx.eval::<(), _>(SPINS).unwrap_err();
            EngineError::capture(&ctx, &err).message().to_owned()
        });

        assert_eq!(message, stopped());
    }

    #[test]
    fn a_module_that_loops_fails_with_the_stop_on_its_first_poll() {
        let engine = engine_with(timed(LIMIT_MS));

        let module = engine.start_module("spins.mjs", SPINS).unwrap();
        let result = engine.poll_module(&module).unwrap();

        assert_eq!(result.unwrap_err().message(), stopped());
    }

    #[test]
    fn a_drain_of_jobs_stops_at_the_time_limit() {
        let engine = engine_with(Limits {
            memory: 64 * MIB,
            ..timed(LIMIT_MS)
        });
        engine.eval::<()>(FANS_OUT).unwrap();

        let failures = engine.run_jobs();

        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].message(), stopped());
    }

    #[test]
    fn polling_a_module_stops_at_the_time_limit() {
        let engine = engine_with(Limits {
            memory: 64 * MIB,
            ..timed(LIMIT_MS)
        });
        let module = engine
            .start_module("fans-out.mjs", &format!("{FANS_OUT}{WAITS_FOREVER}"))
            .unwrap();

        assert!(engine.poll_module(&module).is_none());

        let failures = engine.run_jobs();
        // The stop of the poll comes first, and the drain stops again on the jobs that are left.
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].message(), stopped());
        assert_eq!(failures[1].message(), stopped());
    }

    #[test]
    fn a_drain_after_a_stopped_call_runs_every_job() {
        let engine = engine_with(timed(LIMIT_MS));
        engine.eval::<()>(SPINS).unwrap_err();
        engine
            .eval::<()>(
                r"
                globalThis.ran = 0;
                for (let i = 0; i < 3; i += 1) {
                    Promise.resolve().then(() => {
                        ran += 1;
                    });
                }
                ",
            )
            .unwrap();

        assert!(engine.run_jobs().is_empty());

        assert_eq!(engine.eval::<u32>("ran").unwrap(), 3);
    }

    /// `run_jobs` arms a deadline, and the wait of `PASSED` later it has passed. An entry
    /// that forgets to arm its own runs under that stale one.
    fn let_the_time_pass(engine: &Engine) {
        let _ = engine.run_jobs();
        thread::sleep(PASSED);
    }

    fn eval_after_the_time_passed(engine: &Engine) -> bool {
        let_the_time_pass(engine);
        engine.eval::<()>(LOOPS).is_ok()
    }

    fn with_after_the_time_passed(engine: &Engine) -> bool {
        let_the_time_pass(engine);
        engine.with(|ctx| ctx.eval::<(), _>(LOOPS).is_ok())
    }

    fn start_module_after_the_time_passed(engine: &Engine) -> bool {
        let_the_time_pass(engine);
        let module = engine.start_module("loops.mjs", LOOPS).unwrap();
        engine
            .poll_module(&module)
            .is_some_and(|result| result.is_ok())
    }

    fn poll_module_after_the_time_passed(engine: &Engine) -> bool {
        let source = format!("await Promise.resolve();{LOOPS}");
        let module = engine.start_module("loops.mjs", &source).unwrap();
        thread::sleep(PASSED);
        engine
            .poll_module(&module)
            .is_some_and(|result| result.is_ok())
    }

    fn run_jobs_after_the_time_passed(engine: &Engine) -> bool {
        engine.eval::<()>(LOOPS_IN_A_JOB).unwrap();
        thread::sleep(PASSED);
        engine.run_jobs().is_empty()
    }

    #[rstest]
    #[case::eval(eval_after_the_time_passed)]
    #[case::with(with_after_the_time_passed)]
    #[case::start_module(start_module_after_the_time_passed)]
    #[case::poll_module(poll_module_after_the_time_passed)]
    #[case::run_jobs(run_jobs_after_the_time_passed)]
    fn every_call_into_the_engine_gets_its_own_time(#[case] call: fn(&Engine) -> bool) {
        let engine = engine_with(timed(LIMIT_MS));

        assert!(call(&engine));
    }

    #[test]
    fn an_allocation_past_the_memory_limit_throws_out_of_memory() {
        let engine = engine_with(memory_of(64));

        let err = engine.eval::<()>(FILLS_MEMORY).unwrap_err();

        assert_eq!(err.message(), "InternalError: out of memory");
    }

    #[rstest]
    #[case::with_a_memory_limit(
        memory_of(64),
        "null. the engine also throws null when it runs out of memory"
    )]
    #[case::without_a_memory_limit(Limits::default(), "null")]
    fn a_thrown_null_reads_by_the_memory_limit(#[case] limits: Limits, #[case] expected: &str) {
        let engine = engine_with(limits);

        let err = engine.eval::<()>("throw null").unwrap_err();

        assert_eq!(err.message(), expected);
    }

    #[test]
    fn an_error_that_looks_like_a_stop_keeps_its_text_under_a_time_limit() {
        let engine = engine_with(timed(LIMIT_MS));

        let err = engine
            .eval::<()>("throw new InternalError('interrupted')")
            .unwrap_err();

        assert_eq!(err.message(), "InternalError: interrupted");
    }

    #[test]
    fn an_error_that_looks_like_a_stop_keeps_its_text_after_a_stopped_call() {
        let engine = engine_with(timed(LIMIT_MS));
        engine.eval::<()>(SPINS).unwrap_err();

        let err = engine
            .eval::<()>("throw new InternalError('interrupted')")
            .unwrap_err();

        assert_eq!(err.message(), "InternalError: interrupted");
    }

    #[test]
    fn an_error_that_looks_like_a_stop_keeps_its_text_without_a_time_limit() {
        let engine = engine_with(Limits::default());

        let err = engine
            .eval::<()>("throw new InternalError('interrupted')")
            .unwrap_err();

        assert_eq!(err.message(), "InternalError: interrupted");
    }

    fn depth_under(stack: usize) -> u32 {
        engine_with(Limits {
            stack,
            ..Limits::default()
        })
        .eval(RECURSES)
        .unwrap()
    }

    #[test]
    fn a_larger_stack_limit_allows_deeper_recursion() {
        assert!(depth_under(512 * 1024) > depth_under(256 * 1024));
    }
}
