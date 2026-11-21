// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The natives for element names: `createElement`, `createElementNS` and `attributeKey`.

use std::rc::Rc;

use rquickjs::{Array, Ctx, Exception, Result as JsResult};

use inca_document::{NameError, is_element_local_name, validate_and_extract};
use inca_node::{NodeId, VirtualNode};

use super::attributes::folded_key;
use super::convert::{JsNodeId, or_null, throw_tree_error};
use super::host::Host;

/// Creates the element `local_name` names, lowered in ASCII, and gives its id and tag. A name
/// that is no valid element local name throws `InvalidCharacterError`.
pub(super) fn create_element<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, String) -> JsResult<Array<'js>> {
    move |ctx, local_name| {
        if !is_element_local_name(&local_name) {
            return Err(Exception::throw_dom(
                &ctx,
                "InvalidCharacterError",
                "the element name is not valid. pass a name such as div",
            ));
        }
        let tag = local_name.to_ascii_lowercase();
        let id = host
            .document
            .borrow_mut()
            .tree
            .create_node(tag.clone())
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let pair = Array::new(ctx)?;
        pair.set(0, id)?;
        pair.set(1, tag)?;
        Ok(pair)
    }
}

/// Creates the element `qualified_name` names in `namespace` and gives `[id, localName, namespace,
/// prefix, tagName]`. The name keeps the case it has. An empty `namespace` reads as none. A bad
/// name throws `InvalidCharacterError` and a prefix the namespace does not allow throws
/// `NamespaceError`.
pub(super) fn create_element_ns<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, Option<String>, String) -> JsResult<Array<'js>> {
    move |ctx, namespace, qualified_name| {
        let namespace = namespace.filter(|text| !text.is_empty());
        let (prefix, local_name) = validate_and_extract(namespace.as_deref(), &qualified_name)
            .map_err(|err| throw_name_error(&ctx, err))?;
        let mut host = host.document.borrow_mut();
        let id = host
            .tree
            .create_element_ns(namespace.as_deref(), prefix, local_name)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        match host.tree.get(id) {
            Some(node) => element_entry(&ctx, id, node),
            None => Err(Exception::throw_type(&ctx, "the new element is missing")),
        }
    }
}

/// The key an attribute called `name` has on the node `node_id`.
pub(super) fn attribute_key<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<String> {
    move |ctx, JsNodeId(node_id), name| folded_key(&ctx, &host.document.borrow(), node_id, &name)
}

/// `[id, localName, namespace, prefix, tagName]` of the element `node`.
pub(super) fn element_entry<'js>(
    ctx: &Ctx<'js>,
    id: NodeId,
    node: &VirtualNode,
) -> JsResult<Array<'js>> {
    let entry = Array::new(ctx.clone())?;
    entry.set(0, id)?;
    entry.set(1, node.local_name())?;
    entry.set(2, or_null(ctx, node.namespace_uri())?)?;
    entry.set(3, or_null(ctx, node.prefix())?)?;
    entry.set(4, node.tag_name().into_owned())?;
    Ok(entry)
}

