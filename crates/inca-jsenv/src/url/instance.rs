// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `URL` and `URLSearchParams` instances.

// `UrlInstance` and `ParamsInstance` derive `JsLifetime`. Only `UrlInstance` holds a JS value, its
// `searchParams` object, and its trace names it.

use rquickjs::class::{JsClass, Readable, Trace, Tracer};
use rquickjs::function::Constructor;
use rquickjs::{Class, Ctx, JsLifetime, Object, Result as JsResult};

use super::state::Shared;
use crate::class::plain_class;

/// A `URL` instance. Its `searchParams` object is created with it and traced, so a cycle
/// through it is collected.
#[derive(JsLifetime)]
pub(super) struct UrlInstance<'js> {
    pub(super) state: Shared,
    pub(super) params: Class<'js, ParamsInstance>,
}

impl<'js> Trace<'js> for UrlInstance<'js> {
    // Runs inside a GC callback of QuickJS: it must not panic.
    fn trace<'a>(&self, tracer: Tracer<'a, 'js>) {
        // No `..` here: a new field must fail to compile until it is traced.
        let UrlInstance { state: _, params } = self;
        params.trace(tracer);
    }
}

impl<'js> JsClass<'js> for UrlInstance<'js> {
    const NAME: &'static str = "URL";

    type Mutable = Readable;

    // An instance takes the prototype from the vault when it is made.
    fn prototype(_ctx: &Ctx<'js>) -> JsResult<Option<Object<'js>>> {
        Ok(None)
    }

    fn constructor(_ctx: &Ctx<'js>) -> JsResult<Option<Constructor<'js>>> {
        Ok(None)
    }
}

/// A `URLSearchParams` instance.
// `ParamsInstance` holds the list of pairs and no JS value, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct ParamsInstance(pub(super) Shared);

plain_class!(ParamsInstance, "URLSearchParams");
