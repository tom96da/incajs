// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs an inca app: reads its prebuilt bundle and config, then mounts the
//! bundle in a GPUI window.

#[cfg(all(feature = "automation", not(debug_assertions)))]
compile_error!("the automation feature is for debug builds only");

mod app;
pub mod config;
mod console_output;
mod dev;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod display;
mod menu;
mod run_log;
pub mod script;
#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-support"))]
pub mod harness;
#[cfg(any(test, feature = "test-support", feature = "automation"))]
pub mod snapshot;
#[cfg(any(test, feature = "test-support"))]
pub mod wiring;

pub use app::{BundleError, launcher_bundle, print_config, run_bundle};
