// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `crypto.subtle.digest()`: SHA-1, SHA-256, SHA-384 and SHA-512 over a buffer source.

use rquickjs::{ArrayBuffer, Ctx, Exception, Object, Promise, Result as JsResult, Value};
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha384, Sha512};

use super::classes::SubtleInstance;
use crate::buffer_source::{bytes_of, source_of, store};
use crate::class::{define_this_method, receiver, require};
use crate::quickjs::string_wtf8;

const NO_DATA: &str = "the data is not a buffer source. pass an ArrayBuffer or a view of one";
const NO_NAME: &str = "the algorithm has no name. pass a name or an object with a name";
const NOT_SUPPORTED: &str =
    "the algorithm is not supported. use SHA-1, SHA-256, SHA-384 or SHA-512";

/// The hash functions `digest` offers.
enum Hash {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

impl Hash {
    /// The function whose name is `name`, compared in ASCII case-insensitively.
    fn from_name(name: &[u8]) -> Option<Self> {
        match name.to_ascii_lowercase().as_slice() {
            b"sha-1" => Some(Self::Sha1),
            b"sha-256" => Some(Self::Sha256),
            b"sha-384" => Some(Self::Sha384),
            b"sha-512" => Some(Self::Sha512),
            _ => None,
        }
    }

    fn digest(&self, bytes: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha1 => Sha1::digest(bytes).to_vec(),
            Self::Sha256 => Sha256::digest(bytes).to_vec(),
            Self::Sha384 => Sha384::digest(bytes).to_vec(),
            Self::Sha512 => Sha512::digest(bytes).to_vec(),
        }
    }
}

/// Defines `digest` on `proto`.
pub(super) fn define<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    define_this_method(ctx, proto, "digest", 2, |ctx, this, args| {
        settle(ctx, || run(ctx, &this, args))
    })?;
    Ok(())
}

/// The digest of `args[1]` with the algorithm `args[0]`.
fn run<'js>(ctx: &Ctx<'js>, this: &Value<'js>, args: &[Value<'js>]) -> JsResult<ArrayBuffer<'js>> {
    receiver::<SubtleInstance>(ctx, this)?;
    require(ctx, "digest", args, 2)?;
    let algorithm = &args[0];
    if !algorithm.is_object() {
        string_wtf8(algorithm)?;
    }
    let data = &args[1];
    if data.is_undefined() {
        return Err(Exception::throw_type(ctx, NO_DATA));
    }
    let store = store(ctx)?;
    let source = source_of(
        ctx,
        &store,
        Some(data.clone()),
        "data",
        "an ArrayBuffer or a view of one",
        false,
    )?;
    let hash = Hash::from_name(&algorithm_name(ctx, algorithm)?)
        .ok_or_else(|| Exception::throw_dom(ctx, "NotSupportedError", NOT_SUPPORTED))?;
    let bytes = bytes_of(&store, &source)?;
    ArrayBuffer::new(ctx.clone(), hash.digest(&bytes))
}

/// The WTF-8 bytes of the name of `algorithm`: a string is the name, and an object gives its
/// `name` member, read once.
fn algorithm_name<'js>(ctx: &Ctx<'js>, algorithm: &Value<'js>) -> JsResult<Vec<u8>> {
    let Some(object) = algorithm.as_object() else {
        return string_wtf8(algorithm);
    };
    let name: Value = object.get("name")?;
    if name.is_undefined() {
        return Err(Exception::throw_type(ctx, NO_NAME));
    }
    string_wtf8(&name)
}

