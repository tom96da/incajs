// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use std::fs;

use super::*;
use crate::tests::ScratchDir;

#[test]
fn a_syntax_error_is_returned_as_an_err() {
    let engine = Engine::new().unwrap();
    let result: EngineResult<i32> = engine.eval("1 +");
    assert!(result.is_err());
}

#[test]
fn two_engines_do_not_share_globals() {
    let a = Engine::new().unwrap();
    let b = Engine::new().unwrap();

    a.eval::<()>("globalThis.probe = 42;").unwrap();

    let seen_by_a: i32 = a.eval("probe").unwrap();
    assert_eq!(seen_by_a, 42);

    let seen_by_b: String = b.eval("typeof probe").unwrap();
    assert_eq!(seen_by_b, "undefined");
}

#[test]
fn eval_module_runs_top_level_code_to_completion() {
    let engine = Engine::new().unwrap();
    engine
        .eval_module(
            "probe.mjs",
            "export const answer = 41; globalThis.seen = answer + 1;",
        )
        .unwrap();

    let seen: i32 = engine.eval("globalThis.seen").unwrap();
    assert_eq!(seen, 42);
}

#[test]
fn an_error_gives_up_its_message_and_its_frames() {
    let engine = Engine::new().unwrap();
    let err = engine
        .eval_module("throws.mjs", "throw new Error('bad edit');")
        .unwrap_err();

    assert_eq!(err.message(), "Error: bad edit");
    assert!(
        err.stack().unwrap_or_default().contains("at "),
        "{:?}",
        err.stack()
    );
}

#[test]
fn a_frame_of_a_compiled_function_reads_at_its_source() {
    let engine = Engine::new().unwrap();
    let body = crate::sourcemap::mapped_body("throw new Error('bad edit');");
    engine.with(|ctx| {
        let f = crate::compile_async_function(&ctx, &[], &body, "virtual.js").unwrap();
        ctx.globals().set("f", f).unwrap();
    });
    engine.eval::<()>("f();").unwrap();
    let errors = engine.run_jobs();

    let stack = errors[0].stack().unwrap();
    assert!(stack.contains("src/app.ts:12:5"), "{stack}");
    assert!(!stack.contains("virtual.js:"), "{stack}");
}

#[test]
fn a_frame_of_a_module_reads_through_its_sidecar_map() {
    let dir = ScratchDir::new("module-sidecar-map");
    fs::write(
        dir.0.join("bundle.js.map"),
        r#"{"version":3,"sources":["../src/app.ts"],"mappings":";AAWI"}"#,
    )
    .unwrap();
    let entry = dir.0.join("bundle.js");
    let source = "globalThis.ready = true;\nthrow new Error('bad edit');\n//# sourceMappingURL=bundle.js.map";

    let engine = Engine::new().unwrap();
    let err = engine
        .eval_module(&entry.to_string_lossy(), source)
        .unwrap_err();

    let stack = err.stack().unwrap();
    assert!(stack.contains("src/app.ts:12:5"), "{stack}");
}

#[test]
fn a_thrown_non_error_still_reads_as_something() {
    let engine = Engine::new().unwrap();
    let err = engine.eval::<()>("throw 'just a string';").unwrap_err();

    assert_eq!(err.message(), "just a string");
    assert_eq!(err.stack(), None, "only an Error carries frames");
}

#[test]
fn a_conversion_failure_is_not_a_thrown_value() {
    let engine = Engine::new().unwrap();
    let err = engine.eval::<i32>("'not a number'").unwrap_err();

    assert!(!err.message().is_empty());
    assert_eq!(err.stack(), None);
}

#[test]
fn display_leads_with_the_message() {
    let engine = Engine::new().unwrap();
    let err = engine.eval::<()>("throw new Error('boom');").unwrap_err();

    let shown = err.to_string();
    assert!(shown.starts_with("Error: boom"), "{shown}");
}

#[test]
fn reading_one_failure_does_not_leak_into_the_next() {
    let engine = Engine::new().unwrap();

    let first = engine.eval::<()>("throw new Error('first');").unwrap_err();
    let second = engine.eval::<()>("throw new Error('second');").unwrap_err();

    assert_eq!(first.message(), "Error: first");
    assert_eq!(second.message(), "Error: second");
}

