// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writes the errors and panics of a packaged app to a file per run.

mod entry;
mod files;
mod writer;

pub(crate) use files::log_dir;
pub(crate) use writer::{error, install, start_secs};
