// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The install of `globalThis.__inca_native__` and of the globals and prototypes the bridge defines.

use std::rc::Rc;

use rquickjs::function::IntoJsFunc;
use rquickjs::{Ctx, Function, Object, Result as JsResult};

use super::Host;
use super::members::install_members;

/// Installs `globalThis.__inca_native__` into `ctx`, wired to `host`. The global is read-only and
/// non-configurable, so a script can neither replace nor delete it. `addEventListener` keeps each
/// listener function in the callback table of the engine. The timer functions `setTimeout`,
/// `setInterval`, `clearTimeout` and `clearInterval` run over the timers of `host`. A timer runs
/// when the window runs the due timers through [`crate::JsScript`]; without a window the timer
/// stays queued. `requestAnimationFrame` and `cancelAnimationFrame` run over the frame callbacks
/// of `host`, which the window runs in its frames through [`crate::JsScript`].
///
/// # Errors
///
/// Returns an error if defining a global or a method on `ctx` fails, or the
/// realm has no vault (a context no `Engine` made). A second call on the same
/// `ctx` fails, as the globals are already defined.
pub fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let native = Object::new(ctx.clone())?;
    let natives = Natives {
        ctx,
        native: &native,
        host,
    };
    crate::node::install_tree(&natives)?;
    crate::element::install_tree(&natives)?;
    crate::node::install_mutation(&natives)?;
    crate::node::install_insert_nodes(&natives)?;
    crate::element::install_html(&natives)?;
    crate::node::install_node(&natives)?;
    crate::token_list::install_token_list(&natives)?;
    crate::inline_style::install_style(&natives)?;
    crate::dataset::install_dataset(&natives)?;
    crate::element::install_aria(&natives)?;
    crate::event::install_events(&natives)?;
    crate::element::install_focus(&natives)?;
    crate::element::install_capture(&natives)?;
    crate::element::install_scroll(&natives)?;
    crate::element::install_selector(&natives)?;
    crate::element::install_input(&natives)?;
    crate::node::install_handle(&natives)?;
    natives.set("interfacePrototypes", |_host| {
        crate::interfaces::interface_prototypes()
    })?;
    natives.set("signalId", |_host| crate::abort::signal_id_native)?;
    natives.set("registerUncloneable", |_host| {
        crate::interfaces::register_uncloneable()
    })?;
    inca_jsenv::timers::install(ctx, &host.js.borrow().timers)?;
    inca_jsenv::frames::install(ctx, &host.js.borrow().frames)?;
    crate::event::install(ctx)?;
    crate::interfaces::install(ctx)?;
    install_members(ctx, host)?;
    crate::abort::install(ctx, host)?;
    let globals = ctx.globals();
    globals.prop("__inca_native__", native)
}

/// What every native binding needs while it is registered.
pub(crate) struct Natives<'a, 'js> {
    pub(crate) ctx: &'a Ctx<'js>,
    native: &'a Object<'js>,
    pub(crate) host: &'a Rc<Host>,
}

impl<'js> Natives<'_, 'js> {
    /// Sets the native `name`, built by `make` from a clone of the host.
    pub(crate) fn set<P, F>(&self, name: &str, make: impl FnOnce(Rc<Host>) -> F) -> JsResult<()>
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
