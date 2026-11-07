// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `TextEncoder` and `TextDecoder` classes.
//!
//! Each instance owns plain Rust data: a `Codec` for a decoder and nothing for an encoder. The
//! prototypes and the getters that classify a buffer sit in the vault, so a script that
//! replaces a property of `ArrayBuffer`, `DataView` or an instance changes nothing here.
//! The `encoder` module writes UTF-8 from the WTF-8 form of a string and the `decoder` module
//! decodes bytes. This module installs both classes and converts the shared arguments.

use std::cell::RefCell;

use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, ParamRequirement, Params, This};
use rquickjs::{Class, Ctx, Exception, FromJs, Function, Object, Result as JsResult, Value};

use crate::class::{NEEDS_NEW, new_target_prototype};
use crate::quickjs::{is_array_buffer, string_wtf8};
use crate::vault::vault;

mod decoder;
mod encoder;

use decoder::{Codec, Decoder};
use encoder::Encoder;
pub(crate) use encoder::encode;

/// The vault key of the prototypes and the getters.
const STORE: &str = "encoding";

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

const NOT_AN_OBJECT: &str = "the options are not an object. pass an object or leave them out";

/// Defines the globals `TextEncoder` and `TextDecoder` and keeps their prototypes and the buffer
/// getters in the vault of `ctx`'s realm.
///
/// # Errors
///
/// Returns an error if the realm lacks an intrinsic or has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let store: Object = ctx.eval(GETTERS_JS)?;
    let encoder_proto = Object::new(ctx.clone())?;
    let decoder_proto = Object::new(ctx.clone())?;
    store.set("encoder", encoder_proto.clone())?;
    store.set("decoder", decoder_proto.clone())?;
    vault(ctx)?.set(STORE, store)?;
    encoder::define_encoder(ctx, &encoder_proto)?;
    decoder::define_decoder(ctx, &decoder_proto)
}

/// The body of both constructors.
struct Construct {
    decoder: bool,
}

impl<'js> IntoJsFunc<'js, ()> for Construct {
    fn param_requirements() -> ParamRequirement {
        ParamRequirement::any()
    }

    fn call<'a>(&self, params: Params<'a, 'js>) -> JsResult<Value<'js>> {
        let ctx = params.ctx();
        if !params.is_constructor() {
            return Err(Exception::throw_type(ctx, NEEDS_NEW));
        }
        let store: Object = vault(ctx)?.get(STORE)?;
        if !self.decoder {
            let proto = new_target_prototype(&params, store.get("encoder")?)?;
            return Ok(Class::instance_proto(Encoder, proto)?.into_value());
        }
        let label = match params.arg(0) {
            Some(label) if !label.is_undefined() => string_wtf8(&label)?,
            _ => b"utf-8".to_vec(),
        };
        let options = options_of(ctx, params.arg(1))?;
        let flag = |key| flag_of(options.as_ref(), key);
        let (fatal, ignore_bom) = (flag("fatal")?, flag("ignoreBOM")?);
        let codec = std::str::from_utf8(&label)
            .ok()
            .and_then(|label| Codec::for_label(label, fatal, ignore_bom))
            .ok_or_else(|| {
                Exception::throw_range(
                    ctx,
                    &format!(
                        concat!(
                            "the encoding label \"{}\" is not known. ",
                            "use a label of the Encoding Standard"
                        ),
                        String::from_utf8_lossy(&label)
                    ),
                )
            })?;
        let proto = new_target_prototype(&params, store.get("decoder")?)?;
        let decoder = Decoder {
            codec: RefCell::new(codec),
        };
        Ok(Class::instance_proto(decoder, proto)?.into_value())
    }
}

/// The object of a dictionary argument: `None` for an absent, `undefined` or `null` value.
fn options_of<'js>(ctx: &Ctx<'js>, value: Option<Value<'js>>) -> JsResult<Option<Object<'js>>> {
    match value {
        Some(value) if !value.is_undefined() && !value.is_null() => value
            .into_object()
            .map(Some)
            .ok_or_else(|| Exception::throw_type(ctx, NOT_AN_OBJECT)),
        _ => Ok(None),
    }
}

/// The member `key` of a dictionary converted with `ToBoolean`, `false` when absent.
fn flag_of(options: Option<&Object<'_>>, key: &str) -> JsResult<bool> {
    options.map_or(Ok(false), |options| {
        options.get::<_, Coerced<bool>>(key).map(|flag| flag.0)
    })
}

