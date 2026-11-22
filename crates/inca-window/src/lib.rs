// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The behaviour of a window: the input it receives turned into events and default actions, the
//! frame order, the schedule of timers and frame callbacks, and the state that lives with a
//! `gpui` window.

mod activation;
mod dispatch;
mod fire;
pub mod focus;
mod frame;
mod hover;
mod input;
mod keys;
mod pointer;
mod restyle;
mod script;
mod sink;
mod timers;
mod view;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(test)]
mod tests;

pub use dispatch::EventDispatcher;
pub use focus::{FocusRegistry, dispatch_transition};
pub use script::Script;
pub use view::{Cells, WindowState};
