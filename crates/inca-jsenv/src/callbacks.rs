// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The callback table: the functions a script registered, kept in the vault under ids.

use rquickjs::{Ctx, Object, Result as JsResult, Value};

use crate::vault::vault;

/// The vault key of the table.
const TABLE: &str = "callbacks";
/// The vault key of the id the next [`store`] hands out.
const NEXT: &str = "nextCallback";
/// The last id [`store`] returns. A script reads an id as `id | 0`, which keeps every id up to
/// this one.
pub const LAST_ID: u32 = 0x7fff_ffff;

/// Creates the empty table and starts the ids at 1.
///
/// # Errors
///
/// Returns an error if the realm has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let table = Object::new(ctx.clone())?;
    table.set_prototype(None)?;
    let vault = vault(ctx)?;
    vault.set(TABLE, table)?;
    vault.set(NEXT, 1)
}

/// Sets the allocator so that the next [`store`] finds no id left.
///
/// # Errors
///
/// Returns an error if the realm has no vault.
#[cfg(any(test, feature = "test-support"))]
pub fn exhaust_ids(ctx: &Ctx<'_>) -> JsResult<()> {
    vault(ctx)?.set(NEXT, LAST_ID + 1)
}

/// Stores `value` under the next id and returns it. Ids start at 1 and are never reused in one
/// context. `None` after the id [`LAST_ID`].
///
/// # Errors
///
/// Returns an error if the realm has no vault.
pub fn store<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Option<u32>> {
    let vault = vault(ctx)?;
    let next: i64 = vault.get(NEXT)?;
    let Some(id) = u32::try_from(next).ok().filter(|&id| id <= LAST_ID) else {
        return Ok(None);
    };
    vault.set(NEXT, next + 1)?;
    vault.get::<_, Object>(TABLE)?.set(id, value)?;
    Ok(Some(id))
}

/// The value stored under `id`, or `None` for an id that was never stored or was released.
///
/// # Errors
///
/// Returns an error if the realm has no vault.
pub fn get<'js>(ctx: &Ctx<'js>, id: u32) -> JsResult<Option<Value<'js>>> {
    let value: Value = vault(ctx)?.get::<_, Object>(TABLE)?.get(id)?;
    Ok((!value.is_undefined()).then_some(value))
}

/// Drops the values stored under `ids`. An id nothing holds does nothing.
///
/// # Errors
///
/// Returns an error if the realm has no vault.
pub fn release(ctx: &Ctx<'_>, ids: &[u32]) -> JsResult<()> {
    let table: Object = vault(ctx)?.get(TABLE)?;
    ids.iter().try_for_each(|&id| table.remove(id))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::Engine;

    /// The id `store` returns for a number.
    fn stored(engine: &Engine, n: i32) -> Option<u32> {
        engine.with(|ctx| store(&ctx, Value::new_int(ctx.clone(), n)).unwrap())
    }

    #[test]
    fn ids_rise_from_one_and_are_unique() {
        let engine = Engine::new().unwrap();

        let ids = [stored(&engine, 0), stored(&engine, 0), stored(&engine, 0)];

        assert_eq!(ids, [Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn a_released_id_returns_nothing() {
        let engine = Engine::new().unwrap();
        let id = stored(&engine, 7).unwrap();

        engine.with(|ctx| release(&ctx, &[id]).unwrap());

        assert!(engine.with(|ctx| get(&ctx, id).unwrap().is_none()));
    }

    #[test]
    fn a_released_id_is_not_reused() {
        let engine = Engine::new().unwrap();
        let id = stored(&engine, 7).unwrap();

        engine.with(|ctx| release(&ctx, &[id]).unwrap());

        assert_eq!(stored(&engine, 8), Some(id + 1));
    }

    #[test]
    fn releasing_an_unknown_id_does_nothing() {
        let engine = Engine::new().unwrap();
        let id = stored(&engine, 7).unwrap();

        engine.with(|ctx| release(&ctx, &[99]).unwrap());

        assert!(engine.with(|ctx| get(&ctx, id).unwrap().is_some()));
    }

    #[test]
    fn a_stored_value_comes_back() {
        let engine = Engine::new().unwrap();
        let id = stored(&engine, 7).unwrap();

        assert_eq!(
            engine.with(|ctx| get(&ctx, id).unwrap().unwrap().as_int()),
            Some(7)
        );
    }

    #[test]
    fn the_last_id_is_stored() {
        let engine = Engine::new().unwrap();
        engine.with(|ctx| vault(&ctx).unwrap().set(NEXT, LAST_ID).unwrap());

        assert_eq!(stored(&engine, 1), Some(LAST_ID));
    }

    #[test]
    fn store_after_the_last_id_returns_none() {
        let engine = Engine::new().unwrap();
        engine.with(|ctx| vault(&ctx).unwrap().set(NEXT, LAST_ID + 1).unwrap());

        assert_eq!(stored(&engine, 2), None);
    }

    #[test]
    fn store_after_the_last_id_stores_nothing() {
        let engine = Engine::new().unwrap();
        engine.with(|ctx| vault(&ctx).unwrap().set(NEXT, LAST_ID + 1).unwrap());
        stored(&engine, 2);

        let entries: u32 = engine.with(|ctx| {
            let table: Object = vault(&ctx).unwrap().get(TABLE).unwrap();
            table.keys::<String>().count().try_into().unwrap()
        });
        assert_eq!(entries, 0);
    }

    #[test]
    fn no_global_names_the_table() {
        let engine = Engine::new().unwrap();
        stored(&engine, 7);

        let reachable: bool = engine
            .eval(
                r"
                Object.getOwnPropertyNames(globalThis).some((name) => {
                    const value = globalThis[name];
                    return value !== null && typeof value === 'object' && value[1] === 7;
                });
                ",
            )
            .unwrap();

        assert!(!reachable);
    }

    #[test]
    fn a_new_engine_starts_at_one() {
        let first = Engine::new().unwrap();
        stored(&first, 0);
        stored(&first, 0);
        let second = Engine::new().unwrap();

        assert_eq!(stored(&second, 0), Some(1));
    }
}
