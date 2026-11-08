// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The interface objects `Document` and `ShadowRoot`.
//!
//! Both exist before the app runs. Neither can be constructed: the host has one document and no
//! shadow roots.

use inca_jsenv::class::{define_class, define_global};
use rquickjs::{Ctx, Exception, Object, Result as JsResult};

/// Defines the global `name`, a class whose every call throws a `TypeError` with `message`.
fn define_unconstructable(ctx: &Ctx<'_>, name: &str, message: &'static str) -> JsResult<()> {
    let proto = Object::new(ctx.clone())?;
    let body = move |ctx: Ctx<'_>| -> JsResult<()> { Err(Exception::throw_type(&ctx, message)) };
    let class = define_class(ctx, &proto, name, 0, body)?;
    define_global(ctx, name, class)
}

/// Defines the globals `Document` and `ShadowRoot`.
///
/// # Errors
///
/// Returns an error if defining a global on `ctx` fails.
pub(crate) fn install(ctx: &Ctx<'_>) -> JsResult<()> {
    define_unconstructable(
        ctx,
        "Document",
        "a document cannot be constructed. the host has one document",
    )?;
    define_unconstructable(
        ctx,
        "ShadowRoot",
        "a shadow root cannot be constructed. the host has no shadow roots",
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{dispatcher_with_engine, eval_json};
    use rstest::rstest;

    const DOCUMENT: &str = "a document cannot be constructed. the host has one document";
    const SHADOW_ROOT: &str = "a shadow root cannot be constructed. the host has no shadow roots";

    #[rstest]
    #[case::document_new("new Document()", DOCUMENT)]
    #[case::document_call("Document()", DOCUMENT)]
    #[case::shadow_root_new("new ShadowRoot()", SHADOW_ROOT)]
    #[case::shadow_root_call("ShadowRoot()", SHADOW_ROOT)]
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
    fn an_interface_has_length_zero(#[case] name: &str) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        assert_eq!(
            eval_json(&dispatcher, &format!("{name}.length")),
            serde_json::json!(0)
        );
    }
}
