// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The signal table: the weak references to the signal objects and the strong references that keep
//! a dependent signal alive, kept in the vault.

use inca_jsenv::vault;
use rquickjs::{Ctx, Function, Object, Result as JsResult, Value};

/// The vault key of the table.
const KEY: &str = "abort";

/// Makes the table. The functions bind the built-ins once, so no call of the table reads a
/// property that a script can replace.
const TABLE_JS: &str = r"
    (onCollect) => {
        const apply = Reflect.apply;
        const mapGet = Map.prototype.get;
        const mapSet = Map.prototype.set;
        const mapDelete = Map.prototype.delete;
        const deref = WeakRef.prototype.deref;
        const register = FinalizationRegistry.prototype.register;
        const Weak = WeakRef;
        const refs = new Map();
        const pinned = new Map();
        const registry = new FinalizationRegistry(onCollect);
        const find = (id) => {
            const ref = apply(mapGet, refs, [id]);
            return ref === undefined ? undefined : apply(deref, ref, []);
        };
        return {
            __proto__: null,
            track: (object, id) => {
                apply(mapSet, refs, [id, new Weak(object)]);
                apply(register, registry, [object, id]);
            },
            find,
            forget: (id) => {
                apply(mapDelete, refs, [id]);
                apply(mapDelete, pinned, [id]);
            },
            pin: (id) => {
                const object = find(id);
                if (object !== undefined) apply(mapSet, pinned, [id, object]);
            },
            unpin: (id) => {
                apply(mapDelete, pinned, [id]);
            },
        };
    }
    ";

/// The signal table of a realm.
pub(crate) struct Table<'js>(Object<'js>);

impl<'js> Table<'js> {
    /// Makes the table and keeps it in the vault. `on_collect` runs with the id of a signal
    /// object the engine collected.
    pub(super) fn create(ctx: &Ctx<'js>, on_collect: Function<'js>) -> JsResult<()> {
        let make: Function = ctx.eval(TABLE_JS)?;
        let table: Object = make.call((on_collect,))?;
        vault(ctx)?.set(KEY, table)
    }

    /// The table of `ctx`'s realm.
    pub(crate) fn open(ctx: &Ctx<'js>) -> JsResult<Self> {
        vault(ctx)?.get(KEY).map(Self)
    }

    fn function(&self, name: &str) -> JsResult<Function<'js>> {
        self.0.get(name)
    }

    /// Records `object` as the signal `id`, held weakly.
    pub(crate) fn track(&self, object: &Object<'js>, id: u32) -> JsResult<()> {
        self.function("track")?.call((object.clone(), id))
    }

    /// The object of signal `id`, `None` once it is collected.
    pub(crate) fn find(&self, id: u32) -> JsResult<Option<Object<'js>>> {
        let found: Value = self.function("find")?.call((id,))?;
        Ok(found.into_object())
    }

    /// Drops the records of signal `id`.
    pub(crate) fn forget(&self, id: u32) -> JsResult<()> {
        self.function("forget")?.call((id,))
    }

    /// Holds signal `id` strongly.
    pub(crate) fn pin(&self, id: u32) -> JsResult<()> {
        self.function("pin")?.call((id,))
    }

    /// Ends the strong hold of signal `id`.
    pub(crate) fn unpin(&self, id: u32) -> JsResult<()> {
        self.function("unpin")?.call((id,))
    }
}
