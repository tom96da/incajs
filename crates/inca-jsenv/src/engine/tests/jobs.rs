// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn an_unhandled_rejection_is_returned_once() {
    let engine = Engine::new().unwrap();
    engine
        .eval::<()>("Promise.reject(new Error('lost'));")
        .unwrap();

    let failures = engine.run_jobs();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].message(), "Error: lost");
    assert!(failures[0].stack().is_some());
    assert!(engine.run_jobs().is_empty());
}

#[test]
fn a_rejection_handled_later_in_the_drain_is_not_returned() {
    let engine = Engine::new().unwrap();
    engine
        .eval::<()>(
            r"
            const p = Promise.reject(new Error('late'));
            Promise.resolve().then(() => p.catch(() => {}));
            ",
        )
        .unwrap();

    assert!(engine.run_jobs().is_empty());
}

#[rstest]
#[case::map_set(
    r"
    Map.prototype.set = function () {
        return this;
    };
    "
)]
#[case::map_iterator(
    r"
    Object.getPrototypeOf(new Map().values()).next = () => ({ done: true });
    "
)]
#[case::map_for_each(
    r"
    Map.prototype.forEach = function () {};
    "
)]
fn a_script_cannot_silence_an_unhandled_rejection(#[case] tamper: &str) {
    let engine = Engine::new().unwrap();
    engine.eval::<()>(tamper).unwrap();
    engine
        .eval::<()>("Promise.reject(new Error('lost'));")
        .unwrap();

    assert_eq!(engine.run_jobs().len(), 1);
}

#[test]
fn a_job_that_throws_is_returned() {
    let engine = Engine::new().unwrap();
    engine
        .eval::<()>("queueMicrotask(() => { throw new Error('job'); });")
        .unwrap();

    let failures = engine.run_jobs();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].message(), "Error: job");
    assert!(failures[0].stack().is_some());
}

#[test]
fn a_job_that_throws_during_top_level_await_is_returned_by_run_jobs() {
    let engine = Engine::new().unwrap();
    engine
        .eval_module(
            "startup.mjs",
            "queueMicrotask(() => { throw new Error('startup job'); }); await null;",
        )
        .unwrap();

    let failures = engine.run_jobs();
    assert_eq!(failures.len(), 1);
    assert!(failures[0].message().contains("startup job"));
    assert!(engine.run_jobs().is_empty());
}

#[test]
fn a_top_level_await_that_resolves_completes_the_module() {
    let engine = Engine::new().unwrap();
    engine
        .eval_module(
            "awaits.mjs",
            "await Promise.resolve(); globalThis.seen = 42;",
        )
        .unwrap();

    let seen: i32 = engine.eval("globalThis.seen").unwrap();
    assert_eq!(seen, 42);
    assert!(engine.run_jobs().is_empty());
}

#[test]
fn a_top_level_await_that_rejects_returns_its_own_error() {
    let engine = Engine::new().unwrap();
    let err = engine
        .eval_module(
            "rejects.mjs",
            "await Promise.resolve(); throw new Error('late boom');",
        )
        .unwrap_err();

    assert_eq!(err.message(), "Error: late boom");
}

#[test]
fn a_top_level_await_that_never_settles_would_block() {
    let engine = Engine::new().unwrap();
    let err = engine
        .eval_module("stuck.mjs", "await new Promise(() => {});")
        .unwrap_err();

    assert!(!err.message().is_empty());
    assert_eq!(err.stack(), None);
}

#[test]
fn a_pending_module_settles_once_a_later_eval_resolves_it() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module(
            "pending.mjs",
            "await new Promise((r) => { globalThis.release = r; }); globalThis.seen = 42;",
        )
        .unwrap();

    assert!(engine.poll_module(&module).is_none());
    engine.eval::<()>("release();").unwrap();
    assert_eq!(engine.poll_module(&module), Some(Ok(())));

    let seen: i32 = engine.eval("globalThis.seen").unwrap();
    assert_eq!(seen, 42);
}

#[test]
fn a_module_cannot_replace_its_own_promise() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module(
            "steals.mjs",
            r"
            for (let key = 0; key < 65536; key++) {
                Object.defineProperty(Object.prototype, key, {
                    get() {
                        return Promise.resolve();
                    },
                    set(promise) {
                        globalThis.stolen = promise;
                    },
                });
            }
            await new Promise(() => {});
            ",
        )
        .unwrap();

    assert!(engine.poll_module(&module).is_none());
    let stolen: String = engine.eval("typeof globalThis.stolen").unwrap();
    assert_eq!(stolen, "undefined");
}