#[test]
fn a_shared_dependency_is_evaluated_once_across_two_entries() {
    let dir = ScratchDir::new("shared-dependency");
    fs::write(
        dir.0.join("counter.js"),
        "let calls = 0; export function next() { calls += 1; return calls; }",
    )
    .unwrap();
    let first = dir.0.join("first.js");
    fs::write(
        &first,
        "import { next } from './counter.js'; globalThis.firstSeen = next();",
    )
    .unwrap();
    let second = dir.0.join("second.js");
    fs::write(
        &second,
        "import { next } from './counter.js'; globalThis.secondSeen = next();",
    )
    .unwrap();

    let engine = Engine::builder().module_root(&dir.0).build().unwrap();
    engine
        .eval_module(
            &first.to_string_lossy(),
            &fs::read_to_string(&first).unwrap(),
        )
        .unwrap();
    engine
        .eval_module(
            &second.to_string_lossy(),
            &fs::read_to_string(&second).unwrap(),
        )
        .unwrap();

    // Both entries import the same `counter.js`. One shared module
    // instance means its call counter keeps incrementing, so the second entry
    // sees `2`.
    let first_seen: i32 = engine.eval("globalThis.firstSeen").unwrap();
    let second_seen: i32 = engine.eval("globalThis.secondSeen").unwrap();
    assert_eq!((first_seen, second_seen), (1, 2));
}

#[test]
fn a_shared_dependency_reached_by_two_different_relative_paths_is_one_module() {
    let dir = ScratchDir::new("shared-dependency-path-variants");
    fs::write(
        dir.0.join("shared.js"),
        "let calls = 0; export function next() { calls += 1; return calls; }",
    )
    .unwrap();

    let a_dir = dir.0.join("a");
    fs::create_dir_all(&a_dir).unwrap();
    let first = a_dir.join("x.js");
    fs::write(
        &first,
        "import { next } from '../shared.js'; globalThis.firstSeen = next();",
    )
    .unwrap();

    let b_dir = dir.0.join("b");
    fs::create_dir_all(&b_dir).unwrap();
    let second = b_dir.join("y.js");
    fs::write(
        &second,
        "import { next } from '../shared.js'; globalThis.secondSeen = next();",
    )
    .unwrap();

    let engine = Engine::builder().module_root(&dir.0).build().unwrap();
    engine
        .eval_module(
            &first.to_string_lossy(),
            &fs::read_to_string(&first).unwrap(),
        )
        .unwrap();
    engine
        .eval_module(
            &second.to_string_lossy(),
            &fs::read_to_string(&second).unwrap(),
        )
        .unwrap();

    // `a/x.js` and `b/y.js` both resolve `../shared.js` to the same
    // file, but through textually different paths (`a/../shared.js`
    // vs. `b/../shared.js`). Canonicalizing before caching means one
    // module instance, so the second entry sees `2`, not `1`.
    let first_seen: i32 = engine.eval("globalThis.firstSeen").unwrap();
    let second_seen: i32 = engine.eval("globalThis.secondSeen").unwrap();
    assert_eq!((first_seen, second_seen), (1, 2));
}

#[test]
fn a_module_importing_its_entry_back_reaches_the_entry_itself() {
    let dir = ScratchDir::new("import-entry-back");
    fs::create_dir_all(dir.0.join("sub")).unwrap();
    let entry = dir.0.join("sub/../entry.js");
    let source = "globalThis.runs = (globalThis.runs ?? 0) + 1; import('./back.js');";
    fs::write(dir.0.join("entry.js"), source).unwrap();
    fs::write(dir.0.join("back.js"), "import './entry.js';").unwrap();

    let engine = Engine::builder().module_root(&dir.0).build().unwrap();
    engine
        .eval_module(&entry.to_string_lossy(), source)
        .unwrap();
    let _ = engine.run_jobs();

    let runs: i32 = engine.eval("globalThis.runs").unwrap();
    assert_eq!(runs, 1);
}

#[test]
fn a_dynamic_import_settles_once_pending_jobs_are_drained() {
    let dir = ScratchDir::new("dynamic-import");
    fs::write(dir.0.join("dep.js"), "export const value = 41;").unwrap();
    let entry = dir.0.join("entry.js");
    fs::write(
        &entry,
        "import('./dep.js').then((m) => { globalThis.seen = m.value + 1; });",
    )
    .unwrap();

    let engine = Engine::builder().module_root(&dir.0).build().unwrap();
    engine
        .eval_module(
            &entry.to_string_lossy(),
            &fs::read_to_string(&entry).unwrap(),
        )
        .unwrap();
    let _ = engine.run_jobs();

    let seen: i32 = engine.eval("globalThis.seen").unwrap();
    assert_eq!(seen, 42);
}

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
