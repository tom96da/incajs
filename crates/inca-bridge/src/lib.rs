// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds a `QuickJS` realm ([`inca_jsenv::Engine`]) to `inca`'s retained
//! virtual tree ([`inca_node::VirtualTree`]): the native bridge JS-side
//! renderers drive to build and mutate it, and the turns of the engine that carry
//! events, timers and frame callbacks into JS.

mod abort;
mod bindings;
mod dataset;
mod element;
mod event;
mod inline_style;
mod interfaces;
mod node;
mod script;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(test)]
mod tests;
mod token_list;

pub use bindings::{Host, JsState, install};
pub use script::JsScript;
