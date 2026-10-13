// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, dispatch events from script, and free what
//! it no longer needs.

mod convert;
mod focus;
mod host;
mod listeners;
mod scroll;
mod tree;

pub use host::Host;
pub use listeners::{EventListeners, Listener};

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::function::IntoJsFunc;
use rquickjs::{Ctx, Function, Object, Result as JsResult};

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
    let natives = Natives {
        ctx,
        native: &native,
        host,
    };
    install_tree(&natives)?;
    install_events(&natives)?;
    install_focus(&natives)?;
    install_scroll(&natives)?;
    crate::event::install(ctx)?;
    ctx.globals().set("__inca_native__", native)?;
    Ok(())
}

/// What every native binding needs while it is registered.
pub(super) struct Natives<'a, 'js> {
    ctx: &'a Ctx<'js>,
    native: &'a Object<'js>,
    host: &'a Rc<RefCell<Host>>,
}

impl<'js> Natives<'_, 'js> {
    /// Sets the native `name`, built by `make` from a clone of the host.
    fn set<P, F>(&self, name: &str, make: impl FnOnce(Rc<RefCell<Host>>) -> F) -> JsResult<()>
    where
        F: IntoJsFunc<'js, P> + 'js,
    {
        self.native.set(
            name,
            Function::new(self.ctx.clone(), make(Rc::clone(self.host)))?,
        )
    }
}
