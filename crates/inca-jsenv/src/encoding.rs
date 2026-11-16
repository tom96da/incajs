// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `TextEncoder` and `TextDecoder` classes.
//!
//! Each instance owns plain Rust data: a `Codec` for a decoder and nothing for an encoder. The
//! prototypes are kept in the vault, so a script that replaces a property of an instance changes
//! nothing here.
//! The `encoder` module writes UTF-8 from the WTF-8 form of a string and the `decoder` module
//! decodes bytes. This module installs both classes and converts the shared arguments.

use std::cell::RefCell;

use rquickjs::convert::Coerced;
use rquickjs::function::{IntoJsFunc, ParamRequirement, Params};
use rquickjs::{Class, Ctx, Exception, Object, Result as JsResult, Value};

use crate::class::{NEEDS_NEW, new_target_prototype, options_of};
use crate::quickjs::string_wtf8;
use crate::vault::{required_prototype, store_prototype};

mod decoder;
mod encoder;

use decoder::{Codec, Decoder};
use encoder::Encoder;
pub(crate) use encoder::encode;

/// The names of the prototypes in the prototype store.
const ENCODER: &str = "TextEncoder";
const DECODER: &str = "TextDecoder";

/// Defines the globals `TextEncoder` and `TextDecoder` and keeps their prototypes in the vault of
/// `ctx`'s realm.
///
/// # Errors
///
/// Returns an error if the realm has no vault.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    let encoder_proto = Object::new(ctx.clone())?;
    let decoder_proto = Object::new(ctx.clone())?;
    store_prototype(ctx, ENCODER, &encoder_proto)?;
    store_prototype(ctx, DECODER, &decoder_proto)?;
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
        if !self.decoder {
            let proto = new_target_prototype(&params, required_prototype(ctx, ENCODER)?)?;
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
        let proto = new_target_prototype(&params, required_prototype(ctx, DECODER)?)?;
        let decoder = Decoder {
            codec: RefCell::new(codec),
        };
        Ok(Class::instance_proto(decoder, proto)?.into_value())
    }
}

/// The member `key` of a dictionary converted with `ToBoolean`, `false` when absent.
fn flag_of(options: Option<&Object<'_>>, key: &str) -> JsResult<bool> {
    options.map_or(Ok(false), |options| {
        options.get::<_, Coerced<bool>>(key).map(|flag| flag.0)
    })
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
