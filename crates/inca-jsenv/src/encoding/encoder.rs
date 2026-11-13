// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `TextEncoder` class and the UTF-8 encoding of a WTF-8 string.

// `Encoder` derives `JsLifetime`, and the class has no state and no JS value.
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::function::{Opt, This};
use rquickjs::object::Property;
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, TypedArray, Value};

use super::{Construct, STORE, is_detached, is_resizable, read};
use crate::class::{
    define_class, define_getter, define_global, define_method, define_sized_method, plain_class,
};
use crate::quickjs::{
    UINT8, is_immutable_array_buffer, string_wtf8, typed_array_kind, typed_array_range,
    write_uint8_array,
};
use crate::vault::vault;

const NOT_A_UINT8_ARRAY: &str = "the destination is not a Uint8Array. pass a Uint8Array";
const NOT_AN_ENCODER: &str = "the receiver is not a TextEncoder. call the method on an instance";

/// A `TextEncoder` instance. It has no state.
// `Encoder` has no state, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct Encoder;

plain_class!(Encoder, "TextEncoder");

pub(super) fn define_encoder<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let class = define_class(ctx, proto, "TextEncoder", 0, Construct { decoder: false })?;
    define_getter(
        proto,
        "encoding",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(this): This<Value<'js>>| encoder_of(&ctx, &this).map(|()| "utf-8"),
        )?,
    )?;
    define_method(
        proto,
        "encode",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(this): This<Value<'js>>, input: Opt<Value<'js>>| {
                encoder_of(&ctx, &this)?;
                let wtf8 = match input.0 {
                    Some(input) if !input.is_undefined() => string_wtf8(&input)?,
                    _ => Vec::new(),
                };
                TypedArray::new_copy(ctx, encode(&wtf8))
            },
        )?,
    )?;
    define_sized_method(
        ctx,
        proto,
        "encodeInto",
        2,
        |ctx: Ctx<'js>,
         This(this): This<Value<'js>>,
         source: Opt<Value<'js>>,
         destination: Opt<Value<'js>>| {
            encoder_of(&ctx, &this)?;
            encode_into_destination(&ctx, source.0, destination.0)
        },
    )?;
    define_global(ctx, "TextEncoder", class)
}

/// The `TextEncoder` that `this` is, or a `TypeError`.
fn encoder_of<'js>(ctx: &Ctx<'js>, this: &Value<'js>) -> JsResult<()> {
    Class::<Encoder>::from_value(this)
        .map(drop)
        .map_err(|_| Exception::throw_type(ctx, NOT_AN_ENCODER))
}

/// A new `{ read, written }` object.
fn result_object<'js>(ctx: &Ctx<'js>, read: usize, written: usize) -> JsResult<Object<'js>> {
    let result = Object::new(ctx.clone())?;
    for (key, count) in [("read", read), ("written", written)] {
        result.prop(
            key,
            Property::from(count).writable().enumerable().configurable(),
        )?;
    }
    Ok(result)
}

fn encode_into_destination<'js>(
    ctx: &Ctx<'js>,
    source: Option<Value<'js>>,
    destination: Option<Value<'js>>,
) -> JsResult<Object<'js>> {
    let source = string_wtf8(&source.unwrap_or_else(|| Value::new_undefined(ctx.clone())))?;
    let wrong = || Exception::throw_type(ctx, NOT_A_UINT8_ARRAY);
    let destination = destination
        .filter(|value| typed_array_kind(value) == Some(UINT8))
        .ok_or_else(wrong)?;
    let store: Object = vault(ctx)?.get(STORE)?;
    let buffer: Value = read(&store, "typedBuffer", &destination)?;
    if is_resizable(&store, &buffer)? || is_immutable_array_buffer(&buffer) {
        return Err(wrong());
    }
    if is_detached(&store, &buffer)? {
        return result_object(ctx, 0, 0);
    }
    let (_, room) = typed_array_range(&destination)?;
    let mut bytes = vec![0; room.min(source.len())];
    let (read, written) = encode_into(&source, &mut bytes);
    if written > 0 {
        write_uint8_array(&destination, &bytes[..written])?;
    }
    result_object(ctx, read, written)
}

