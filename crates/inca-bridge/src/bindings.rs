// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, dispatch events from script, read and
//! write the text of an input, and free what it no longer needs. It defines
//! the timer and frame callback functions of the global object.

pub(crate) mod convert;
mod host;
mod install;
pub(crate) mod members;

pub use host::{Host, JsState};
pub(crate) use install::Natives;
pub use install::install;
