// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `QuickJS` runtime bootstrap.
//!
//! An [`Engine`] pairs one `QuickJS` runtime with one execution context. Each
//! `Engine` is fully independent: creating a global on one has no effect on
//! any other `Engine`, since they don't share a runtime or a heap.

mod jobs;
mod runtime;

pub use jobs::{PendingModule, report_jobs};
pub use runtime::{
    Engine, EngineBuilder, EngineError, EngineResult, ErrorReporter, stderr_reporter,
};
