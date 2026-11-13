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

use rquickjs::{Ctx, Error, Exception, Function, Object, Result as JsResult};

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

// A call into an engine and the native calls inside it run on one thread, so the flag tracks it.
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

/// The vault key of the prototype store: one prototype under each name.
const PROTOTYPES: &str = "prototypes";

/// Keeps `proto` in the prototype store of `ctx`'s realm under `name`. The first call creates the
/// store, a null-prototype object in the vault.
///
/// # Errors
///
/// Returns an error if the realm has no vault.
pub fn store_prototype<'js>(ctx: &Ctx<'js>, name: &str, proto: &Object<'js>) -> JsResult<()> {
    let vault = vault(ctx)?;
    if let Some(store) = vault.get::<_, Option<Object>>(PROTOTYPES)? {
        return store.set(name, proto.clone());
    }
    let store = Object::new(ctx.clone())?;
    store.set_prototype(None)?;
    vault.set(PROTOTYPES, store.clone())?;
    store.set(name, proto.clone())
}

/// The prototype kept under `name` in the prototype store, `None` if there is none.
///
/// # Errors
///
/// Returns an error if the realm has no vault.
pub fn stored_prototype<'js>(ctx: &Ctx<'js>, name: &str) -> JsResult<Option<Object<'js>>> {
    let Some(store) = vault(ctx)?.get::<_, Option<Object>>(PROTOTYPES)? else {
        return Ok(None);
    };
    store.get(name)
}

/// The prototype kept under `name` in the prototype store.
///
/// # Errors
///
/// Returns a `TypeError` if no prototype is stored under `name`, and an error if the realm has no
/// vault.
pub fn required_prototype<'js>(ctx: &Ctx<'js>, name: &str) -> JsResult<Object<'js>> {
    stored_prototype(ctx, name)?.ok_or_else(|| {
        Exception::throw_type(
            ctx,
            &format!("no prototype is stored for {name}. define the class first"),
        )
    })
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
    fn a_prototype_is_none_before_any_is_stored() {
        with_realm(|ctx| {
            create_vault(&ctx).unwrap();

            assert!(stored_prototype(&ctx, "Widget").unwrap().is_none());
        });
    }

    #[test]
    fn a_stored_prototype_is_returned() {
        with_realm(|ctx| {
            create_vault(&ctx).unwrap();
            let proto = Object::new(ctx.clone()).unwrap();
            proto.set("kept", 7).unwrap();
            store_prototype(&ctx, "Widget", &proto).unwrap();

            let got = stored_prototype(&ctx, "Widget").unwrap().unwrap();

            assert_eq!(got.get::<_, i32>("kept").unwrap(), 7);
        });
    }

    #[test]
    fn a_missing_prototype_throws_a_type_error() {
        with_realm(|ctx| {
            create_vault(&ctx).unwrap();

            assert!(required_prototype(&ctx, "Widget").is_err());
            let error = ctx.catch();
            let error = error.as_object().unwrap();

            assert_eq!(error.get::<_, String>("name").unwrap(), "TypeError");
            assert_eq!(
                error.get::<_, String>("message").unwrap(),
                "no prototype is stored for Widget. define the class first"
            );
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
