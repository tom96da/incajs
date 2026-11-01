// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The wrappers over the string and buffer calls of `QuickJS`.
//!
//! Each wrapper copies out or writes in during one call, and no script runs in between.

use rquickjs::{Error, Exception, Result, Value, qjs};

/// The kind that [`typed_array_kind`] reports for a `Uint8Array`.
pub(crate) const UINT8: i32 = qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT8.cast_signed();

/// A `size_t` of the engine as a `usize`.
fn to_usize(size: qjs::size_t) -> usize {
    usize::try_from(size).unwrap_or(usize::MAX)
}

/// The WTF-8 bytes of `value` converted with `ToString`. An unpaired surrogate becomes a
/// three-byte sequence, and a surrogate pair one four-byte sequence.
///
/// # Errors
///
/// Returns [`Error::Exception`] with the exception pending when the conversion throws, as it
/// does for a `Symbol`.
pub(crate) fn string_wtf8(value: &Value<'_>) -> Result<Vec<u8>> {
    let ctx = value.ctx().as_raw().as_ptr();
    let mut len: qjs::size_t = 0;
    // SAFETY: the context and the value are live, and the call leaves the value's reference
    // count alone. It returns null with an exception pending, or a pointer to `len` bytes that
    // the string owns until `JS_FreeCString`.
    let ptr = unsafe { qjs::JS_ToCStringLen2(ctx, &raw mut len, value.as_raw(), false) };
    if ptr.is_null() {
        return Err(Error::Exception);
    }
    // SAFETY: `ptr` points to `len` initialized bytes that stay valid until the free below, and
    // nothing runs between the copy and the free.
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), to_usize(len)) }.to_vec();
    // SAFETY: `ptr` is the one string `JS_ToCStringLen2` returned, released here once.
    unsafe { qjs::JS_FreeCString(ctx, ptr) };
    Ok(bytes)
}

/// The kind of the typed array `value` is, as `JS_TYPED_ARRAY_*` counts it, or `None` for any
/// other value.
pub(crate) fn typed_array_kind(value: &Value<'_>) -> Option<i32> {
    // SAFETY: the call reads the class id of the value and leaves the reference count alone.
    let kind = unsafe { qjs::JS_GetTypedArrayType(value.as_raw()) };
    (kind >= 0).then_some(kind)
}

/// Whether `value` is a `DataView`.
pub(crate) fn is_data_view(value: &Value<'_>) -> bool {
    // SAFETY: the call reads the class id of the value and leaves the reference count alone.
    unsafe { qjs::JS_IsDataView(value.as_raw()) }
}

/// Whether `value` is an `ArrayBuffer`. A `SharedArrayBuffer` is not.
pub(crate) fn is_array_buffer(value: &Value<'_>) -> bool {
    // SAFETY: the call reads the class id of the value and leaves the reference count alone.
    unsafe { qjs::JS_IsArrayBuffer(value.as_raw()) }
}

/// Whether `value` is an immutable `ArrayBuffer`.
pub(crate) fn is_immutable_array_buffer(value: &Value<'_>) -> bool {
    // SAFETY: the call reads the class and the flag of the value and leaves the reference count
    // alone. It returns -1 for any value that is not an `ArrayBuffer`.
    unsafe { qjs::JS_IsImmutableArrayBuffer(value.as_raw()) == 1 }
}

/// The buffer, the byte offset and the byte length of the typed array `view`.
fn typed_array_parts<'js>(view: &Value<'js>) -> Result<(Value<'js>, usize, usize)> {
    let ctx = view.ctx();
    let (mut offset, mut length): (qjs::size_t, qjs::size_t) = (0, 0);
    // SAFETY: the context and the value are live. The call returns a new reference to the
    // buffer, or the exception value, which owns nothing. `Value::from_raw` takes over the
    // reference, so it is released once, when the value drops.
    let buffer = unsafe {
        Value::from_raw(
            ctx.clone(),
            qjs::JS_GetTypedArrayBuffer(
                ctx.as_raw().as_ptr(),
                view.as_raw(),
                &raw mut offset,
                &raw mut length,
                std::ptr::null_mut(),
            ),
        )
    };
    if buffer.is_exception() {
        return Err(Error::Exception);
    }
    Ok((buffer, to_usize(offset), to_usize(length)))
}

/// The byte offset and the byte length of the typed array `view`. The length of a
/// length-tracking view over a resizable buffer is its length at creation. `copy_range` cuts it
/// to the buffer.
///
/// # Errors
///
/// Returns [`Error::Exception`] with the exception pending when `view` is no typed array, or
/// when its buffer is detached or out of bounds.
pub(crate) fn typed_array_range(view: &Value<'_>) -> Result<(usize, usize)> {
    typed_array_parts(view).map(|(_, offset, length)| (offset, length))
}

