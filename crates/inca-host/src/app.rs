// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Loads an entry module (resolving its own `import`s against sibling
//! files on disk) and opens a GPUI window on whatever tree it mounts.
//! Production evaluates the entry to completion before the window opens. The
//! host starts it as a [`Loading`] for the development window, and
//! [`crate::dev`] polls it until it settles.
//!
//! One binary serves any app, so it cannot know whether a bundle registers
//! input handlers; it always renders through `EventDispatcher`, which tracks
//! the pointer once a listener is registered or a node can take focus.
//!
//! `--dev` additionally answers messages on stdin and writes them on
//! stdout, via [`crate::dev`] speaking the development wire format.
//! stdout is then the message channel and nothing else may go there, so
//! every diagnostic, and the app's own `console`, go to stderr.
//!
//! Nothing here panics on a failure a user can cause.
//!
//! The JS runtime, the bridge and GPUI's `App` run on the main thread. GPUI
//! builds the `App` there and holds it in an `Rc`.

mod bundle;
mod entry;
mod session;
mod size;
mod window;

pub use bundle::{BundleError, launcher_bundle};
pub use entry::{print_config, run_bundle};
pub(crate) use session::{Failure, Session, clock_of};
pub(crate) use size::maybe_auto_resize_to_content;
pub(crate) use window::{HostedApp, Loading, open, start};
