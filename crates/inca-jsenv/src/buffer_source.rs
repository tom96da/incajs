// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The checks that read a buffer source: an `ArrayBuffer`, a `SharedArrayBuffer`, a typed array
//! or a `DataView`.
//!
//! The getters that classify a buffer are kept in the vault, so a script that replaces a property
//! of `ArrayBuffer`, `DataView` or an instance changes nothing here.

use rquickjs::function::This;
use rquickjs::{Ctx, Exception, FromJs, Function, Object, Result as JsResult, Value};

use crate::quickjs::{
    copy_range, is_array_buffer, is_data_view, typed_array_kind, typed_array_range,
};
use crate::vault::vault;

/// The vault key of the getters that classify a buffer.
const STORE: &str = "buffers";

/// Evaluates to the getters that classify a buffer, read from the intrinsics before any script
/// runs.
const GETTERS_JS: &str = r"
    (() => {
        const get = (object, key) => Object.getOwnPropertyDescriptor(object, key).get;
        const typed = Object.getPrototypeOf(Uint8Array.prototype);
        return {
            __proto__: null,
            resizable: get(ArrayBuffer.prototype, 'resizable'),
            growable: get(SharedArrayBuffer.prototype, 'growable'),
            detached: get(ArrayBuffer.prototype, 'detached'),
            typedBuffer: get(typed, 'buffer'),
            viewBuffer: get(DataView.prototype, 'buffer'),
            viewOffset: get(DataView.prototype, 'byteOffset'),
            viewLength: get(DataView.prototype, 'byteLength'),
        };
    })()
    ";

/// Keeps the getters that classify a buffer in the vault of `ctx`'s realm.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let store: Object = ctx.eval(GETTERS_JS)?;
    vault(ctx)?.set(STORE, store)
}

/// The getters that classify a buffer, from the vault of `ctx`'s realm.
pub(crate) fn store<'js>(ctx: &Ctx<'js>) -> JsResult<Object<'js>> {
    vault(ctx)?.get(STORE)
}

/// Calls the getter `key` of the store on `this`.
pub(crate) fn read<'js, T: FromJs<'js>>(
    store: &Object<'js>,
    key: &str,
    this: &Value<'js>,
) -> JsResult<T> {
    store.get::<_, Function>(key)?.call((This(this.clone()),))
}

/// Whether the getter `resizable` of an `ArrayBuffer` or `growable` of a `SharedArrayBuffer`
/// says `buffer` can change its size.
pub(crate) fn is_resizable<'js>(store: &Object<'js>, buffer: &Value<'js>) -> JsResult<bool> {
    let key = if is_array_buffer(buffer) {
        "resizable"
    } else {
        "growable"
    };
    read(store, key, buffer)
}

/// Whether `buffer` is a detached `ArrayBuffer`. A `SharedArrayBuffer` cannot detach.
pub(crate) fn is_detached<'js>(store: &Object<'js>, buffer: &Value<'js>) -> JsResult<bool> {
    if is_array_buffer(buffer) {
        read(store, "detached", buffer)
    } else {
        Ok(false)
    }
}

/// Where the bytes of a buffer source are, found before any option is read.
pub(crate) enum Source<'js> {
    Empty,
    /// An `ArrayBuffer` or a `SharedArrayBuffer`.
    Whole(Value<'js>),
    /// A typed array and its buffer.
    Typed(Value<'js>, Value<'js>),
    /// A `DataView` and its buffer.
    Data(Value<'js>, Value<'js>),
}

/// Finds the buffer of `input` and checks it can be read. `what` names the argument and `pass`
/// says what to pass, in the `TypeError` for a value that is no buffer source.
pub(crate) fn source_of<'js>(
    ctx: &Ctx<'js>,
    store: &Object<'js>,
    input: Option<Value<'js>>,
    what: &str,
    pass: &str,
) -> JsResult<Source<'js>> {
    let wrong = || {
        Exception::throw_type(
            ctx,
            &format!("the {what} is not a buffer source. pass {pass}"),
        )
    };
    let Some(input) = input.filter(|input| !input.is_undefined()) else {
        return Ok(Source::Empty);
    };
    let source = if typed_array_kind(&input).is_some() {
        let buffer = read(store, "typedBuffer", &input)?;
        Source::Typed(input, buffer)
    } else if is_data_view(&input) {
        let buffer = read(store, "viewBuffer", &input)?;
        Source::Data(input, buffer)
    } else if is_array_buffer(&input) || is_shared_array_buffer(ctx, store, &input) {
        Source::Whole(input)
    } else {
        return Err(wrong());
    };
    let (Source::Whole(buffer) | Source::Typed(_, buffer) | Source::Data(_, buffer)) = &source
    else {
        return Ok(source);
    };
    if is_resizable(store, buffer)? {
        return Err(wrong());
    }
    Ok(source)
}

/// Whether `value` is a `SharedArrayBuffer`: the getter `growable` fails only on another class,
/// and that failure is cleared.
fn is_shared_array_buffer<'js>(ctx: &Ctx<'js>, store: &Object<'js>, value: &Value<'js>) -> bool {
    let probe = read::<bool>(store, "growable", value).is_ok();
    if !probe {
        drop(ctx.catch());
    }
    probe
}

/// A copy of the bytes `source` covers.
pub(crate) fn bytes_of<'js>(store: &Object<'js>, source: &Source<'js>) -> JsResult<Vec<u8>> {
    match source {
        Source::Empty => Ok(Vec::new()),
        Source::Whole(buffer) if is_detached(store, buffer)? => Ok(Vec::new()),
        Source::Whole(buffer) => copy_range(buffer, 0, usize::MAX),
        Source::Typed(_, buffer) | Source::Data(_, buffer) if is_detached(store, buffer)? => {
            Ok(Vec::new())
        }
        Source::Typed(view, buffer) => {
            let (offset, length) = typed_array_range(view)?;
            copy_range(buffer, offset, length)
        }
        Source::Data(view, buffer) => {
            let offset: usize = read(store, "viewOffset", view)?;
            let length: usize = read(store, "viewLength", view)?;
            copy_range(buffer, offset, length)
        }
    }
}