/// A promise that `run` settles before this returns: resolved with its buffer, or rejected with
/// the exception it left pending.
fn settle<'js>(
    ctx: &Ctx<'js>,
    run: impl FnOnce() -> JsResult<ArrayBuffer<'js>>,
) -> JsResult<Promise<'js>> {
    let (promise, resolve, reject) = Promise::new(ctx)?;
    match run() {
        Ok(buffer) => resolve.call::<_, ()>((buffer,))?,
        Err(_) => reject.call::<_, ()>((ctx.catch(),))?,
    }
    Ok(promise)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::Engine;
    use crate::tests::around;

    const HEX_JS: &str = r"
        const hex = (b) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, '0')).join('');
        const abc = new TextEncoder().encode('abc');
        ";

    const NO_DATA: &str =
        "TypeError: the data is not a buffer source. pass an ArrayBuffer or a view of one";
    const SHARED: &str = concat!(
        "TypeError: the data is shared memory. ",
        "pass an ArrayBuffer or a view of one that is not shared"
    );
    const RESIZABLE: &str = concat!(
        "TypeError: the data is resizable memory. ",
        "pass an ArrayBuffer or a view of one that is not resizable"
    );

    /// The result of the async function body `body`, as a string.
    fn run_async(body: &str) -> String {
        let engine = Engine::new().unwrap();
        engine
            .eval::<String>(&format!(
                r"
                (async () => {{
                    {HEX_JS}
                    {body}
                }})().then((v) => (globalThis.out = String(v)));
                'started';
                "
            ))
            .unwrap();
        assert!(engine.run_jobs().is_empty());
        engine.eval("out").unwrap()
    }

    /// What `call` gives: `<byte length>:<first 8 bytes in hex>`, `<name>: <message>` for a
    /// rejection and `threw: <name>: <message>` for an exception thrown by the call itself.
    fn outcome(call: &str) -> String {
        run_async(&format!(
            r"
            let p;
            try {{
                p = {call};
            }} catch (e) {{
                return `threw: ${{e.name}}: ${{e.message}}`;
            }}
            try {{
                const b = await p;
                return `${{b.byteLength}}:${{hex(b).slice(0, 16)}}`;
            }} catch (e) {{
                return `${{e.name}}: ${{e.message}}`;
            }}
            "
        ))
    }

    #[rstest]
    #[case::sha1("SHA-1", "20:a9993e364706816a")]
    #[case::sha256("SHA-256", "32:ba7816bf8f01cfea")]
    #[case::sha384("SHA-384", "48:cb00753f45a35e8b")]
    #[case::sha512("SHA-512", "64:ddaf35a193617aba")]
    fn digest_gives_the_known_value(#[case] name: &str, #[case] expected: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest('{name}', abc)")),
            expected
        );
    }

    #[rstest]
    #[case::window(
        "new Uint16Array(new Uint8Array([255, 255, 97, 98, 99, 100, 255, 255]).buffer, 2, 2)",
        "new Uint8Array([97, 98, 99, 100])"
    )]
    #[case::empty_window("new Uint8Array(new ArrayBuffer(8), 4, 0)", "new Uint8Array(0)")]
    fn digest_hashes_the_byte_range_of_the_view(#[case] view: &str, #[case] same: &str) {
        assert_eq!(
            run_async(&format!(
                r"
                const a = hex(await crypto.subtle.digest('SHA-256', {view}));
                const b = hex(await crypto.subtle.digest('SHA-256', {same}));
                return a === b;
                "
            )),
            "true"
        );
    }

    #[rstest]
    #[case::string("'SHA-256'")]
    #[case::object("{ name: 'SHA-256' }")]
    #[case::to_string("{ name: { toString() { return 'SHA-256'; } } }")]
    #[case::lower_case("'sha-256'")]
    #[case::mixed_case("'Sha-256'")]
    fn digest_accepts_the_forms_of_an_algorithm_identifier(#[case] algorithm: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest({algorithm}, abc)")),
            "32:ba7816bf8f01cfea"
        );
    }

    #[rstest]
    #[case::md5("'MD5'")]
    #[case::sha224("'SHA-224'")]
    #[case::sha3("'SHA-3-256'")]
    #[case::padded("' SHA-256'")]
    #[case::surrogate("'\\uD800'")]
    #[case::surrogate_name("{ name: '\\uD800' }")]
    fn digest_rejects_an_unknown_algorithm(#[case] algorithm: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest({algorithm}, abc)")),
            "NotSupportedError: the algorithm is not supported. use SHA-1, SHA-256, SHA-384 or SHA-512"
        );
    }

    #[test]
    fn digest_rejects_with_a_dom_exception_of_code_9() {
        assert_eq!(
            run_async(
                r"
                try {
                    await crypto.subtle.digest('MD5', abc);
                } catch (e) {
                    return `${e instanceof DOMException} ${e.code}`;
                }
                "
            ),
            "true 9"
        );
    }

    #[rstest]
    #[case::symbol("Symbol()", "TypeError")]
    #[case::array("[]", "TypeError")]
    #[case::empty_object("{}", "TypeError")]
    #[case::undefined_name("{ name: undefined }", "TypeError")]
    #[case::getter_throws("{ get name() { throw new RangeError('boom'); } }", "RangeError")]
    #[case::number("5", "NotSupportedError")]
    #[case::undefined("undefined", "NotSupportedError")]
    #[case::null("null", "NotSupportedError")]
    #[case::function("function () {}", "NotSupportedError")]
    #[case::number_name("{ name: 5 }", "NotSupportedError")]
    fn digest_rejects_a_bad_algorithm_value(#[case] algorithm: &str, #[case] name: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest({algorithm}, abc)"))
                .split(':')
                .next(),
            Some(name)
        );
    }

    #[test]
    fn digest_reads_the_name_once() {
        assert_eq!(
            run_async(
                r"
                let reads = 0;
                const algorithm = {
                    get name() {
                        reads += 1;
                        return reads === 1 ? 'SHA-256' : 'MD5';
                    },
                };
                const b = await crypto.subtle.digest(algorithm, abc);
                return `${reads}:${hex(b).slice(0, 16)}`;
                "
            ),
            "1:ba7816bf8f01cfea"
        );
    }

    #[rstest]
    #[case::none("crypto.subtle.digest()")]
    #[case::algorithm_only("crypto.subtle.digest('SHA-256')")]
    fn digest_requires_two_arguments(#[case] call: &str) {
        assert_eq!(
            outcome(call),
            "TypeError: too few arguments for digest. pass at least 2"
        );
    }

    #[rstest]
    #[case::string("'SHA-256', 'abc'")]
    #[case::array("'SHA-256', [1]")]
    #[case::null("'SHA-256', null")]
    #[case::undefined("'SHA-256', undefined")]
    #[case::unknown_algorithm("'MD5', 'abc'")]
    fn digest_checks_the_arguments_in_order(#[case] arguments: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest({arguments})")),
            NO_DATA
        );
    }

    #[test]
    fn digest_copies_the_data_after_it_reads_the_name() {
        assert_eq!(
            run_async(
                r"
                const data = new Uint8Array([1, 2, 3]);
                const algorithm = {
                    get name() {
                        data.fill(255);
                        return 'SHA-256';
                    },
                };
                const a = hex(await crypto.subtle.digest(algorithm, data));
                const b = hex(await crypto.subtle.digest('SHA-256', new Uint8Array([255, 255, 255])));
                return a === b;
                "
            ),
            "true"
        );
    }

    #[rstest]
    #[case::view("new Uint8Array(new SharedArrayBuffer(4))")]
    #[case::buffer("new SharedArrayBuffer(4)")]
    #[case::growable_view("new Uint8Array(new SharedArrayBuffer(4, { maxByteLength: 4 }))")]
    fn digest_refuses_shared_memory(#[case] data: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest('SHA-256', {data})")),
            SHARED
        );
    }

    #[rstest]
    #[case::length_tracking_view("new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }))")]
    #[case::fixed_view("new Uint8Array(new ArrayBuffer(4, { maxByteLength: 8 }), 0, 2)")]
    #[case::buffer("new ArrayBuffer(4, { maxByteLength: 8 })")]
    fn digest_refuses_resizable_memory(#[case] data: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest('SHA-256', {data})")),
            RESIZABLE
        );
    }

    #[test]
    fn digest_rejects_a_wrong_receiver() {
        assert_eq!(
            outcome("crypto.subtle.digest.call({}, 'SHA-256', abc)"),
            "TypeError: the receiver is not a SubtleCrypto. call the method on an instance"
        );
    }

    #[test]
    fn digest_returns_a_promise_of_an_array_buffer() {
        assert_eq!(
            run_async(
                r"
                const p = crypto.subtle.digest('SHA-256', abc);
                const b = await p;
                return `${p instanceof Promise} ${b instanceof ArrayBuffer} ${b.resizable}`;
                "
            ),
            "true true false"
        );
    }

    #[test]
    fn digesting_leaves_the_counters_at_the_baseline() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>("crypto.subtle.digest('SHA-256', new Uint8Array(4));")
                .unwrap();
        });

        assert_eq!(before, after);
    }

    #[rstest]
    #[case::array_buffer("new Uint8Array([97, 98, 99]).buffer")]
    #[case::data_view("new DataView(new Uint8Array([97, 98, 99]).buffer)")]
    fn digest_accepts_a_buffer_and_a_data_view(#[case] data: &str) {
        assert_eq!(
            outcome(&format!("crypto.subtle.digest('SHA-256', {data})")),
            "32:ba7816bf8f01cfea"
        );
    }

    #[rstest]
    #[case::view("u")]
    #[case::buffer("b")]
    fn digest_hashes_zero_bytes_for_a_detached_buffer(#[case] data: &str) {
        assert_eq!(
            outcome(&format!(
                r"
                (() => {{
                    const b = new ArrayBuffer(4);
                    const u = new Uint8Array(b);
                    b.transfer();
                    return crypto.subtle.digest('SHA-256', {data});
                }})()
                "
            )),
            "32:e3b0c44298fc1c14"
        );
    }
}
