// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `globalThis.console` object: [`install`] defines it, and [`Output`]
//! receives what it prints.

mod assert;
mod counters;
mod format;
mod format_number;
mod object;
mod state;
mod table;

pub use object::{Level, Output, install};
