// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Drives a mounted app headlessly: reads its tree, sends input and waits
//! for it to go idle.

mod mounted;
mod pointer;
mod text;

pub use inca_gpui::Scrollbars;
pub use mounted::Harness;
