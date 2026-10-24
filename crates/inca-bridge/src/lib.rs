// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds a `QuickJS` realm ([`inca_jsenv::Engine`]) to `inca`'s retained
//! virtual tree ([`inca_gpui::VirtualTree`]): the native bridge JS-side
//! renderers drive to build and mutate it, and the event dispatch that carries
//! native input back into JS.

mod bindings;
mod dispatch;
mod event;
mod event_target;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use bindings::{EventListeners, Host, Listener, install};
pub use dispatch::{EventDispatcher, drain_jobs_and_refresh};
pub use event_target::EventTargetId;
pub use focus::{FocusRegistry, FocusTransition};
pub use inca_gpui::focus;
