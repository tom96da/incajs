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
//! [`Timers`] queues the timers a script starts, and [`timers::install`] defines the Web timer
//! functions over it. [`Frames`] queues the callbacks of `requestAnimationFrame`, and
//! [`frames::install`] defines it and `cancelAnimationFrame`. [`callbacks`] keeps the functions a
//! script registered under ids.
//!
//! The public [`class`] module holds the steps that define a Web global: a native class with its
//! constructor, methods, getters and global, a free function and a namespace object. A function,
//! class or namespace that a Web standard defines goes through it. An internal object, such as the
//! vault, may use `set` and `prop`.
//!
//! Each `install` function takes `ctx` first, then the shared state by reference (`&Rc<Timers>`,
//! `&Clock`, `&Output`), then plain options by value. It is `pub` when another crate calls it and
//! `pub(crate)` otherwise. It names the lifetime as `Ctx<'js>` when a closure that captures state
//! needs the name and as `Ctx<'_>` otherwise. Its doc has an `# Errors` section.
//!
//! [`Engine`] installs `Performance`, `performance`, `TextEncoder`, `TextDecoder`, `URL` and
//! `URLSearchParams`. The decoder reads every encoding of the Encoding Standard. The caller
//! installs `console`, the timers and the frame callbacks.
//!
//! [`Limits`] bounds the memory, the stack and the time a script may use.
//!
//! The renderer, windows and processes are outside it, so what it installs can
//! be replaced without reaching the code that draws a UI.

pub mod callbacks;
pub mod class;
pub mod console;
mod encoding;
mod engine;
pub mod frames;

mod compile;
mod inspect;
mod limits;
mod loader;
mod paint;
mod quickjs;
mod sourcemap;
#[cfg(test)]
mod tests;
mod time;
pub mod timers;
mod url;
mod vault;

pub use compile::compile_async_function;
pub use engine::{
    Engine, EngineBuilder, EngineError, EngineResult, ErrorReporter, PendingModule, report_jobs,
    stderr_reporter,
};
pub use frames::{Frames, run_frames};
pub use limits::Limits;
pub use quickjs::FunctionAccessor;
pub use sourcemap::StackPaths;
pub use time::now;
pub use timers::{Clock, Timers, run_timers, system_clock};
pub use vault::vault;
