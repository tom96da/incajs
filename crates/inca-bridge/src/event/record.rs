// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The native record of an event: the structs, their lifetime marker and
//! their trace.

// A derive on a struct with a lifetime generates items that an `allow` on the
// struct does not cover.
#![allow(clippy::disallowed_macros, clippy::disallowed_types)]

use std::cell::{Cell, RefCell};

use rquickjs::class::{JsClass, Readable, Trace, Tracer};
use rquickjs::function::Constructor;
use rquickjs::{Ctx, JsLifetime, Object, Result as JsResult, Value};

use super::Flags;
use super::payload::Payload;

/// An event: the object every listener of one walk receives, and the state
/// its getters and methods read.
#[derive(JsLifetime)]
pub(super) struct IncaEvent<'js> {
    pub(super) state: State,
    pub(super) held: Held<'js>,
}

/// Every JS value an event holds. Each one is traced, so it lives as long as
/// the event, and a cycle through it is collected. An untraced value aborts
/// the process when the runtime drops.
#[derive(JsLifetime)]
pub(super) struct Held<'js> {
    /// What stands for the node the event was last dispatched at, `None`
    /// before the first dispatch.
    pub(super) target: RefCell<Option<Value<'js>>>,
    /// What stands for each node of the path while a walk runs, target
    /// first. Empty outside a walk.
    pub(super) path: RefCell<Vec<Value<'js>>>,
    /// What stands for the other node of a mouse or focus event.
    pub(super) related: Option<Value<'js>>,
    /// The `detail` of a `CustomEvent`, `None` for any other event.
    pub(super) detail: RefCell<Option<Value<'js>>>,
}

/// The fields of an [`IncaEvent`] that hold no JS value. It has no lifetime
/// parameter, so a JS value cannot be added here. A walk sets the cells
/// between listener calls. `initEvent` sets the type and the flags of an idle
/// event.
#[derive(JsLifetime)]
pub(super) struct State {
    pub(super) name: RefCell<String>,
    pub(super) flags: Cell<Flags>,
    pub(super) payload: Payload,
    pub(super) time_stamp: f64,
    pub(super) trusted: Cell<bool>,
    /// The number of the last walk that reached a listener.
    pub(super) event_id: Cell<Option<u64>>,
    pub(super) dispatching: Cell<bool>,
    pub(super) stop: Cell<bool>,
    pub(super) immediate: Cell<bool>,
    pub(super) canceled: Cell<bool>,
    /// Whether the running listener is passive.
    pub(super) passive: Cell<bool>,
    pub(super) phase: Cell<u8>,
    /// The index in the path of the node whose listeners run.
    pub(super) current: Cell<Option<usize>>,
}

impl<'js> Trace<'js> for IncaEvent<'js> {
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        let IncaEvent { state: _, held } = self;
        held.trace(tracer);
    }
}

// Runs inside a GC callback of QuickJS: it must not panic, so no `borrow()`,
// `unwrap` or indexing here.
impl<'js> Trace<'js> for Held<'js> {
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        // No `..` here: a new field must fail to compile until it is traced.
        let Held {
            target,
            path,
            related,
            detail,
        } = self;
        related.trace(tracer);
        // rquickjs traces a `Readable` class even mid-walk, so each cell guards itself.
        if let Ok(target) = target.try_borrow() {
            target.trace(tracer);
        }
        if let Ok(path) = path.try_borrow() {
            path.trace(tracer);
        }
        if let Ok(detail) = detail.try_borrow() {
            detail.trace(tracer);
        }
    }
}

impl<'js> JsClass<'js> for IncaEvent<'js> {
    const NAME: &'static str = "Event";

    type Mutable = Readable;

    // An event takes the prototype of its kind from the vault when it is made.
    fn prototype(_ctx: &Ctx<'js>) -> JsResult<Option<Object<'js>>> {
        Ok(None)
    }

    fn constructor(_ctx: &Ctx<'js>) -> JsResult<Option<Constructor<'js>>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rquickjs::Class;

    use super::*;
    use crate::test_support::engine_with_bindings;

    #[test]
    fn a_collection_with_the_cells_borrowed_does_not_abort() {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval::<()>(
                r"
                globalThis.detail = {};
                globalThis.kept = new CustomEvent('x', { detail });
                ",
            )
            .unwrap();

        engine.with(|ctx| {
            let this: Value = ctx.globals().get("kept").unwrap();
            let event = Class::<IncaEvent>::from_value(&this).unwrap();
            let event = event.borrow();
            let _target = event.held.target.borrow_mut();
            let _path = event.held.path.borrow_mut();
            let _detail = event.held.detail.borrow_mut();
            ctx.run_gc();
        });

        assert!(engine.eval::<bool>("kept.detail === detail").unwrap());
    }
}
