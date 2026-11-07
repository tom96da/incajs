// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `TextDecoder` class and the `Codec` that decodes bytes.

use std::cell::RefCell;

use encoding_rs::{DecoderResult, Encoding, UTF_8, UTF_16BE, UTF_16LE};
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::function::{Opt, This};
use rquickjs::{Class, Ctx, Exception, Function, Object, Result as JsResult, Value};

use super::{Construct, STORE, flag_of, is_detached, is_resizable, options_of, read};
use crate::class::{define_class, define_getter, define_global, define_method, plain_class};
use crate::quickjs::{
    copy_range, is_array_buffer, is_data_view, typed_array_kind, typed_array_range,
};
use crate::vault::vault;

const NOT_A_BUFFER: &str = "the input is not a buffer source. pass an ArrayBuffer or a view of one";
const NOT_VALID: &str =
    "the data is not valid for the encoding. decode with fatal false to replace it";
const NOT_A_DECODER: &str = "the receiver is not a TextDecoder. call the method on an instance";

/// A `TextDecoder` instance.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct Decoder {
    pub(super) codec: RefCell<Codec>,
}

plain_class!(Decoder, "TextDecoder");

pub(super) fn define_decoder<'js>(ctx: &Ctx<'js>, proto: &Object<'js>) -> JsResult<()> {
    let class = define_class(ctx, proto, "TextDecoder", 0, Construct { decoder: true })?;
    define_getter(
        proto,
        "encoding",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
                decoder_of(&ctx, &this).map(|decoder| decoder.borrow().codec.borrow().name())
            },
        )?,
    )?;
    define_getter(
        proto,
        "fatal",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
                decoder_of(&ctx, &this).map(|decoder| decoder.borrow().codec.borrow().fatal())
            },
        )?,
    )?;
    define_getter(
        proto,
        "ignoreBOM",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>, This(this): This<Value<'js>>| {
                decoder_of(&ctx, &this).map(|decoder| decoder.borrow().codec.borrow().ignore_bom())
            },
        )?,
    )?;
    define_method(
        proto,
        "decode",
        Function::new(
            ctx.clone(),
            |ctx: Ctx<'js>,
             This(this): This<Value<'js>>,
             input: Opt<Value<'js>>,
             options: Opt<Value<'js>>| {
                let decoder = decoder_of(&ctx, &this)?;
                decode(&ctx, &decoder, input.0, options.0)
            },
        )?,
    )?;
    define_global(ctx, "TextDecoder", class)
}

/// The `TextDecoder` that `this` is, or a `TypeError`.
fn decoder_of<'js>(ctx: &Ctx<'js>, this: &Value<'js>) -> JsResult<Class<'js, Decoder>> {
    Class::<Decoder>::from_value(this).map_err(|_| Exception::throw_type(ctx, NOT_A_DECODER))
}

/// Where the bytes of a buffer source are, found before any option is read.
enum Source<'js> {
    Empty,
    /// An `ArrayBuffer` or a `SharedArrayBuffer`.
    Whole(Value<'js>),
    /// A typed array and its buffer.
    Typed(Value<'js>, Value<'js>),
    /// A `DataView` and its buffer.
    Data(Value<'js>, Value<'js>),
}

/// Finds the buffer of `input` and checks it can be read.
fn source_of<'js>(
    ctx: &Ctx<'js>,
    store: &Object<'js>,
    input: Option<Value<'js>>,
) -> JsResult<Source<'js>> {
    let wrong = || Exception::throw_type(ctx, NOT_A_BUFFER);
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
fn bytes_of<'js>(store: &Object<'js>, source: &Source<'js>) -> JsResult<Vec<u8>> {
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

fn decode<'js>(
    ctx: &Ctx<'js>,
    decoder: &Class<'js, Decoder>,
    input: Option<Value<'js>>,
    options: Option<Value<'js>>,
) -> JsResult<String> {
    let store: Object = vault(ctx)?.get(STORE)?;
    let source = source_of(ctx, &store, input)?;
    let stream = flag_of(options_of(ctx, options)?.as_ref(), "stream")?;
    let bytes = bytes_of(&store, &source)?;
    let result = decoder.borrow().codec.borrow_mut().push(&bytes, stream);
    result.map_err(|_| Exception::throw_type(ctx, NOT_VALID))
}

/// The U+FEFF a decoder drops from the start of its output.
const BOM: char = '\u{FEFF}';

/// Input that is not valid for the encoding, with `fatal` set.
#[derive(Debug, PartialEq, Eq)]
struct Malformed;

