// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The timer queue behind the Web timer functions.
//!
//! Rust keeps ids, due times and periods. What each id runs lives in the
//! [callback table](crate::callbacks), so no JS value outlives the engine.

mod globals;
mod queue;

pub use globals::install;
pub use queue::{Clock, Timers, run_timers, system_clock};
