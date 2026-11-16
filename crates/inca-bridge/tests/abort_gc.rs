// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests whose failure aborts the process: a JS value a signal or a controller does not trace
//! makes the engine's drop abort. Each test runs its body in a child process of this binary, so an
//! abort fails that one test.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use inca_bridge::Host;
use inca_bridge::test_support::engine_for;

/// Set in the child process that runs a test's body.
const CHILD: &str = "INCA_ABORT_GC_CHILD";

/// Whether this process is the child that runs a test's body.
fn in_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

/// Runs the test `name` of this binary in a child process and fails unless the child exits with
/// success after running exactly that one test.
fn run_in_child(name: &str) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Builds the signals of `setup`, then drops the engine.
fn drop_after(setup: &str) {
    let host = Rc::new(RefCell::new(Host::default()));
    let engine = engine_for(&host);
    engine.eval::<()>(setup).unwrap();
    drop(engine);
}

#[test]
fn the_engine_drops_with_a_live_controller_and_signal() {
    if !in_child() {
        return run_in_child("the_engine_drops_with_a_live_controller_and_signal");
    }
    drop_after(
        r"
        globalThis.kept = new AbortController();
        globalThis.timer = AbortSignal.timeout(1000);
        globalThis.dependent = AbortSignal.any([kept.signal]);
        __inca_native__.addEventListener(dependent, 'abort', () => {});
        ",
    );
}

#[test]
fn the_engine_drops_with_a_reason_that_refers_to_its_signal() {
    if !in_child() {
        return run_in_child("the_engine_drops_with_a_reason_that_refers_to_its_signal");
    }
    drop_after(
        r"
        globalThis.aborted = new AbortController();
        aborted.abort({ signal: aborted.signal });
        globalThis.cycle = AbortSignal.abort({});
        cycle.reason.signal = cycle;
        ",
    );
}

#[test]
fn the_engine_drops_with_a_controller_cycle_that_is_not_aborted() {
    if !in_child() {
        return run_in_child("the_engine_drops_with_a_controller_cycle_that_is_not_aborted");
    }
    drop_after(
        r"
        const c = new AbortController();
        c.signal.owner = c;
        c.owner = c.signal;
        ",
    );
}