/// The state of one `TextDecoder`: its encoding, its flags and the decoder that holds the
/// bytes of a character split over calls.
pub(super) struct Codec {
    encoding: &'static Encoding,
    decoder: encoding_rs::Decoder,
    fatal: bool,
    ignore_bom: bool,
    /// Whether the output of the current stream has produced its first item.
    bom_seen: bool,
}

impl Codec {
    /// The codec for `label`, or `None` for an unknown label and for a label of the `replacement`
    /// encoding.
    pub(super) fn for_label(label: &str, fatal: bool, ignore_bom: bool) -> Option<Self> {
        let encoding = Encoding::for_label_no_replacement(label.as_bytes())?;
        Some(Self {
            encoding,
            decoder: encoding.new_decoder_without_bom_handling(),
            fatal,
            ignore_bom,
            bom_seen: false,
        })
    }

    /// The name of the encoding in ASCII lower case.
    fn name(&self) -> String {
        self.encoding.name().to_ascii_lowercase()
    }

    fn fatal(&self) -> bool {
        self.fatal
    }

    fn ignore_bom(&self) -> bool {
        self.ignore_bom
    }

    /// Decodes `bytes`. With `stream` the decoder keeps the bytes of an unfinished character for
    /// the next call. Without it the call flushes: an unfinished character is malformed and the
    /// next call starts a new stream.
    fn push(&mut self, bytes: &[u8], stream: bool) -> Result<String, Malformed> {
        let mut out = String::new();
        let decoded = if self.fatal {
            let size = self
                .decoder
                .max_utf8_buffer_length_without_replacement(bytes.len());
            out.reserve(size.unwrap_or_default());
            let (result, _) = self
                .decoder
                .decode_to_string_without_replacement(bytes, &mut out, !stream);
            result == DecoderResult::InputEmpty
        } else {
            out.reserve(
                self.decoder
                    .max_utf8_buffer_length(bytes.len())
                    .unwrap_or_default(),
            );
            let (result, _, _) = self.decoder.decode_to_string(bytes, &mut out, !stream);
            result == encoding_rs::CoderResult::InputEmpty
        };
        if !decoded {
            self.reset();
            return Err(Malformed);
        }
        self.apply_bom_rule(&mut out);
        if !stream {
            self.reset();
        }
        Ok(out)
    }

    /// Starts a new stream: a new decoder with no pending bytes, and the first item not yet seen.
    fn reset(&mut self) {
        self.decoder = self.encoding.new_decoder_without_bom_handling();
        self.bom_seen = false;
    }

