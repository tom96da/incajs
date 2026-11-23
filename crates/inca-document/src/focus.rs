// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which node holds focus, which focus and blur requests wait for the next frame, and where Tab
//! starts and moves.
//!
//! A focus or blur request can arrive while no window is at hand. The request is recorded here
//! and applied by [`FocusModel::apply_pending`], which the caller runs once per frame.
//!
//! Transitions come from comparing the node focused now with the node focused last frame. A
//! per-frame comparison sees a focus that moves in the same frame a listener is registered.

mod model;
mod recheck;
mod requests;
mod start;
mod tab;

use model::PendingFocus;
pub use model::{FocusModel, FocusSurface, FocusTransition};
use start::Start;
