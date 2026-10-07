// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds a `QuickJS` realm ([`inca_jsenv::Engine`]) to `inca`'s retained
//! virtual tree ([`inca_gpui::VirtualTree`]): the native bridge JS-side
//! renderers drive to build and mutate it, the event dispatch that carries
//! native input back into JS, and the dev-only `__inca_dev__` channel
//! ([`dev`]) a bundler integration rides without this crate knowing its
//! name.

pub mod bindings;
pub mod dev;
pub mod dispatch;
pub mod focus;

pub use bindings::{EventListeners, Host};
pub use dev::{DevSend, call_dev_receive, install_dev};
pub use dispatch::{ErrorReporter, EventDispatcher, drain_jobs_and_refresh, stderr_reporter};
pub use focus::{FocusRegistry, FocusTransition};