    /// Drops the first U+FEFF of a UTF-8 or UTF-16 stream once, unless `ignore_bom` is set.
    fn apply_bom_rule(&mut self, out: &mut String) {
        let strips = [UTF_8, UTF_16LE, UTF_16BE].contains(&self.encoding);
        let first_item = !out.is_empty();
        if strips && !self.ignore_bom && !self.bom_seen && out.starts_with(BOM) {
            out.remove(0);
        }
        self.bom_seen |= first_item;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::Engine;
    use crate::encoding::NOT_AN_OBJECT;
    use crate::tests::{around, run, thrown};

    /// A codec for `label` with the default flags.
    fn codec(label: &str) -> Codec {
        Codec::for_label(label, false, false).unwrap()
    }

    /// Decodes `bytes` in one flushing call.
    fn decode(label: &str, bytes: &[u8]) -> String {
        codec(label).push(bytes, false).unwrap()
    }

    #[rstest]
    #[case::decode("TextDecoder.prototype.decode.call({})")]
    #[case::encoding(
        "Object.getOwnPropertyDescriptor(TextDecoder.prototype, 'encoding').get.call({})"
    )]
    #[case::fatal("Object.getOwnPropertyDescriptor(TextDecoder.prototype, 'fatal').get.call({})")]
    #[case::ignore_bom(
        "Object.getOwnPropertyDescriptor(TextDecoder.prototype, 'ignoreBOM').get.call({})"
    )]
    fn a_decoder_member_on_a_wrong_receiver_throws(#[case] call: &str) {
        assert_eq!(
            thrown(call),
            "TypeError: the receiver is not a TextDecoder. call the method on an instance"
        );
    }

    #[rstest]
    #[case::default("", "utf-8")]
    #[case::undefined("undefined", "utf-8")]
    #[case::latin1("'latin1'", "windows-1252")]
    fn the_decoder_encoding_is_the_name_of_its_label(#[case] label: &str, #[case] name: &str) {
        assert_eq!(run(&format!("new TextDecoder({label}).encoding")), name);
    }

    #[rstest]
    #[case::unknown("'nope'", "nope")]
    #[case::null("null", "null")]
    fn an_unknown_label_throws_a_range_error_with_the_label(
        #[case] label: &str,
        #[case] shown: &str,
    ) {
        assert_eq!(
            thrown(&format!("new TextDecoder({label})")),
            format!(
                concat!(
                    "RangeError: the encoding label \"{}\" is not known. ",
                    "use a label of the Encoding Standard"
                ),
                shown
            )
        );
    }

    #[test]
    fn a_label_with_a_lone_surrogate_throws_a_range_error() {
        assert!(thrown("new TextDecoder('\\uD800')").starts_with("RangeError"));
    }

    #[test]
    fn a_replacement_label_throws_the_unknown_label_error() {
        assert_eq!(
            thrown("new TextDecoder('iso-2022-kr')"),
            concat!(
                "RangeError: the encoding label \"iso-2022-kr\" is not known. ",
                "use a label of the Encoding Standard"
            )
        );
    }

    #[test]
    fn constructor_options_that_are_no_object_throw() {
        assert_eq!(
            thrown("new TextDecoder('utf-8', 1)"),
            format!("TypeError: {NOT_AN_OBJECT}")
        );
    }

    #[rstest]
    #[case::null("null")]
    #[case::undefined("undefined")]
    fn constructor_options_of_null_and_undefined_are_accepted(#[case] options: &str) {
        assert_eq!(
            thrown(&format!("new TextDecoder('utf-8', {options})")),
            "no throw"
        );
    }

    #[rstest]
    #[case::fatal("fatal")]
    #[case::ignore_bom("ignoreBOM")]
    fn a_decoder_flag_reads_the_constructor_value_as_a_boolean(#[case] flag: &str) {
        assert_eq!(
            run(&format!(
                "String(new TextDecoder('utf-8', {{ fatal: 1, ignoreBOM: 'x' }}).{flag})"
            )),
            "true"
        );
    }

    #[test]
    fn ignore_bom_keeps_the_bom() {
        assert_eq!(
            run(r"
                const bytes = new Uint8Array([0xEF, 0xBB, 0xBF, 0x41]);
                new TextDecoder('utf-8', { ignoreBOM: true }).decode(bytes);
            "),
            "\u{FEFF}A"
        );
    }

    #[rstest]
    #[case::no_argument("decode()")]
    #[case::undefined("decode(undefined)")]
    fn decode_without_input_returns_an_empty_string(#[case] call: &str) {
        assert_eq!(run(&format!("new TextDecoder().{call}")), "");
    }

    #[rstest]
    #[case::null("null")]
    #[case::array("[1]")]
    fn decode_rejects_input_that_is_no_buffer_source(#[case] input: &str) {
        assert_eq!(
            thrown(&format!("new TextDecoder().decode({input})")),
            format!("TypeError: {NOT_A_BUFFER}")
        );
    }

    #[rstest]
    #[case::data_view("Object.create(DataView.prototype)")]
    #[case::typed_array("Object.create(Uint8Array.prototype)")]
    #[case::proxy("new Proxy(new Uint8Array(1), {})")]
    fn decode_rejects_an_object_without_the_brand_of_a_view(#[case] input: &str) {
        assert_eq!(
            thrown(&format!("new TextDecoder().decode({input})")),
            format!("TypeError: {NOT_A_BUFFER}")
        );
    }

    #[test]
    fn decode_returns_an_empty_string_when_the_stream_getter_detaches_the_input() {
        assert_eq!(
            run(r"
                const u = new Uint8Array([97]);
                new TextDecoder().decode(u, {
                    get stream() {
                        u.buffer.transfer();
                        return false;
                    },
                });
            "),
            ""
        );
    }

    #[test]
    fn decode_converts_the_input_before_it_reads_the_options() {
        assert_eq!(
            run(r"
                let read = false;
                try {
                    new TextDecoder().decode(5, {
                        get stream() {
                            read = true;
                            return false;
                        },
                    });
                } catch (e) {
                    // The throw is the expected outcome.
                }
                String(read);
            "),
            "false"
        );
    }

    #[test]
    fn decode_options_that_are_no_object_throw() {
        assert_eq!(
            thrown("new TextDecoder().decode(new Uint8Array(1), 1)"),
            format!("TypeError: {NOT_AN_OBJECT}")
        );
    }

    #[rstest]
    #[case::array_buffer("new Uint8Array([65, 66]).buffer")]
    #[case::shared_array_buffer(
        "new Uint8Array(new SharedArrayBuffer(2)).fill(65, 0, 1).fill(66, 1).buffer"
    )]
    #[case::uint16_array("new Uint16Array([0x4241])")]
    #[case::view_with_offset("new Uint8Array(new Uint8Array([64, 65, 66, 67]).buffer, 1, 2)")]
    #[case::view_over_a_larger_buffer(
        "new Uint8Array(new Uint8Array([65, 66, 67, 68]).buffer, 0, 2)"
    )]
    #[case::data_view_with_offset("new DataView(new Uint8Array([64, 65, 66, 67]).buffer, 1, 2)")]
    fn decode_reads_the_bytes_of_a_buffer_source(#[case] input: &str) {
        assert_eq!(run(&format!("new TextDecoder().decode({input})")), "AB");
    }

    #[test]
    fn decode_reads_a_data_view_through_the_intrinsics() {
        assert_eq!(
            run(r"
                const v = new DataView(new Uint8Array([0x41, 0x42, 0x43]).buffer, 1, 1);
                for (const key of ['buffer', 'byteOffset', 'byteLength']) {
                    const value = key === 'buffer' ? new ArrayBuffer(0) : 0;
                    Object.defineProperty(v, key, { value });
                }
                new TextDecoder().decode(v);
            "),
            "B"
        );
    }

    #[rstest]
    #[case::array_buffer("b")]
    #[case::typed_array("new Uint8Array(b)")]
    #[case::data_view("new DataView(b)")]
    fn decode_of_a_detached_input_returns_an_empty_string(#[case] input: &str) {
        assert_eq!(
            run(&format!(
                r"
                const b = new ArrayBuffer(4);
                const input = {input};
                b.transfer();
                new TextDecoder().decode(input);
                "
            )),
            ""
        );
    }

    #[rstest]
    #[case::fixed_typed("new Uint8Array(rab, 0, 2)")]
    #[case::fixed_data_view("new DataView(rab, 0, 2)")]
    #[case::buffer("rab")]
    #[case::growable_buffer("new SharedArrayBuffer(4, { maxByteLength: 4 })")]
    fn decode_of_a_view_over_a_resizable_buffer_throws(#[case] input: &str) {
        assert_eq!(
            thrown(&format!(
                r"
                const rab = new ArrayBuffer(4, {{ maxByteLength: 8 }});
                new TextDecoder().decode({input});
                "
            )),
            format!("TypeError: {NOT_A_BUFFER}")
        );
    }

    #[rstest]
    #[case::buffer("rab")]
    #[case::view("u")]
    fn decode_of_a_detached_resizable_buffer_throws(#[case] input: &str) {
        assert_eq!(
            thrown(&format!(
                r"
                const rab = new ArrayBuffer(4, {{ maxByteLength: 8 }});
                const u = new Uint8Array(rab);
                rab.transfer();
                new TextDecoder().decode({input})
                "
            )),
            format!("TypeError: {NOT_A_BUFFER}")
        );
    }

    #[test]
    fn a_fatal_throw_resets_the_stream() {
        assert_eq!(
            run(r"
                const d = new TextDecoder('utf-8', { fatal: true });
                d.decode(new Uint8Array([0x41]), { stream: true });
                try {
                    d.decode(new Uint8Array([0xff]), { stream: true });
                } catch (e) {
                    // The throw is the expected outcome.
                }
                d.decode(new Uint8Array([0xEF, 0xBB, 0xBF, 0x42]));
            "),
            "B"
        );
    }

    #[test]
    fn a_fatal_throw_names_the_encoding_error() {
        assert_eq!(
            thrown("new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array([0xff]))"),
            format!("TypeError: {NOT_VALID}")
        );
    }

    #[test]
    fn decode_with_stream_joins_a_character_split_over_calls() {
        assert_eq!(
            run(r"
                const d = new TextDecoder();
                const first = d.decode(new Uint8Array([0xE3]), { stream: true });
                const second = d.decode(new Uint8Array([0x81, 0x82]));
                JSON.stringify([first, second]);
            "),
            "[\"\",\"あ\"]"
        );
    }

    #[test]
    fn decode_returns_a_copy_of_the_input() {
        // The decoder copies the bytes it reads and keeps no reference to the buffer, so a later
        // write to the buffer cannot reach the string.
        assert_eq!(
            run(r"
                const u = new Uint8Array([65, 66]);
                const text = new TextDecoder().decode(u);
                u.fill(0);
                text;
            "),
            "AB"
        );
    }

    #[test]
    fn decode_with_stream_copies_the_pending_bytes() {
        // The stream state holds the pending bytes by value and no reference to the buffer, so a
        // later write to the buffer cannot reach the next call.
        assert_eq!(
            run(r"
                const d = new TextDecoder();
                const first = new Uint8Array([0xE3]);
                d.decode(first, { stream: true });
                first.fill(0);
                d.decode(new Uint8Array([0x81, 0x82]));
            "),
            "あ"
        );
    }

    #[test]
    fn two_decoders_keep_separate_stream_state() {
        assert_eq!(
            run(r"
                const a = new TextDecoder();
                const b = new TextDecoder();
                a.decode(new Uint8Array([0xE3]), { stream: true });
                b.decode(new Uint8Array([0x41]), { stream: true });
                a.decode(new Uint8Array([0x81, 0x82]));
            "),
            "あ"
        );
    }

    #[test]
    fn a_decoder_abandoned_with_pending_bytes_leaves_the_counters_at_the_baseline() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                "new TextDecoder().decode(new Uint8Array([0xE3]), { stream: true });",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }

    #[test]
    fn an_array_buffer_passed_to_decode_is_collectable_after_the_call() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>("globalThis.d = new TextDecoder();")
            .unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>("d.decode(new ArrayBuffer(16));").unwrap();
        });

        assert_eq!(before, after);
    }

    #[test]
    fn decode_works_on_an_instance_whose_prototype_changed() {
        assert_eq!(
            run(r"
                const d = Object.setPrototypeOf(new TextDecoder(), {});
                TextDecoder.prototype.decode.call(d, new Uint8Array([65]));
            "),
            "A"
        );
    }

    #[rstest]
    #[case::replaced(false, Ok("\u{FFFD}".to_owned()))]
    #[case::fatal(true, Err(Malformed))]
    fn a_flush_with_a_truncated_tail_is_malformed(
        #[case] fatal: bool,
        #[case] expected: Result<String, Malformed>,
    ) {
        let mut codec = Codec::for_label("utf-8", fatal, false).unwrap();

        assert_eq!(codec.push(&[0xE3, 0x81], false), expected);
    }

    #[test]
    fn a_push_after_a_flush_starts_a_new_stream() {
        let mut codec = codec("utf-8");
        codec.push(&[0xE3], false).unwrap();

        assert_eq!(codec.push(&[0x41], false).unwrap(), "A");
    }

    #[rstest]
    #[case::utf8("utf-8", &[0xEF, 0xBB, 0xBF, 0x41])]
    #[case::utf16le("utf-16le", &[0xFF, 0xFE, 0x41, 0x00])]
    #[case::utf16be("utf-16be", &[0xFE, 0xFF, 0x00, 0x41])]
    fn the_bom_drops_once(#[case] label: &str, #[case] bytes: &[u8]) {
        assert_eq!(decode(label, bytes), "A");
    }

    #[test]
    fn a_bom_split_over_pushes_drops() {
        let mut codec = codec("utf-8");
        let mut out = codec.push(&[0xEF, 0xBB], true).unwrap();
        out += &codec.push(&[0xBF, 0x41], false).unwrap();

        assert_eq!(out, "A");
    }

    #[test]
    fn a_bom_after_the_first_output_stays() {
        let mut codec = codec("utf-8");
        let mut out = codec.push(&[0x41], true).unwrap();
        out += &codec.push(&[0xEF, 0xBB, 0xBF], false).unwrap();

        assert_eq!(out, "A\u{FEFF}");
    }

    #[test]
    fn a_bom_alone_in_the_first_push_counts_as_the_first_item() {
        let mut codec = codec("utf-8");
        let mut out = codec.push(&[0xEF, 0xBB, 0xBF], true).unwrap();
        out += &codec.push(&[0xEF, 0xBB, 0xBF], false).unwrap();

        assert_eq!(out, "\u{FEFF}");
    }

    #[test]
    fn the_next_stream_drops_its_own_bom() {
        let mut codec = codec("utf-8");
        codec.push(&[0x41], false).unwrap();

        assert_eq!(codec.push(&[0xEF, 0xBB, 0xBF, 0x42], false).unwrap(), "B");
    }

    #[test]
    fn the_bom_rule_leaves_a_u_feff_of_another_encoding() {
        assert_eq!(decode("gb18030", &[0x84, 0x31, 0x95, 0x33]), "\u{FEFF}");
    }
}
