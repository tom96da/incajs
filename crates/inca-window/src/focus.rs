// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persists GPUI's per-node focus handles across frames (`FocusHandle`s
//! don't come from the retained tree, so nothing else keeps them alive)
//! and turns GPUI's own focus changes into the four focus events.
//!
//! The focus model of the document decides which node holds focus and where Tab goes, and plans
//! each frame and each Tab press. The window applies the plan: it creates and drops the handle of
//! each node the plan names and moves and parks its own focus.

mod registry;
mod requests;
mod tab;

pub use registry::{FocusRegistry, dispatch_transition};
