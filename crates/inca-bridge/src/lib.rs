// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds a `QuickJS` realm ([`inca_jsenv::Engine`]) to `inca`'s retained
//! virtual tree ([`inca_gpui::VirtualTree`]): the native bridge JS-side
//! renderers drive to build and mutate it, and the event dispatch that
//! carries native input back into JS.

pub mod bindings;
pub mod dispatch;
pub mod focus;

pub use bindings::{EventListeners, Host};
pub use dispatch::{ErrorReporter, EventDispatcher, drain_jobs_and_refresh, stderr_reporter};
pub use focus::FocusRegistry;
