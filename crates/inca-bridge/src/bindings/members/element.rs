// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members of `Element` that read the node itself: the name members `tagName`, `localName`,
//! `namespaceURI` and `prefix`, `matches`, the pointer capture and `childElementCount`. An element
//! the host has freed reads `""` for the first two names and `null` for the others.

use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, FromJs, IntoJs, Result as JsResult, Value};

use inca_jsenv::class::{define_this_getter, require, text};
use inca_jsenv::required_prototype;
use inca_node::VirtualNode;
use inca_style::{check_selector, matches_selector};

use super::{getter, method};
use crate::bindings::capture::{has_capture, release_capture, set_capture};
use crate::bindings::convert::or_null;
use crate::bindings::handle::{ELEMENT, PARENT_NODE, receiver_of};
use crate::bindings::host::Host;

/// Defines the name getters on `Element.prototype`.
pub(super) fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let proto = required_prototype(ctx, "Element")?;
    define_name(ctx, &proto, host, "tagName", |ctx, node| {
        node.map_or_else(String::new, |node| node.tag_name().into_owned())
            .into_js(ctx)
    })?;
    define_name(ctx, &proto, host, "localName", |ctx, node| {
        node.map_or_else(String::new, |node| node.local_name().to_owned())
            .into_js(ctx)
    })?;
    define_name(ctx, &proto, host, "namespaceURI", |ctx, node| {
        or_null(ctx, node.and_then(|node| node.namespace_uri()))
    })?;
    define_name(ctx, &proto, host, "prefix", |ctx, node| {
        or_null(ctx, node.and_then(|node| node.prefix()))
    })?;
    method(
        ctx,
        &proto,
        host,
        "matches",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            require(ctx, "matches", args, 1)?;
            let selector = text(ctx, &args[0])?;
            let mut host = host.document.borrow_mut();
            host.sync_focus_style();
            let syntax = |err: inca_style::SelectorError| {
                Exception::throw_dom(ctx, "SyntaxError", &err.to_string())
            };
            if host.tree.get(id).is_none() {
                check_selector(&selector).map_err(syntax)?;
                return Ok(false);
            }
            matches_selector(&host.tree, id, &selector).map_err(syntax)
        },
    )?;
    pointer_capture(ctx, &proto, host)?;
    for name in ["Element", "Document", "DocumentFragment"] {
        let proto = required_prototype(ctx, name)?;
        getter(
            ctx,
            &proto,
            host,
            "childElementCount",
            &PARENT_NODE,
            |_, host, id, _| {
                Ok(
                    u32::try_from(host.document.borrow().tree.child_count(id, true))
                        .unwrap_or(u32::MAX),
                )
            },
        )?;
    }
    Ok(())
}

/// The pointer id argument of a pointer capture member.
fn pointer_id<'js>(ctx: &Ctx<'js>, name: &str, args: &[Value<'js>]) -> JsResult<i32> {
    require(ctx, name, args, 1)?;
    Ok(Coerced::<i32>::from_js(ctx, args[0].clone())?.0)
}

/// Defines the pointer capture members on `proto`.
fn pointer_capture<'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<Host>,
) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "setPointerCapture",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            set_capture(ctx, host, id, pointer_id(ctx, "setPointerCapture", args)?)
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "releasePointerCapture",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            release_capture(
                ctx,
                host,
                id,
                pointer_id(ctx, "releasePointerCapture", args)?,
            )
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "hasPointerCapture",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            let pointer = pointer_id(ctx, "hasPointerCapture", args)?;
            Ok(has_capture(&host.document.borrow(), id, pointer))
        },
    )
}

/// Defines the getter `key`, which reads `read` over the node of the receiver, `None` when the
/// host has freed it.
fn define_name<'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<Host>,
    key: &str,
    read: impl Fn(&Ctx<'js>, Option<&VirtualNode>) -> JsResult<Value<'js>> + 'js,
) -> JsResult<()> {
    let host = Rc::clone(host);
    define_this_getter(ctx, proto, key, move |ctx, this: Value<'js>| {
        let (node, _, _) = receiver_of(ctx, &this, &ELEMENT)?;
        read(ctx, host.document.borrow().tree.get(node))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use serde_json::json;

    use crate::test_support::{HTML, MATHML, SVG, engine_with_bindings, member_error, member_json};

    /// `[tagName, localName, namespaceURI, prefix]` of the handle of the node `make` builds.
    fn names_of(make: &str) -> String {
        let (engine, _host) = engine_with_bindings();
        engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const el = n.nodeHandle({make})[0];
                JSON.stringify([el.tagName, el.localName, el.namespaceURI, el.prefix]);
                "
            ))
            .unwrap()
    }

    #[rstest]
    #[case::html("n.createNode('div')", format!(r#"["DIV","div","{HTML}",null]"#))]
    #[case::svg(
        &format!("n.createElementNS('{SVG}', 'foreignObject')[0]"),
        format!(r#"["foreignObject","foreignObject","{SVG}",null]"#)
    )]
    #[case::mathml(
        &format!("n.createElementNS('{MATHML}', 'mi')[0]"),
        format!(r#"["mi","mi","{MATHML}",null]"#)
    )]
    #[case::prefixed(
        &format!("n.createElementNS('{SVG}', 's:g')[0]"),
        format!(r#"["s:g","g","{SVG}","s"]"#)
    )]
    #[case::no_namespace("n.createElementNS(null, 'a')[0]", r#"["a","a",null,null]"#.to_owned())]
    #[case::html_ascii_upper_case(
        "n.createNode('straße')",
        format!(r#"["STRAßE","straße","{HTML}",null]"#)
    )]
    fn the_name_members_read_the_name_of_the_element(#[case] make: &str, #[case] expected: String) {
        assert_eq!(names_of(make), expected);
    }

    #[rstest]
    #[case::tag_name("e.tagName", json!(""))]
    #[case::local_name("e.localName", json!(""))]
    #[case::namespace_uri("e.namespaceURI", json!(null))]
    #[case::prefix("e.prefix", json!(null))]
    fn a_name_member_of_a_freed_element_reads_the_default(
        #[case] read: &str,
        #[case] expected: serde_json::Value,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            free(e);
            return {read};
            "
        ));

        assert_eq!(got, expected);
    }

    #[test]
    fn matches_of_a_freed_node_gives_false() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            free(e);
            return e.matches('div');
            ",
        );

        assert_eq!(got, json!(false));
    }

    #[test]
    fn matches_of_a_freed_node_throws_for_a_bad_selector() {
        let got = member_error(
            r"
            const e = mk('div');
            free(e);
            e.matches('div >');
            ",
        );

        assert_eq!(got, "SyntaxError");
    }

    #[test]
    fn a_focus_request_shows_in_matches_in_the_same_tick() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('tabindex', '0');
            e.focus();
            return e.matches(':focus');
            ",
        );

        assert_eq!(got, json!(true));
    }

    #[test]
    fn child_element_count_skips_text() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            n.appendChild(e.__node_id, n.createNode('span'));
            n.appendChild(e.__node_id, n.createNode('#text'));
            const doc = h(n.documentNodeId());
            return [e.childElementCount, doc.childElementCount];
            ",
        );

        assert_eq!(got, json!([1, 1]));
    }

    #[test]
    fn child_element_count_of_a_freed_node_is_zero() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            n.appendChild(e.__node_id, n.createNode('span'));
            free(e);
            return e.childElementCount;
            ",
        );

        assert_eq!(got, json!(0));
    }
}
