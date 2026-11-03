// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, dispatch events from script, read and
//! write the text of an input, and free what it no longer needs. It defines
//! the timer and frame callback functions of the global object.

mod attributes;
mod capture;
pub(crate) mod convert;
mod focus;
mod host;
mod input;
mod listeners;
mod scroll;
mod selector;
mod tree;

pub use host::Host;
pub use listeners::{EventListeners, Listener};

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::function::IntoJsFunc;
use rquickjs::{Ctx, Function, Object, Result as JsResult};

use capture::install_capture;
use focus::install_focus;
use input::install_input;
use listeners::install_events;
use scroll::install_scroll;
use selector::install_selector;
use tree::install_tree;

/// Installs `globalThis.__inca_native__` into `ctx`, wired to `host`. The global is read-only and
/// non-configurable, so a script can neither replace nor delete it. `addEventListener` keeps each
/// listener function in the callback table of the engine. The timer functions `setTimeout`,
/// `setInterval`, `clearTimeout` and `clearInterval` run over the timers of `host`. A timer runs
/// only after [`crate::EventDispatcher::start_timers`] or
/// [`crate::EventDispatcher::run_timers_idle`] runs; without one the timer
/// stays queued. `requestAnimationFrame` and `cancelAnimationFrame` run over the frame callbacks
/// of `host`, which run in the frames of the window that
/// [`crate::EventDispatcher::start_timers`] serves.
///
/// # Errors
///
/// Returns an error if defining a global or a method on `ctx` fails, or the
/// realm has no vault (a context no `Engine` made). A second call on the same
/// `ctx` fails, as the globals are already defined.
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
    install_capture(&natives)?;
    install_scroll(&natives)?;
    install_selector(&natives)?;
    install_input(&natives)?;
    inca_jsenv::timers::install(ctx, &host.borrow().timers)?;
    inca_jsenv::frames::install(ctx, &host.borrow().frames)?;
    crate::event::install(ctx)?;
    let globals = ctx.globals();
    globals.prop("__inca_native__", native)
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::install;
    use crate::test_support::engine_with_bindings;

    #[rstest]
    #[case::native_assigned("__inca_native__", "globalThis.__inca_native__ = null;")]
    #[case::native_deleted("__inca_native__", "delete globalThis.__inca_native__;")]
    fn a_script_cannot_replace_a_bridge_global(#[case] name: &str, #[case] change: &str) {
        let (engine, _host) = engine_with_bindings();

        let kept: bool = engine
            .eval(&format!(
                r"
                const before = globalThis.{name};
                try {{
                    {change}
                }} catch {{}}
                globalThis.{name} === before;
                "
            ))
            .unwrap();

        assert!(kept, "{name}: {change}");
    }

    #[test]
    fn a_second_install_on_one_context_fails() {
        let (engine, host) = engine_with_bindings();

        assert!(engine.with(|ctx| install(&ctx, &host)).is_err());
    }
}
