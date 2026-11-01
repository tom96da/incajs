// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use std::fs;

use rstest::rstest;

use super::*;
use crate::tests::ScratchDir;

mod eval;
mod jobs;
mod limits;

/// The counters a leak moves, after a gc run.
pub(crate) fn counters(engine: &Engine) -> [i64; 5] {
    engine.runtime.run_gc();
    let usage = engine.runtime.memory_usage();
    [
        usage.atom_count,
        usage.str_count,
        usage.obj_count,
        usage.malloc_count,
        usage.memory_used_count,
    ]
}

/// Calls `each` once to warm up, then 10 000 times, and returns the counters before and after.
pub(crate) fn around(engine: &Engine, each: impl Fn(&Ctx<'_>)) -> ([i64; 5], [i64; 5]) {
    engine.with(|ctx| each(&ctx));
    let before = counters(engine);
    engine.with(|ctx| {
        for _ in 0..10_000 {
            each(&ctx);
        }
    });
    (before, counters(engine))
}
