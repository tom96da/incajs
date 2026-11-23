// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `AbortController` and `AbortSignal`.
//!
//! Rust keeps the state record of each signal under its id. The signal objects live in JS and are
//! found by id through the signal table, a weak table in the vault. The abort steps remove nothing
//! outside the signal in this module. The `abort` event reaches the listeners of the signal
//! through the walk that serves nodes and `window`.

mod any;
mod classes;
mod statics;
mod steps;
mod table;

pub(crate) use classes::IncaSignal;
pub(crate) use steps::{
    abort_signal, after_removal, dom_exception, install, new_signal, release_signal, settle_freed,
    signal_id, signal_id_native,
};
pub(crate) use table::Table;
