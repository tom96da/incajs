// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `crypto.getRandomValues()` and `crypto.randomUUID()`, which read the system random source.

use rquickjs::{Ctx, Exception, Object, Result as JsResult, Value};

use super::classes::CryptoInstance;
use crate::buffer_source::{Source, is_detached, source_of, store};
use crate::class::{define_this_method, receiver, require};
use crate::quickjs::{is_integer_kind, typed_array_kind, typed_array_range, write_typed_array};

/// The lowercase hexadecimal digits.
const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// The most bytes one call of `getRandomValues` fills.
const MAX_BYTES: usize = 65_536;

const NOT_INTEGER: &str = concat!(
    "the array is not an integer typed array. pass an Int8Array, Uint8Array, Uint8ClampedArray, ",
    "Int16Array, Uint16Array, Int32Array, Uint32Array, BigInt64Array or BigUint64Array"
);
const TOO_LONG: &str = "the array is longer than 65536 bytes. fill it in parts";
const NOT_TYPED: &str = "the array is not a typed array. pass an integer typed array";
const SOURCE_FAILED: &str = "the system random source failed. try again";

/// Defines `getRandomValues` and `randomUUID` on `proto`.
pub(super) fn define<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    define_this_method(ctx, proto, "getRandomValues", 1, |ctx, this, args| {
        get_random_values(ctx, &this, args)
    })?;
    define_this_method(ctx, proto, "randomUUID", 0, |ctx, this, _| {
        receiver::<CryptoInstance>(ctx, &this)?;
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&random_bytes(ctx, 16, getrandom::fill)?);
        Ok(format_uuid(&bytes))
    })?;
    Ok(())
}

/// Fills the integer typed array `args[0]` with random bytes and returns it.
fn get_random_values<'js>(
    ctx: &Ctx<'js>,
    this: &Value<'js>,
    args: &[Value<'js>],
) -> JsResult<Value<'js>> {
    receiver::<CryptoInstance>(ctx, this)?;
    require(ctx, "getRandomValues", args, 1)?;
    let store = store(ctx)?;
    let source = source_of(
        ctx,
        &store,
        args.first().cloned(),
        "array",
        "an integer typed array",
        false,
    )?;
    let (view, buffer) = match source {
        Source::Typed(view, buffer) => (view, buffer),
        Source::Data(..) => {
            return Err(Exception::throw_dom(ctx, "TypeMismatchError", NOT_INTEGER));
        }
        Source::Whole(_) | Source::Empty => return Err(Exception::throw_type(ctx, NOT_TYPED)),
    };
    if !typed_array_kind(&view).is_some_and(is_integer_kind) {
        return Err(Exception::throw_dom(ctx, "TypeMismatchError", NOT_INTEGER));
    }
    if is_detached(&store, &buffer)? {
        return Ok(view);
    }
    let (_, length) = typed_array_range(&view)?;
    if length > MAX_BYTES {
        return Err(Exception::throw_dom(ctx, "QuotaExceededError", TOO_LONG));
    }
    write_typed_array(&view, &random_bytes(ctx, length, getrandom::fill)?)?;
    Ok(view)
}

/// `len` bytes that `fill` wrote.
///
/// # Errors
///
/// Throws an `OperationError` when `fill` fails.
fn random_bytes(
    ctx: &Ctx<'_>,
    len: usize,
    fill: impl FnOnce(&mut [u8]) -> Result<(), getrandom::Error>,
) -> JsResult<Vec<u8>> {
    let mut bytes = vec![0; len];
    fill(&mut bytes).map_err(|_| Exception::throw_dom(ctx, "OperationError", SOURCE_FAILED))?;
    Ok(bytes)
}

