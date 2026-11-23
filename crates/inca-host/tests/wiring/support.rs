// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scenes the wiring tests run on: a window over a document, with a JS engine as the script of
//! the dispatch.

pub mod events;
pub mod input;
pub mod keys;
pub mod pointer;
pub mod rig;
pub mod scroll;

pub use events::*;
pub use input::*;
pub use keys::*;
pub use rig::*;