/// The bytes of U+FFFD.
const REPLACEMENT: [u8; 3] = [0xEF, 0xBF, 0xBD];

/// The byte length of the sequence that starts with `lead`.
fn sequence_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Writes the whole scalar values of `wtf8` that fit into `dest`, replacing each unpaired
/// surrogate with U+FFFD. Returns the number of UTF-16 code units consumed (a pair counts 2)
/// and the number of bytes written.
fn encode_into(wtf8: &[u8], dest: &mut [u8]) -> (usize, usize) {
    let (mut read, mut written, mut at) = (0, 0, 0);
    while let Some(&lead) = wtf8.get(at) {
        let len = sequence_len(lead);
        let lone = lead == 0xED && wtf8.get(at + 1).is_some_and(|&next| next >= 0xA0);
        let bytes = if lone {
            &REPLACEMENT[..]
        } else {
            &wtf8[at..(at + len).min(wtf8.len())]
        };
        let Some(slot) = dest.get_mut(written..written + bytes.len()) else {
            break;
        };
        slot.copy_from_slice(bytes);
        written += bytes.len();
        read += if len == 4 { 2 } else { 1 };
        at += len;
    }
    (read, written)
}

/// The UTF-8 bytes of `wtf8`, with each unpaired surrogate replaced by U+FFFD.
pub(crate) fn encode(wtf8: &[u8]) -> Vec<u8> {
    let mut out = vec![0; wtf8.len()];
    let (_, written) = encode_into(wtf8, &mut out);
    out.truncate(written);
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::Engine;
    use crate::tests::{memory_size, run, thrown};

    #[rstest]
    #[case::empty("encode()", "")]
    #[case::undefined("encode(undefined)", "")]
    #[case::japanese("encode('あ')", "227,129,130")]
    fn encode_returns_a_uint8_array_with_its_own_buffer(#[case] call: &str, #[case] bytes: &str) {
        let result = run(&format!(
            r"
            const u = new TextEncoder().{call};
            [
                u instanceof Uint8Array,
                u.buffer.byteLength === u.length,
                u.byteOffset,
                u.join(),
            ].join('|');
            "
        ));

        assert_eq!(result, format!("true|true|0|{bytes}"));
    }

    #[test]
    fn encode_into_returns_read_and_written_as_own_enumerable_properties() {
        assert_eq!(
            run("Object.keys(new TextEncoder().encodeInto('a', new Uint8Array(2))).join()"),
            "read,written"
        );
    }

    #[test]
    fn encode_into_writes_only_whole_characters() {
        assert_eq!(
            run(r"
                const u = new Uint8Array(4);
                const result = new TextEncoder().encodeInto('aあい', u);
                JSON.stringify([result, Array.from(u)]);
            "),
            r#"[{"read":2,"written":4},[97,227,129,130]]"#
        );
    }

    #[rstest]
    #[case::clamped("new Uint8ClampedArray(4)")]
    #[case::array("[0, 0, 0, 0]")]
    fn encode_into_rejects_a_destination_that_is_no_uint8_array(#[case] destination: &str) {
        assert_eq!(
            thrown(&format!("new TextEncoder().encodeInto('a', {destination})")),
            format!("TypeError: {NOT_A_UINT8_ARRAY}")
        );
    }

    #[test]
    fn encode_into_writes_into_a_shared_array_buffer() {
        assert_eq!(
            run(r"
                const u = new Uint8Array(new SharedArrayBuffer(4));
                new TextEncoder().encodeInto('ab', u);
                Array.from(u).join();
            "),
            "97,98,0,0"
        );
    }

    #[test]
    fn encode_into_a_detached_destination_returns_zero_without_a_throw() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(8);
                const u = new Uint8Array(b);
                b.transfer();
                JSON.stringify(new TextEncoder().encodeInto('a', u));
            "),
            r#"{"read":0,"written":0}"#
        );
    }

    #[rstest]
    #[case::length_tracking(
        r"
        const destination = new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }));
        "
    )]
    #[case::growable_shared(
        r"
        const destination = new Uint8Array(new SharedArrayBuffer(4, { maxByteLength: 4 }));
        "
    )]
    #[case::detached_resizable(
        r"
        const b = new ArrayBuffer(4, { maxByteLength: 8 });
        const destination = new Uint8Array(b);
        b.transfer();
        "
    )]
    #[case::immutable(
        r"
        const destination = new Uint8Array(new ArrayBuffer(4).transferToImmutable());
        "
    )]
    fn encode_into_rejects_a_destination_over_a_buffer_that_cannot_be_written(#[case] setup: &str) {
        assert_eq!(
            thrown(&format!(
                "{setup}\nnew TextEncoder().encodeInto('a', destination)"
            )),
            format!("TypeError: {NOT_A_UINT8_ARRAY}")
        );
    }

    #[test]
    fn encode_into_converts_the_source_before_it_checks_the_destination() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                const u = new Uint8Array(b);
                const source = {
                    toString() {
                        b.transfer();
                        return 'a';
                    },
                };
                JSON.stringify(new TextEncoder().encodeInto(source, u));
            "),
            r#"{"read":0,"written":0}"#
        );
    }

    #[test]
    fn the_encoder_encoding_is_utf_8() {
        assert_eq!(run("new TextEncoder().encoding"), "utf-8");
    }

    #[rstest]
    #[case::encode("TextEncoder.prototype.encode.call({})")]
    #[case::encode_into("TextEncoder.prototype.encodeInto.call({}, 'a', new Uint8Array(1))")]
    #[case::encoding(
        "Object.getOwnPropertyDescriptor(TextEncoder.prototype, 'encoding').get.call({})"
    )]
    fn an_encoder_member_on_a_wrong_receiver_throws(#[case] call: &str) {
        assert_eq!(
            thrown(call),
            "TypeError: the receiver is not a TextEncoder. call the method on an instance"
        );
    }

    #[test]
    fn the_memory_limit_counts_the_array_that_encode_returns() {
        let engine = Engine::new().unwrap();
        let before = memory_size(&engine);
        engine
            .eval::<()>("globalThis.kept = new TextEncoder().encode('a'.repeat(1024 * 1024));")
            .unwrap();

        assert!(memory_size(&engine) - before >= 1024 * 1024);
    }

    #[rstest]
    #[case::no_room("あ", 0, 0, 0)]
    #[case::half_a_character("あ", 2, 0, 0)]
    #[case::exact_character("あ", 3, 1, 3)]
    #[case::pair_without_room("😀", 3, 0, 0)]
    #[case::pair_with_room("😀", 4, 2, 4)]
    fn encode_into_stops_before_a_sequence_that_does_not_fit(
        #[case] text: &str,
        #[case] room: usize,
        #[case] read: usize,
        #[case] written: usize,
    ) {
        let mut dest = vec![0; room];

        assert_eq!(encode_into(text.as_bytes(), &mut dest), (read, written));
    }

    #[rstest]
    #[case::high(&[0xED, 0xA0, 0x80], 1)]
    #[case::low_between(&[0x61, 0xED, 0xB0, 0x80, 0x62], 3)]
    fn encode_into_replaces_a_lone_surrogate_and_reads_one_unit(
        #[case] wtf8: &[u8],
        #[case] read: usize,
    ) {
        let mut dest = vec![0; 16];
        let (units, written) = encode_into(wtf8, &mut dest);

        assert_eq!((units, written), (read, wtf8.len()));
        assert!(dest[..written].windows(3).any(|w| w == [0xEF, 0xBF, 0xBD]));
    }
}
