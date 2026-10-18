// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The vault: one null-prototype object per realm that host code keeps JS
//! values in between calls, under plain keys.
//!
//! The global [`VAULT`] holds a function that returns the vault only while
//! [`vault`] calls it. The function asks a native gate that only [`vault`]
//! opens, for the length of that one call. A script that calls the function
//! gets `undefined`, and it cannot replace or delete the global.

use std::cell::Cell;

use rquickjs::{Ctx, Error, Function, Object, Result as JsResult};

use crate::quickjs::ObjectExt;

/// The global that opens the vault.
const VAULT: &str = "__inca_vault__";

/// Makes the function [`VAULT`] holds from the native gate `allowed`.
const OPEN_JS: &str = r"
    (allowed) => {
        const vault = { __proto__: null };
        return () => (allowed() ? vault : undefined);
    }
    ";

thread_local! {
    /// Whether [`vault`] is calling the vault global on this thread.
    static OPENING: Cell<bool> = const { Cell::new(false) };
}

/// Defines the vault of `ctx`'s realm on a read-only, non-configurable
/// global. [`Engine`](crate::Engine) calls it before any script runs.
///
/// # Errors
///
/// Returns an error if the global cannot be defined, as on a second call.
pub(crate) fn create_vault(ctx: &Ctx<'_>) -> JsResult<()> {
    let allowed = Function::new(ctx.clone(), || OPENING.get())?;
    let make: Function = ctx.eval(OPEN_JS)?;
    let open: Function = make.call((allowed,))?;
    ctx.globals().prop(VAULT, open)
}

/// The vault of `ctx`'s realm: a null-prototype object that host code keeps JS
/// values in between calls, under plain keys. An [`Engine`](crate::Engine)
/// creates it before any script runs.
///
/// # Errors
///
/// Returns an error if the realm has no vault, as in a context that no
/// `Engine` created.
pub fn vault<'js>(ctx: &Ctx<'js>) -> JsResult<Object<'js>> {
    let open = ctx
        .globals()
        .get_own_property_descriptor(VAULT)?
        .and_then(|own| own.value.into_function())
        .ok_or_else(|| Error::new_from_js("undefined", "vault"))?;
    OPENING.set(true);
    let opened = open.call(());
    OPENING.set(false);
    opened
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use rquickjs::Value;
    use rstest::rstest;

    use super::*;
    use crate::tests::with_realm;

    #[test]
    fn the_vault_keeps_a_value_between_calls() {
        with_realm(|ctx| {
            create_vault(&ctx).unwrap();
            vault(&ctx).unwrap().set("kept", 7).unwrap();

            assert_eq!(vault(&ctx).unwrap().get::<_, i32>("kept").unwrap(), 7);
        });
    }

    #[test]
    fn a_script_calling_the_vault_global_gets_nothing() {
        with_realm(|ctx| {
            create_vault(&ctx).unwrap();
            vault(&ctx).unwrap();
            let got: Value<'_> = ctx.eval("globalThis.__inca_vault__(true)").unwrap();

            assert!(got.is_undefined());
        });
    }

    #[rstest]
    #[case::assigned("globalThis.__inca_vault__ = () => ({ kept: 0 });")]
    #[case::deleted("delete globalThis.__inca_vault__;")]
    fn a_script_cannot_replace_the_vault(#[case] change: &str) {
        with_realm(|ctx| {
            create_vault(&ctx).unwrap();
            vault(&ctx).unwrap().set("kept", 7).unwrap();
            ctx.eval::<(), _>(format!(
                r"
                try {{
                    {change}
                }} catch {{}}
                "
            ))
            .unwrap();

            assert_eq!(vault(&ctx).unwrap().get::<_, i32>("kept").unwrap(), 7);
        });
    }

    #[test]
    fn an_object_prototype_accessor_leaves_the_vault_alone() {
        with_realm(|ctx| {
            create_vault(&ctx).unwrap();
            ctx.eval::<(), _>(
                r"
                Object.defineProperty(Object.prototype, 'kept', {
                    get() {
                        return 0;
                    },
                    set(value) {},
                });
                ",
            )
            .unwrap();
            vault(&ctx).unwrap().set("kept", 7).unwrap();

            assert_eq!(vault(&ctx).unwrap().get::<_, i32>("kept").unwrap(), 7);
        });
    }

    #[test]
    fn an_inherited_vault_global_gives_an_error() {
        with_realm(|ctx| {
            ctx.eval::<(), _>("Object.prototype.__inca_vault__ = () => ({});")
                .unwrap();

            assert!(vault(&ctx).is_err());
        });
    }
}
