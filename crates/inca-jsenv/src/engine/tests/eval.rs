// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

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
fn an_engine_without_stack_paths_names_files_in_full() {
    let engine = Engine::new().unwrap();
    let err = engine
        .eval_module("/app/dist/probe.mjs", "throw new Error('x');")
        .unwrap_err();

    let stack = err.stack().unwrap();
    assert!(stack.contains("/app/dist/probe.mjs:1:"), "{stack}");
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