/// The version 4 UUID text of `bytes`: the version and variant bits are set, then the bytes are
/// written as lowercase hexadecimal in groups of 4, 2, 2, 2 and 6.
fn format_uuid(bytes: &[u8; 16]) -> String {
    let mut bytes = *bytes;
    bytes[6] = bytes[6] & 0x0f | 0x40;
    bytes[8] = bytes[8] & 0x3f | 0x80;
    let hex = |part: &[u8]| {
        part.iter()
            .flat_map(|b| [DIGITS[usize::from(b >> 4)], DIGITS[usize::from(b & 15)]])
            .map(char::from)
            .collect::<String>()
    };
    format!(
        "{}-{}-{}-{}-{}",
        hex(&bytes[..4]),
        hex(&bytes[4..6]),
        hex(&bytes[6..8]),
        hex(&bytes[8..10]),
        hex(&bytes[10..])
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::Engine;
    use crate::tests::{around, run, thrown, with_realm};

    #[test]
    fn get_random_values_returns_the_argument() {
        assert_eq!(
            run(r"
                const a = new Uint8Array(4);
                String(crypto.getRandomValues(a) === a);
                "),
            "true"
        );
    }

    #[test]
    fn get_random_values_fills_the_whole_byte_length() {
        assert_eq!(
            run(r"
                const bytes = new Uint8Array(crypto.getRandomValues(new Uint32Array(1024)).buffer);
                const distinct = new Set(bytes).size;
                const edges = bytes.slice(0, 16).some(Boolean) && bytes.slice(-16).some(Boolean);
                String(distinct > 200 && edges);
                "),
            "true"
        );
    }

    #[test]
    fn get_random_values_writes_only_inside_the_view() {
        assert_eq!(
            run(r"
                const buffer = new ArrayBuffer(48);
                crypto.getRandomValues(new Int32Array(buffer, 16, 4));
                const bytes = new Uint8Array(buffer);
                const zero = (part) => part.every((b) => b === 0);
                String(zero(bytes.slice(0, 16)) && zero(bytes.slice(32)) && !zero(bytes.slice(16, 32)));
                "),
            "true"
        );
    }

    const SHARED: &str =
        "TypeError: the array is shared memory. pass an integer typed array that is not shared";
    const RESIZABLE: &str = concat!(
        "TypeError: the array is resizable memory. ",
        "pass an integer typed array that is not resizable"
    );
    const NOT_A_VIEW: &str =
        "TypeError: the array is not a typed array. pass an integer typed array";

    /// The text `crypto.getRandomValues(input)` throws, `Name: message`.
    fn thrown_by(input: &str) -> String {
        thrown(&format!("crypto.getRandomValues({input})"))
    }

    #[test]
    fn get_random_values_returns_a_detached_view() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4);
                const u = new Uint8Array(b);
                b.transfer();
                String(crypto.getRandomValues(u) === u);
                "),
            "true"
        );
    }

    #[test]
    fn get_random_values_refuses_a_detached_buffer() {
        assert_eq!(
            thrown(
                r"
                const b = new ArrayBuffer(4);
                b.transfer();
                crypto.getRandomValues(b)
                "
            ),
            NOT_A_VIEW
        );
    }

    #[rstest]
    #[case::uint8_clamped("Uint8ClampedArray", "true")]
    #[case::big_uint64("BigUint64Array", "true")]
    #[case::float16("Float16Array", "17")]
    fn get_random_values_takes_only_integer_kinds(#[case] class: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                try {{
                    crypto.getRandomValues(new {class}(4));
                    'true';
                }} catch (e) {{
                    String(e.code);
                }}
                "
            )),
            expected
        );
    }

    #[rstest]
    #[case::float32("Float32Array", 20000)]
    #[case::float64("Float64Array", 9000)]
    fn get_random_values_checks_the_kind_before_the_quota(
        #[case] class: &str,
        #[case] length: usize,
    ) {
        assert_eq!(
            run(&format!(
                r"
                try {{
                    crypto.getRandomValues(new {class}({length}));
                }} catch (e) {{
                    `${{e.name}} ${{e.code}}`;
                }}
                "
            )),
            "TypeMismatchError 17"
        );
    }

    #[test]
    fn get_random_values_refuses_a_data_view() {
        assert_eq!(
            run(r"
                try {
                    crypto.getRandomValues(new DataView(new ArrayBuffer(4)));
                } catch (e) {
                    `${e instanceof DOMException} ${e.name} ${e.message}`;
                }
                "),
            format!("true TypeMismatchError {NOT_INTEGER}")
        );
    }

    #[test]
    fn get_random_values_requires_one_argument() {
        assert_eq!(
            thrown("crypto.getRandomValues()"),
            "TypeError: too few arguments for getRandomValues. pass at least 1"
        );
    }

    #[test]
    fn get_random_values_works_with_the_receiver_given_by_call() {
        assert_eq!(
            run("String(crypto.getRandomValues.call(crypto, new Uint8Array(1)).length)"),
            "1"
        );
    }

    #[rstest]
    #[case::undefined("undefined", NOT_A_VIEW)]
    #[case::array_buffer("new ArrayBuffer(4)", NOT_A_VIEW)]
    #[case::null(
        "null",
        "TypeError: the array is not a buffer source. pass an integer typed array"
    )]
    fn get_random_values_refuses_a_value_that_is_no_view(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(thrown_by(input), expected);
    }

    #[rstest]
    #[case::shared_view("new Uint8Array(new SharedArrayBuffer(4))")]
    #[case::shared_buffer("new SharedArrayBuffer(4)")]
    #[case::growable_buffer("new SharedArrayBuffer(4, { maxByteLength: 4 })")]
    #[case::growable_view("new Uint8Array(new SharedArrayBuffer(4, { maxByteLength: 4 }))")]
    fn get_random_values_names_a_shared_buffer(#[case] input: &str) {
        assert_eq!(thrown_by(input), SHARED);
    }

    #[rstest]
    #[case::length_tracking_view(
        "new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }))",
        RESIZABLE
    )]
    #[case::fixed_view(
        "new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }), 0, 2)",
        RESIZABLE
    )]
    #[case::buffer("new ArrayBuffer(4, { maxByteLength: 8 })", RESIZABLE)]
    fn get_random_values_names_a_resizable_buffer(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(thrown_by(input), expected);
    }

    #[test]
    fn get_random_values_refuses_an_immutable_buffer() {
        assert_eq!(
            run(r"
                const b = new ArrayBuffer(4).transferToImmutable();
                let name;
                try {
                    crypto.getRandomValues(new Uint8Array(b));
                } catch (e) {
                    name = e.name;
                }
                `${name} ${new Uint8Array(b).every((x) => x === 0)}`;
                "),
            "TypeError true"
        );
    }

    #[rstest]
    #[case::at_the_limit("new Uint8Array(65536)", "ok")]
    #[case::one_over("new Uint8Array(65537)", "QuotaExceededError 22")]
    #[case::uint32_over("new Uint32Array(16385)", "QuotaExceededError 22")]
    fn get_random_values_limits_the_byte_length(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(
            run(&format!(
                r"
                try {{
                    crypto.getRandomValues({input});
                    'ok';
                }} catch (e) {{
                    `${{e.name}} ${{e.code}}`;
                }}
                "
            )),
            expected
        );
    }

    #[test]
    fn random_bytes_throws_operation_error_when_the_source_fails() {
        with_realm(|ctx| {
            let result = random_bytes(&ctx, 4, |_| Err(getrandom::Error::UNSUPPORTED));
            let error: Value = ctx.catch();

            assert!(result.is_err());
            assert_eq!(
                error
                    .into_object()
                    .unwrap()
                    .get::<_, String>("name")
                    .unwrap(),
                "OperationError"
            );
        });
    }

    #[test]
    fn random_bytes_returns_the_bytes_of_the_source() {
        with_realm(|ctx| {
            let bytes = random_bytes(&ctx, 3, |out| {
                out.fill(0xAB);
                Ok(())
            });

            assert_eq!(bytes.unwrap(), [0xAB; 3]);
        });
    }

    #[test]
    fn random_uuid_has_the_shape_of_a_uuid() {
        assert_eq!(
            run(r"
                const a = crypto.randomUUID();
                const shaped = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(a);
                String(shaped && a !== crypto.randomUUID());
                "),
            "true"
        );
    }

    #[test]
    fn format_uuid_sets_the_version() {
        let digit = |byte: u8| format_uuid(&[byte; 16]).as_bytes()[14];

        assert_eq!([digit(0), digit(0xFF)], [b'4', b'4']);
    }

    #[test]
    fn format_uuid_sets_the_variant() {
        let digit = |byte: u8| format_uuid(&[byte; 16]).as_bytes()[19];

        assert_eq!([digit(0), digit(0xFF)], [b'8', b'b']);
    }

    #[test]
    fn format_uuid_keeps_the_byte_order() {
        let bytes: [u8; 16] = std::array::from_fn(|i| u8::try_from(i).unwrap());

        assert_eq!(format_uuid(&bytes), "00010203-0405-4607-8809-0a0b0c0d0e0f");
    }

    #[test]
    fn creating_values_leaves_the_counters_at_the_baseline() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                r"
                crypto.getRandomValues(new Uint8Array(4));
                crypto.randomUUID();
                ",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }
}
