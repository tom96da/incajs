// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Crypto` class and the accessor `crypto`.
//!
//! The one instance lives in the vault, so a script that replaces a property of the global
//! changes nothing here.

// `CryptoInstance` and `SubtleInstance` derive `JsLifetime`, and the classes hold no JS value.
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::{Class, Ctx, Function, Object, Result as JsResult};

use super::random;
use crate::class::{define_class, define_getter, define_global, illegal_constructor, plain_class};
use crate::vault::vault;

/// The vault key of the `crypto` object.
const CRYPTO: &str = "crypto";

/// A `Crypto` instance. It has no state.
// `CryptoInstance` has no state, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(super) struct CryptoInstance;

plain_class!(CryptoInstance, "Crypto");

/// Defines the global class `Crypto` and the getter `crypto`, whose value is the one instance.
///
/// # Errors
///
/// Returns an error if the realm has no vault or a global cannot be defined.
pub(crate) fn install<'js>(ctx: &Ctx<'js>) -> JsResult<()> {
    let proto = Object::new(ctx.clone())?;
    let class = define_class(ctx, &proto, "Crypto", 0, illegal_constructor())?;
    random::define(ctx, &proto)?;
    define_global(ctx, "Crypto", class)?;
    vault(ctx)?.set(CRYPTO, Class::instance_proto(CryptoInstance, proto)?)?;
    let get = Function::new(ctx.clone(), |ctx: Ctx<'js>| {
        vault(&ctx)?.get::<_, Object>(CRYPTO)
    })?;
    define_getter(&ctx.globals(), "crypto", get)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::tests::{run, thrown};

    #[test]
    fn the_crypto_global_is_a_getter_without_a_setter() {
        assert_eq!(
            run(r"
                const d = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
                [typeof d.get, d.get.name, d.set, d.enumerable, d.configurable].join();
                "),
            "function,get crypto,,true,true"
        );
    }

    #[test]
    fn assigning_to_crypto_is_ignored_in_sloppy_mode() {
        assert_eq!(
            run(r"
                const before = crypto;
                new Function('crypto = 1')();
                String(crypto === before);
                "),
            "true"
        );
    }

    #[test]
    fn assigning_to_crypto_throws_in_strict_mode() {
        assert_eq!(
            thrown(
                r"
                'use strict';
                crypto = 1
                "
            )
            .split(':')
            .next(),
            Some("TypeError")
        );
    }

    #[test]
    fn crypto_is_one_instance_of_crypto() {
        assert_eq!(
            run("String(crypto === crypto && crypto instanceof Crypto)"),
            "true"
        );
    }

    #[test]
    fn the_constructor_throws_illegal_constructor() {
        assert_eq!(thrown("new Crypto()"), "TypeError: illegal constructor");
    }

    #[test]
    fn the_class_global_has_the_attributes_of_a_class() {
        assert_eq!(
            run(r"
                const d = Object.getOwnPropertyDescriptor(globalThis, 'Crypto');
                [d.writable, d.enumerable, d.configurable].join();
                "),
            "true,false,true"
        );
    }

    #[rstest]
    #[case::get_random_values("getRandomValues", "1")]
    #[case::random_uuid("randomUUID", "0")]
    fn a_method_has_its_length(#[case] method: &str, #[case] length: &str) {
        assert_eq!(
            run(&format!("String(Crypto.prototype.{method}.length)")),
            length
        );
    }

    #[rstest]
    #[case::get_random_values("crypto.getRandomValues.call({}, new Uint8Array(1))")]
    #[case::random_uuid("crypto.randomUUID.call({})")]
    fn a_member_called_on_a_wrong_receiver_throws(#[case] call: &str) {
        assert_eq!(
            thrown(call),
            "TypeError: the receiver is not a Crypto. call the method on an instance"
        );
    }
}
