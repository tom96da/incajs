// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use std::thread;
use std::time::Duration;

use rstest::rstest;

use super::*;
use crate::{EngineError, Limits};

const MIB: usize = 1024 * 1024;
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
/// Queues two jobs per job and spins 1 ms in each, so only a stop of the drain ends it before the
/// cap of 5 s. The jobs queued at the stop must not run.
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

    let failures = engine.take_startup_failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].message(), stopped());
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
