// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_native__`: the small set of native functions
//! JS calls to read the root handle, mutate the retained virtual tree,
//! register input-event callbacks, dispatch events from script, and free what
//! it no longer needs. It also creates the empty `globalThis.__inca_callbacks__`,
//! where JS stores each listener under its callback id.

pub(crate) mod convert;
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

/// Installs `globalThis.__inca_native__` into `ctx`, wired to `host`, and an
/// empty null-prototype `globalThis.__inca_callbacks__`. Both globals are
/// read-only and non-configurable, so a script can neither replace nor delete
/// them.
///
/// # Errors
///
/// Returns an error if defining a global or a method on `ctx` fails. A
/// second call on the same `ctx` fails, as the globals are already defined.
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
    let globals = ctx.globals();
    globals.prop("__inca_native__", native)?;
    let table = Object::new(ctx.clone())?;
    table.set_prototype(None)?;
    globals.prop("__inca_callbacks__", table)
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
    #[case::callbacks_assigned("__inca_callbacks__", "globalThis.__inca_callbacks__ = null;")]
    #[case::callbacks_deleted("__inca_callbacks__", "delete globalThis.__inca_callbacks__;")]
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
    fn the_callback_table_ignores_object_prototype_accessors() {
        let (engine, _host) = engine_with_bindings();

        let own: bool = engine
            .eval(
                r"
                Object.defineProperty(Object.prototype, 0, {
                    get() {
                        return 'inherited';
                    },
                    set(value) {
                        globalThis.caught = value;
                    },
                });
                globalThis.__inca_callbacks__[0] = 'own';
                globalThis.caught === undefined && globalThis.__inca_callbacks__[0] === 'own';
                ",
            )
            .unwrap();

        assert!(own);
    }

    #[test]
    fn a_second_install_on_one_context_fails() {
        let (engine, host) = engine_with_bindings();

        assert!(engine.with(|ctx| install(&ctx, &host)).is_err());
    }
}
