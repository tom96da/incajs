// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs an inca app: reads its prebuilt bundle and config, then mounts the
//! bundle in a GPUI window.

mod app;
pub mod config;
mod dev;
mod menu;
mod protocol;

#[cfg(any(test, feature = "test-support"))]
pub mod harness;
#[cfg(any(test, feature = "test-support"))]
pub mod snapshot;

pub use app::{bundle_beside_exe, run_bundle};