#[test]
fn a_pending_module_returns_a_later_rejection_as_its_own_error() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module(
            "pending-rejects.mjs",
            "await new Promise((_, reject) => { globalThis.fail = reject; });",
        )
        .unwrap();

    assert!(engine.poll_module(&module).is_none());
    engine.eval::<()>("fail(new Error('later'));").unwrap();
    let err = engine.poll_module(&module).unwrap().unwrap_err();
    assert_eq!(err.message(), "Error: later");
}

#[test]
fn a_job_that_throws_during_a_poll_is_returned_by_run_jobs() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module(
            "poll-job.mjs",
            r"
            queueMicrotask(() => { throw new Error('poll job'); });
            await new Promise((r) => { globalThis.release = r; });
            ",
        )
        .unwrap();

    assert!(engine.poll_module(&module).is_none());
    let failures = engine.run_jobs();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].message(), "Error: poll job");
    assert!(engine.run_jobs().is_empty());

    engine.eval::<()>("release();").unwrap();
    assert_eq!(engine.poll_module(&module), Some(Ok(())));
}

#[test]
fn polling_a_completed_module_again_returns_ok() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module("done.mjs", "await Promise.resolve();")
        .unwrap();

    assert_eq!(engine.poll_module(&module), Some(Ok(())));
    assert_eq!(engine.poll_module(&module), Some(Ok(())));
}

#[test]
fn polling_a_rejected_module_again_returns_the_same_error() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module(
            "rejected.mjs",
            "await Promise.resolve(); throw new Error('again');",
        )
        .unwrap();

    let first = engine.poll_module(&module).unwrap().unwrap_err();
    let second = engine.poll_module(&module).unwrap().unwrap_err();
    assert_eq!(first.message(), "Error: again");
    assert_eq!(first, second);
}

#[test]
fn a_module_from_another_engine_is_an_error() {
    let other = Engine::new().unwrap();
    let module = other
        .start_module("foreign.mjs", "await new Promise(() => {});")
        .unwrap();
    let engine = Engine::new().unwrap();
    let _own = engine
        .start_module("own.mjs", "await new Promise(() => {});")
        .unwrap();

    let err = engine.poll_module(&module).unwrap().unwrap_err();
    assert!(!err.message().is_empty());
    assert_eq!(err.stack(), None);
}

#[test]
fn a_settled_rejection_is_also_returned_by_run_jobs() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module(
            "settled.mjs",
            "await Promise.resolve(); throw new Error('settled');",
        )
        .unwrap();

    let err = engine.poll_module(&module).unwrap().unwrap_err();
    let failures = engine.run_jobs();
    assert_eq!(failures, vec![err]);
    assert!(engine.run_jobs().is_empty());
}

#[test]
fn take_startup_failures_returns_the_jobs_that_threw_before_the_module_rejected() {
    let engine = Engine::new().unwrap();
    let module = engine
        .start_module(
            "taken.mjs",
            r"
            queueMicrotask(() => { throw new Error('side'); });
            await Promise.resolve(); throw new Error('entry');
            ",
        )
        .unwrap();

    let err = engine.poll_module(&module).unwrap().unwrap_err();
    let taken = engine.take_startup_failures();
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].message(), "Error: side");
    assert_eq!(err.message(), "Error: entry");
    assert!(engine.take_startup_failures().is_empty());
    assert_eq!(engine.run_jobs(), vec![err]);
}

#[test]
fn no_module_root_leaves_an_import_unresolved() {
    let engine = Engine::new().unwrap();
    let result = engine.eval_module("entry.js", "import './dep.js';");
    assert!(result.is_err());
}

#[test]
fn report_jobs_delivers_a_failed_job_once() {
    let engine = Engine::new().unwrap();
    engine
        .eval::<()>("Promise.resolve().then(() => { throw new Error('boom'); });")
        .unwrap();
    let reported: Rc<RefCell<Vec<EngineError>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&reported);
    let report: ErrorReporter = Rc::new(move |err| sink.borrow_mut().push(err.clone()));

    report_jobs(&engine, &report);
    assert_eq!(reported.borrow().len(), 1);
    assert_eq!(reported.borrow()[0].message(), "Error: boom");

    report_jobs(&engine, &report);
    assert_eq!(reported.borrow().len(), 1);
}
