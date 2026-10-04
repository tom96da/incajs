// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs an inca app: reads its prebuilt bundle and config, then mounts the
//! bundle in a GPUI window.

mod app;
pub mod config;
mod dev;
mod menu;
mod protocol;

pub use app::{bundle_beside_exe, run_bundle};
