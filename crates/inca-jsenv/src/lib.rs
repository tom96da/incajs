// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `QuickJS` runtime bootstrap, and the host objects installed into it.
//!
//! `QuickJS` supplies only the language (`Array`, `JSON`, `Promise` and the rest
//! of the ECMAScript intrinsics). Everything a script expects the *host* to
//! provide starts here. [`compile_async_function`] builds source text into an
//! async function whose stack frames name its file. [`vault`] keeps the JS
//! values host code needs in a later call out of reach of scripts.
//!
//! [`Timers`] queues the timers a script starts.
//!
//! [`Limits`] bounds the memory, the stack and the time a script may use.
//!
//! The renderer, windows and processes are outside it, so what it installs can
//! be replaced without reaching the code that draws a UI.

pub mod console;
mod engine;

mod compile;
mod inspect;
mod limits;
mod loader;
mod paint;
mod quickjs;
mod sourcemap;
#[cfg(test)]
mod tests;
mod timers;
mod vault;

pub use compile::compile_async_function;
pub use engine::{
    Engine, EngineBuilder, EngineError, EngineResult, ErrorReporter, PendingModule, report_jobs,
    stderr_reporter,
};
pub use limits::Limits;
pub use quickjs::FunctionAccessor;
pub use sourcemap::StackPaths;
pub use timers::{Clock, Timers, run_timers, system_clock};
pub use vault::vault;
