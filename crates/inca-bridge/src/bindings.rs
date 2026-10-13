// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, dispatch events from script, and free what
//! it no longer needs.

mod convert;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod fixtures;
mod focus;
mod host;
mod listeners;
mod scroll;
mod tree;

pub use host::Host;
pub use listeners::{EventListeners, Listener};

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Object, Result as JsResult};

use focus::install_focus;
use listeners::install_events;
use scroll::install_scroll;
use tree::install_tree;

/// Installs `globalThis.__inca_native__` into `ctx`, wired to `host`.
///
/// # Errors
///
/// Returns an error if defining `globalThis.__inca_native__` or any of its
/// methods on `ctx` fails.
pub fn install(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let native = Object::new(ctx.clone())?;
    install_tree(ctx, host, &native)?;
    install_events(ctx, host, &native)?;
    install_focus(ctx, host, &native)?;
    install_scroll(ctx, host, &native)?;
    crate::event::install(ctx)?;
    ctx.globals().set("__inca_native__", native)?;
    Ok(())
}