/// A copy of the bytes of `buffer`, an `ArrayBuffer` or a `SharedArrayBuffer`, from `offset` for
/// `length` bytes. A range that reaches past the end of the buffer is cut to it.
///
/// # Errors
///
/// Returns [`Error::Exception`] with the exception pending when `buffer` is detached or no
/// buffer.
pub(crate) fn copy_range(buffer: &Value<'_>, offset: usize, length: usize) -> Result<Vec<u8>> {
    let mut size: qjs::size_t = 0;
    // SAFETY: the context and the value are live, and the call leaves the reference count alone.
    // It returns null with an exception pending, or a pointer to `size` bytes.
    let data = unsafe {
        qjs::JS_GetArrayBuffer(
            buffer.ctx().as_raw().as_ptr(),
            &raw mut size,
            buffer.as_raw(),
        )
    };
    if data.is_null() {
        return Err(Error::Exception);
    }
    let size = to_usize(size);
    let end = offset.saturating_add(length).min(size);
    let start = offset.min(end);
    // SAFETY: `data` points to `size` bytes and `start..end` lies inside them. The bytes are
    // copied before any script can detach the buffer.
    Ok(unsafe { std::slice::from_raw_parts(data.add(start), end - start) }.to_vec())
}

/// Writes `bytes` to the start of the `Uint8Array` `target`. Bytes past its
/// length, or past the end of its buffer, are dropped.
///
/// # Errors
///
/// Returns [`Error::Exception`] with the exception pending when `target` is neither a
/// `Uint8Array` nor a `Uint8ClampedArray`, when its buffer is detached or out of bounds, and
/// when its buffer is immutable.
pub(crate) fn write_uint8_array(target: &Value<'_>, bytes: &[u8]) -> Result<()> {
    let ctx = target.ctx();
    let (buffer, offset, length) = typed_array_parts(target)?;
    if is_immutable_array_buffer(&buffer) {
        return Err(Exception::throw_type(
            ctx,
            "the buffer is immutable. write to a mutable buffer",
        ));
    }
    let mut size: qjs::size_t = 0;
    // SAFETY: the context and the value are live, and the call leaves the reference count alone.
    // It returns null with an exception pending, or a pointer to `size` writable bytes.
    let data =
        unsafe { qjs::JS_GetUint8Array(ctx.as_raw().as_ptr(), &raw mut size, target.as_raw()) };
    if data.is_null() {
        return Err(Error::Exception);
    }
    let mut buffer_size: qjs::size_t = 0;
    // SAFETY: the context and the value are live, and the call leaves the reference count alone.
    // It returns null with an exception pending, or a pointer to `buffer_size` bytes.
    let base = unsafe {
        qjs::JS_GetArrayBuffer(ctx.as_raw().as_ptr(), &raw mut buffer_size, buffer.as_raw())
    };
    if base.is_null() {
        return Err(Error::Exception);
    }
    // The length of a length-tracking view is stale after the buffer shrinks, so the end of the
    // buffer bounds the write too.
    let count = bytes
        .len()
        .min(length)
        .min(to_usize(buffer_size).saturating_sub(offset));
    // SAFETY: `data` is `offset` bytes into a buffer of `buffer_size` bytes and `count` is at
    // most the rest of it. `bytes` is a Rust slice, so it cannot overlap engine memory.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, count) };
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rquickjs::Ctx;
    use rstest::rstest;

    use super::*;
    use crate::tests::with_realm;

    /// The value that `source` evaluates to.
    fn value<'js>(ctx: &Ctx<'js>, source: &str) -> Value<'js> {
        ctx.eval(source).unwrap()
    }

    #[rstest]
    #[case::lone_high("'\\uD800'", &[0xED, 0xA0, 0x80])]
    #[case::pair("'\\uD83D\\uDE00'", &[0xF0, 0x9F, 0x98, 0x80])]
    #[case::number("12", b"12")]
    fn string_wtf8_converts_with_to_string(#[case] source: &str, #[case] bytes: &[u8]) {
        with_realm(|ctx| {
            assert_eq!(string_wtf8(&value(&ctx, source)).unwrap(), bytes);
        });
    }

    #[test]
    fn string_wtf8_of_a_symbol_leaves_a_type_error_pending() {
        with_realm(|ctx| {
            assert!(string_wtf8(&value(&ctx, "Symbol()")).is_err());
            assert!(ctx.catch().is_error());
        });
    }

    #[rstest]
    #[case::uint8("new Uint8Array(1)", Some(UINT8))]
    #[case::clamped(
        "new Uint8ClampedArray(1)",
        Some(qjs::JSTypedArrayEnum_JS_TYPED_ARRAY_UINT8C.cast_signed())
    )]
    #[case::object("({})", None)]
    fn typed_array_kind_names_the_kind_of_a_typed_array(
        #[case] source: &str,
        #[case] kind: Option<i32>,
    ) {
        with_realm(|ctx| {
            assert_eq!(typed_array_kind(&value(&ctx, source)), kind);
        });
    }

    #[rstest]
    #[case::array_buffer("new ArrayBuffer(1)", [true, false])]
    #[case::shared_array_buffer("new SharedArrayBuffer(1)", [false, false])]
    #[case::data_view("new DataView(new ArrayBuffer(1))", [false, true])]
    fn the_class_checks_tell_a_buffer_from_a_shared_buffer_and_a_view(
        #[case] source: &str,
        #[case] expected: [bool; 2],
    ) {
        with_realm(|ctx| {
            let v = value(&ctx, source);

            assert_eq!([is_array_buffer(&v), is_data_view(&v)], expected);
        });
    }

    #[rstest]
    #[case::plain("new ArrayBuffer(1)", false)]
    #[case::immutable("new ArrayBuffer(1).transferToImmutable()", true)]
    #[case::shared("new SharedArrayBuffer(1)", false)]
    #[case::not_a_buffer("({})", false)]
    fn is_immutable_array_buffer_is_true_only_for_an_immutable_buffer(
        #[case] source: &str,
        #[case] immutable: bool,
    ) {
        with_realm(|ctx| {
            assert_eq!(is_immutable_array_buffer(&value(&ctx, source)), immutable);
        });
    }

    #[test]
    fn typed_array_range_gives_the_offset_and_the_byte_length() {
        with_realm(|ctx| {
            let view = value(&ctx, "new Uint16Array(new ArrayBuffer(16), 4, 3)");

            assert_eq!(typed_array_range(&view).unwrap(), (4, 6));
        });
    }

    #[rstest]
    #[case::detached(
        r"
        const b = new ArrayBuffer(4);
        const u = new Uint8Array(b);
        b.transfer();
        u;
        "
    )]
    #[case::not_a_view("({})")]
    fn typed_array_range_fails_without_a_readable_view(#[case] source: &str) {
        with_realm(|ctx| {
            assert!(typed_array_range(&value(&ctx, source)).is_err());
        });
    }

    #[rstest]
    #[case::inside(1, 2, &[2, 3])]
    #[case::cut_at_the_end(2, 10, &[3, 4])]
    #[case::past_the_end(9, 1, &[])]
    #[case::overflowing_length(0, usize::MAX, &[1, 2, 3, 4])]
    fn copy_range_cuts_the_range_to_the_buffer(
        #[case] offset: usize,
        #[case] length: usize,
        #[case] bytes: &[u8],
    ) {
        with_realm(|ctx| {
            let buffer = value(&ctx, "new Uint8Array([1, 2, 3, 4]).buffer");

            assert_eq!(copy_range(&buffer, offset, length).unwrap(), bytes);
        });
    }

    #[rstest]
    #[case::detached(
        r"
        const b = new ArrayBuffer(4);
        b.transfer();
        b;
        "
    )]
    #[case::not_a_buffer("({})")]
    fn copy_range_fails_without_a_readable_buffer(#[case] source: &str) {
        with_realm(|ctx| {
            assert!(copy_range(&value(&ctx, source), 0, 1).is_err());
        });
    }

    #[test]
    fn write_uint8_array_writes_the_bytes_that_fit() {
        with_realm(|ctx| {
            let target = value(&ctx, "globalThis.u = new Uint8Array(2)");
            write_uint8_array(&target, &[7, 8, 9]).unwrap();

            assert_eq!(value(&ctx, "u.join()").get::<String>().unwrap(), "7,8");
        });
    }

    #[test]
    fn write_uint8_array_fails_for_another_typed_array() {
        with_realm(|ctx| {
            assert!(write_uint8_array(&value(&ctx, "new Uint16Array(2)"), &[1]).is_err());
        });
    }

    #[test]
    fn write_uint8_array_stops_at_the_end_of_a_shrunk_buffer() {
        with_realm(|ctx| {
            let target = value(
                &ctx,
                r"
                globalThis.b = new ArrayBuffer(4, { maxByteLength: 8 });
                globalThis.u = new Uint8Array(b, 1);
                b.resize(2);
                u;
                ",
            );
            write_uint8_array(&target, &[7, 8, 9, 10]).unwrap();

            assert_eq!(
                value(&ctx, "new Uint8Array(b).join()")
                    .get::<String>()
                    .unwrap(),
                "0,7"
            );
        });
    }

    #[test]
    fn write_uint8_array_refuses_an_immutable_buffer() {
        with_realm(|ctx| {
            let target = value(
                &ctx,
                "new Uint8Array(new ArrayBuffer(2).transferToImmutable())",
            );

            assert!(write_uint8_array(&target, &[1]).is_err());
        });
    }
}
