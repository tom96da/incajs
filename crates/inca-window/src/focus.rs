// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persists GPUI's per-node focus handles across frames (`FocusHandle`s
//! don't come from the retained tree, so nothing else keeps them alive)
//! and turns GPUI's own focus changes into the four focus events.
//!
//! The focus model of the document decides which node holds focus and where Tab goes. The
//! registry gives it the window side, a handle per node that can take focus. The model is the
//! caller's: every method that reads or moves it takes it as a parameter.

mod registry;
mod requests;
mod tab;

pub use registry::{FocusRegistry, dispatch_transition};