/// Calls the getter `key` of the store on `this`.
fn read<'js, T: FromJs<'js>>(store: &Object<'js>, key: &str, this: &Value<'js>) -> JsResult<T> {
    store.get::<_, Function>(key)?.call((This(this.clone()),))
}

/// Whether the getter `resizable` of an `ArrayBuffer` or `growable` of a `SharedArrayBuffer`
/// says `buffer` can change its size.
fn is_resizable<'js>(store: &Object<'js>, buffer: &Value<'js>) -> JsResult<bool> {
    let key = if is_array_buffer(buffer) {
        "resizable"
    } else {
        "growable"
    };
    read(store, key, buffer)
}

/// Whether `buffer` is a detached `ArrayBuffer`. A `SharedArrayBuffer` cannot detach.
fn is_detached<'js>(store: &Object<'js>, buffer: &Value<'js>) -> JsResult<bool> {
    if is_array_buffer(buffer) {
        read(store, "detached", buffer)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::Engine;
    use crate::tests::{around, run, thrown};

    #[rstest]
    #[case::encoder("TextEncoder")]
    #[case::decoder("TextDecoder")]
    fn the_global_is_a_function(#[case] name: &str) {
        assert_eq!(run(&format!("typeof {name}")), "function");
    }

    #[rstest]
    #[case::encoder("TextEncoder")]
    #[case::decoder("TextDecoder")]
    fn the_global_is_writable_and_configurable_and_not_enumerable(#[case] name: &str) {
        let flags = run(&format!(
            r"
            const d = Object.getOwnPropertyDescriptor(globalThis, '{name}');
            [d.writable, d.enumerable, d.configurable].join();
            "
        ));

        assert_eq!(flags, "true,false,true");
    }

    #[rstest]
    #[case::encoder("TextEncoder")]
    #[case::decoder("TextDecoder")]
    fn the_constructor_has_length_zero(#[case] name: &str) {
        assert_eq!(run(&format!("String({name}.length)")), "0");
    }

    #[rstest]
    #[case::encoder("TextEncoder")]
    #[case::decoder("TextDecoder")]
    fn an_instance_has_the_string_tag_of_its_class(#[case] name: &str) {
        assert_eq!(
            run(&format!("Object.prototype.toString.call(new {name}())")),
            format!("[object {name}]")
        );
    }

    #[rstest]
    #[case::encode("TextEncoder", "encode", "0")]
    #[case::encode_into("TextEncoder", "encodeInto", "2")]
    #[case::decode("TextDecoder", "decode", "0")]
    fn a_method_has_its_length(#[case] class: &str, #[case] method: &str, #[case] length: &str) {
        assert_eq!(
            run(&format!("String({class}.prototype.{method}.length)")),
            length
        );
    }

    #[rstest]
    #[case::encoder("TextEncoder")]
    #[case::decoder("TextDecoder")]
    fn calling_the_constructor_without_new_throws(#[case] name: &str) {
        assert_eq!(
            thrown(&format!("{name}()")),
            format!("TypeError: {NEEDS_NEW}")
        );
    }

    #[test]
    fn creating_and_dropping_instances_leaves_the_counters_at_the_baseline() {
        let engine = Engine::new().unwrap();
        let (before, after) = around(&engine, |ctx| {
            ctx.eval::<(), _>(
                r"
                new TextDecoder('shift_jis', { fatal: true }).decode(new Uint8Array([0x82, 0xA0]));
                new TextEncoder().encode('a');
                ",
            )
            .unwrap();
        });

        assert_eq!(before, after);
    }

    #[rstest]
    #[case::encoder_instance("TextEncoder", "String(new A() instanceof A)", "true")]
    #[case::decoder_instance("TextDecoder", "String(new A() instanceof A)", "true")]
    fn a_subclass_keeps_its_own_prototype(
        #[case] base: &str,
        #[case] expression: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                class A extends {base} {{
                    hello() {{
                        return 'hi';
                    }}
                }}
                {expression};
                "
            )),
            expected
        );
    }

    #[rstest]
    #[case::encoder("TextEncoder", "r.encode('a').join()", "97")]
    #[case::decoder("TextDecoder", "r.decode(new Uint8Array([65]))", "A")]
    fn a_target_without_a_prototype_object_gives_the_class_prototype(
        #[case] class: &str,
        #[case] expression: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            run(&format!(
                r"
                const target = function () {{}};
                target.prototype = null;
                const r = Reflect.construct({class}, [], target);
                const same = Object.getPrototypeOf(r) === {class}.prototype;
                same + '|' + {expression};
                "
            )),
            format!("true|{expected}")
        );
    }
}
