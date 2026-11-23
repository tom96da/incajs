// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which node holds focus, which focus and blur requests wait for the next frame, and where Tab
//! starts and moves.
//!
//! A focus or blur request can arrive while no window is at hand. The request is recorded here,
//! and [`FocusModel::plan_frame`], which the caller runs once per frame, turns it into a
//! [`FocusPlan`] the window applies.
//!
//! Transitions come from comparing the node focused now with the node focused last frame. A
//! per-frame comparison sees a focus that moves in the same frame a listener is registered.

mod model;
mod plan;
mod recheck;
mod requests;
mod start;
mod tab;

use model::PendingFocus;
pub use model::{FocusModel, FocusTransition};
pub(crate) use plan::FocusReveal;
pub use plan::{FocusPlan, FocusStep};
use start::Start;
