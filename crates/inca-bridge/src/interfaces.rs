// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The interface objects `Document`, `ShadowRoot` and `DOMStringMap`, and the helpers that keep the
//! prototypes of the interfaces and of their iterators in the prototype store of the vault.
//!
//! All exist before the app runs. None can be constructed: the host has one document, no shadow
//! roots, and a `DOMStringMap` only as the `dataset` of an element.

use inca_jsenv::class::{define_class, define_global};
use inca_jsenv::store_prototype;
use rquickjs::{Ctx, Exception, Object, Result as JsResult};

/// Defines the global `name`, a class over `proto` whose every call throws a `TypeError` with
/// `message`.
fn define_interface<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    message: &'static str,
) -> JsResult<()> {
    let body = move |ctx: Ctx<'_>| -> JsResult<()> { Err(Exception::throw_type(&ctx, message)) };
    let class = define_class(ctx, proto, name, 0, body)?;
    define_global(ctx, name, class)
}

/// Defines the global `name` like [`define_interface`] and keeps `proto` in the prototype store
/// under `name`.
///
/// # Errors
///
/// Returns an error if defining the global or storing the prototype fails.
pub(crate) fn define_unconstructable<'js>(
    ctx: &Ctx<'js>,
    proto: &Object<'js>,
    name: &str,
    message: &'static str,
) -> JsResult<()> {
    define_interface(ctx, proto, name, message)?;
    store_prototype(ctx, name, proto)
}

/// Defines the globals `Document`, `ShadowRoot` and `DOMStringMap`.
///
/// # Errors
///
/// Returns an error if defining a global on `ctx` fails.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    define_interface(
        ctx,
        &Object::new(ctx.clone())?,
        "Document",
        "a document cannot be constructed. the host has one document",
    )?;
    define_interface(
        ctx,
        &Object::new(ctx.clone())?,
        "ShadowRoot",
        "a shadow root cannot be constructed. the host has no shadow roots",
    )?;
    define_unconstructable(
        ctx,
        &Object::new(ctx.clone())?,
        "DOMStringMap",
        "a DOMStringMap cannot be constructed. read it from dataset",
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::engine_with_bindings;
    use crate::tests::{dispatcher_with_engine, eval_json};
    use inca_jsenv::required_prototype;
    use rquickjs::Function;
    use rstest::rstest;

    const DOCUMENT: &str = "a document cannot be constructed. the host has one document";
    const SHADOW_ROOT: &str = "a shadow root cannot be constructed. the host has no shadow roots";
    const TOKEN_LIST: &str = "a token list cannot be constructed. read it from classList";
    const STYLE: &str =
        "a style declaration cannot be constructed. read it from the style of an element";
    const DOMSTRING_MAP: &str = "a DOMStringMap cannot be constructed. read it from dataset";

    #[rstest]
    #[case::document_new("new Document()", DOCUMENT)]
    #[case::document_call("Document()", DOCUMENT)]
    #[case::shadow_root_new("new ShadowRoot()", SHADOW_ROOT)]
    #[case::shadow_root_call("ShadowRoot()", SHADOW_ROOT)]
    #[case::token_list_new("new DOMTokenList()", TOKEN_LIST)]
    #[case::token_list_call("DOMTokenList()", TOKEN_LIST)]
    #[case::style_new("new CSSStyleDeclaration()", STYLE)]
    #[case::style_call("CSSStyleDeclaration()", STYLE)]
    #[case::dom_string_map_new("new DOMStringMap()", DOMSTRING_MAP)]
    #[case::dom_string_map_call("DOMStringMap()", DOMSTRING_MAP)]
    fn constructing_an_interface_throws_a_type_error(#[case] call: &str, #[case] message: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let got = eval_json(
            &dispatcher,
            &format!(
                r"
                (() => {{
                    try {{
                        {call};
                        return 'ran';
                    }} catch (err) {{
                        return [err instanceof TypeError, err.message];
                    }}
                }})()
                "
            ),
        );
        assert_eq!(got, serde_json::json!([true, message]));
    }

    #[rstest]
    #[case::document("Document")]
    #[case::shadow_root("ShadowRoot")]
    #[case::token_list("DOMTokenList")]
    #[case::style("CSSStyleDeclaration")]
    #[case::dom_string_map("DOMStringMap")]
    fn an_interface_has_length_zero(#[case] name: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        assert_eq!(
            eval_json(&dispatcher, &format!("{name}.length")),
            serde_json::json!(0)
        );
    }

    #[rstest]
    #[case::token_list("DOMTokenList")]
    #[case::style("CSSStyleDeclaration")]
    #[case::dom_string_map("DOMStringMap")]
    fn the_store_keeps_the_prototype_of_every_interface(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        let got = engine.with(|ctx| {
            let proto = required_prototype(&ctx, name).unwrap();
            let class: Function = proto.get("constructor").unwrap();
            class.get::<_, String>("name").unwrap()
        });

        assert_eq!(got, name);
    }
}