fn throw_name_error(ctx: &Ctx<'_>, err: NameError) -> rquickjs::Error {
    let (name, message) = match err {
        NameError::InvalidCharacter => (
            "InvalidCharacterError",
            "the element name is not valid. pass a name such as div or svg:g",
        ),
        NameError::Namespace => (
            "NamespaceError",
            "the prefix does not fit the namespace. pass the namespace the prefix belongs to",
        ),
    };
    Exception::throw_dom(ctx, name, message)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{HTML, SVG, engine_with_bindings, error_name};

    const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

    #[rstest]
    #[case::upper("DIV", "div")]
    #[case::colon_first(":a", ":a")]
    #[case::underscore_first("_a", "_a")]
    #[case::colon_inside("a:b", "a:b")]
    #[case::non_ascii_stays("\u{c9}", "\u{c9}")]
    #[case::underscore_then_name_characters("_a-1.b", "_a-1.b")]
    #[case::underscore_then_a_non_xml_character("_\u{d7}", "_\u{d7}")]
    fn a_valid_name_creates_an_element_with_the_lowered_tag(#[case] name: &str, #[case] tag: &str) {
        let (engine, host) = engine_with_bindings();

        engine
            .eval::<()>(&format!(
                "globalThis.pair = __inca_native__.createElement('{name}');"
            ))
            .unwrap();
        let id: u32 = engine.eval("pair[0]").unwrap();
        let given: String = engine.eval("pair[1]").unwrap();

        assert_eq!(
            host.document.borrow().tree.get(id).unwrap().local_name(),
            tag
        );
        assert_eq!(given, tag);
    }

    #[rstest]
    #[case::empty("")]
    #[case::hash("#document")]
    #[case::space("a b")]
    #[case::greater_than("a>")]
    #[case::slash("a/")]
    #[case::nul("a\\u0000")]
    #[case::colon_first_then_a_space(":a b")]
    fn an_invalid_name_throws_invalid_character_error(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, &format!("__inca_native__.createElement('{name}')")),
            "InvalidCharacterError"
        );
    }

    #[test]
    fn create_element_ns_gives_the_five_part_entry_with_the_case_given() {
        let (engine, _host) = engine_with_bindings();

        let entry: Vec<Option<String>> = engine
            .eval(&format!(
                "__inca_native__.createElementNS('{SVG}', 's:Foo').map(String)"
            ))
            .unwrap();

        assert_eq!(
            entry[1..],
            [
                Some("Foo".into()),
                Some(SVG.into()),
                Some("s".into()),
                Some("s:Foo".into())
            ]
        );
    }

    #[test]
    fn create_element_ns_creates_the_node_the_entry_names() {
        let (engine, host) = engine_with_bindings();

        let id: u32 = engine
            .eval("__inca_native__.createElementNS(null, 'a')[0]")
            .unwrap();

        assert_eq!(
            host.document.borrow().tree.get(id).unwrap().local_name(),
            "a"
        );
    }

    #[test]
    fn create_element_ns_gives_the_xml_prefix_and_local_name() {
        let (engine, _host) = engine_with_bindings();

        let entry: String = engine
            .eval(&format!(
                "JSON.stringify(__inca_native__.createElementNS('{XML_NS}', 'xml:a').slice(1, 4))"
            ))
            .unwrap();

        assert_eq!(entry, format!(r#"["a","{XML_NS}","xml"]"#));
    }

    #[test]
    fn create_element_ns_gives_null_for_a_missing_namespace_and_prefix() {
        let (engine, _host) = engine_with_bindings();

        let strict: bool = engine
            .eval(
                r"
                const entry = __inca_native__.createElementNS(null, 'a');
                entry[2] === null && entry[3] === null;
                ",
            )
            .unwrap();

        assert!(strict);
    }

    #[rstest]
    #[case::undefined("undefined")]
    #[case::null("null")]
    #[case::empty("''")]
    fn create_element_ns_reads_a_missing_namespace_as_none(#[case] namespace: &str) {
        let (engine, _host) = engine_with_bindings();

        let entry: String = engine
            .eval(&format!(
                "JSON.stringify(__inca_native__.createElementNS({namespace}, 'a').slice(1))"
            ))
            .unwrap();

        assert_eq!(entry, r#"["a",null,null,"a"]"#);
    }

    #[rstest]
    #[case::bad_name("'urn:x'", "a:1", "InvalidCharacterError")]
    #[case::prefix_without_namespace("null", "p:q", "NamespaceError")]
    #[case::prefix_with_empty_namespace("''", "p:q", "NamespaceError")]
    fn create_element_ns_throws_the_error_of_the_name_check(
        #[case] namespace: &str,
        #[case] name: &str,
        #[case] error: &str,
    ) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(
                &engine,
                &format!("__inca_native__.createElementNS({namespace}, '{name}')")
            ),
            error
        );
    }

    #[test]
    fn create_element_ns_keeps_the_text_after_the_first_colon_as_the_local_name() {
        let (engine, _host) = engine_with_bindings();

        let given: String = engine
            .eval("__inca_native__.createElementNS('urn:x', 'a:b:c')[1]")
            .unwrap();

        assert_eq!(given, "b:c");
    }

    #[test]
    fn create_element_keeps_a_colon_name_whole() {
        let (engine, _host) = engine_with_bindings();

        let pair: String = engine
            .eval("JSON.stringify(__inca_native__.createElement('A:1').slice(1))")
            .unwrap();

        assert_eq!(pair, r#"["a:1"]"#);
    }

    #[test]
    fn attribute_key_follows_the_namespace_of_the_node() {
        let (engine, _host) = engine_with_bindings();

        let given: String = engine
            .eval(&format!(
                "__inca_native__.attributeKey(__inca_native__.createElementNS('{HTML}', 'e')[0], 'viewBox')"
            ))
            .unwrap();

        assert_eq!(given, "viewbox");
    }

    #[test]
    fn attribute_key_of_a_freed_id_throws() {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, "__inca_native__.attributeKey(987654, 'a')"),
            "TypeError"
        );
    }
}
