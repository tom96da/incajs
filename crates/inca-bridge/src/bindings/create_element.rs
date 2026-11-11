// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The natives for element names: `createElement`, `createElementNS` and `attributeKey`.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Array, Ctx, Exception, Result as JsResult};

use inca_node::{NodeId, VirtualNode};

use super::attributes::folded_key;
use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;

/// The namespace the `xml` prefix is bound to.
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
/// The namespace of `xmlns` names.
const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";

/// Creates the element `local_name` names, lowered in ASCII, and gives its id and tag. A name
/// that is no valid element local name throws `InvalidCharacterError`.
pub(super) fn create_element<'js>(
    host: Rc<RefCell<Host>>,
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
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, Option<String>, String) -> JsResult<Array<'js>> {
    move |ctx, namespace, qualified_name| {
        let namespace = namespace.filter(|text| !text.is_empty());
        let (prefix, local_name) = validate_and_extract(namespace.as_deref(), &qualified_name)
            .map_err(|err| err.throw(&ctx))?;
        let mut host = host.borrow_mut();
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
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<String> {
    move |ctx, JsNodeId(node_id), name| folded_key(&ctx, &host.borrow(), node_id, &name)
}

/// `[id, localName, namespace, prefix, tagName]` of the element `node`.
pub(super) fn element_entry<'js>(
    ctx: &Ctx<'js>,
    id: NodeId,
    node: &VirtualNode,
) -> JsResult<Array<'js>> {
    let entry = Array::new(ctx.clone())?;
    entry.set(0, id)?;
    entry.set(1, node.tag_name())?;
    entry.set(2, node.namespace_uri())?;
    entry.set(3, node.prefix())?;
    entry.set(4, node.dom_tag_name().into_owned())?;
    Ok(entry)
}

enum NameError {
    InvalidCharacter,
    Namespace,
}

impl NameError {
    fn throw(&self, ctx: &Ctx<'_>) -> rquickjs::Error {
        let (name, message) = match self {
            Self::InvalidCharacter => (
                "InvalidCharacterError",
                "the element name is not valid. pass a name such as div or svg:g",
            ),
            Self::Namespace => (
                "NamespaceError",
                "the prefix does not fit the namespace. pass the namespace the prefix belongs to",
            ),
        };
        Exception::throw_dom(ctx, name, message)
    }
}

/// Splits `qualified_name` at its first colon into the prefix and the local name, which ends at
/// a second colon, and checks both against `namespace`.
fn validate_and_extract<'a>(
    namespace: Option<&str>,
    qualified_name: &'a str,
) -> Result<(Option<&'a str>, &'a str), NameError> {
    let (prefix, rest) = match qualified_name.split_once(':') {
        Some((prefix, rest)) => (Some(prefix), rest),
        None => (None, qualified_name),
    };
    let local_name = if prefix.is_some() {
        rest.split(':').next().unwrap_or_default()
    } else {
        rest
    };
    if !is_element_local_name(local_name) || prefix.is_some_and(|p| !is_prefix(p)) {
        return Err(NameError::InvalidCharacter);
    }
    let is_xmlns = qualified_name == "xmlns" || prefix == Some("xmlns");
    let allowed = match (prefix, namespace) {
        (Some(_), None) => false,
        (Some("xml"), namespace) if namespace != Some(XML_NS) => false,
        _ if is_xmlns => namespace == Some(XMLNS_NS),
        _ => namespace != Some(XMLNS_NS),
    };
    if allowed {
        Ok((prefix, local_name))
    } else {
        Err(NameError::Namespace)
    }
}

/// ASCII whitespace, NUL, `/` and `>`, which no part of an element name holds.
fn is_forbidden(c: char) -> bool {
    c.is_ascii_whitespace() || matches!(c, '\0' | '/' | '>')
}

/// A prefix is not empty and holds no forbidden character.
fn is_prefix(prefix: &str) -> bool {
    !prefix.is_empty() && !prefix.contains(is_forbidden)
}

/// An ASCII letter followed by no forbidden character, or a name that starts with `:`, `_` or a
/// code point from U+0080 on and continues with name characters.
fn is_element_local_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => !chars.any(is_forbidden),
        Some(':' | '_' | '\u{80}'..) => chars.all(is_name_char),
        _ => false,
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.') || c >= '\u{80}'
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::{XML_NS, XMLNS_NS};
    use crate::test_support::{HTML, SVG, engine_with_bindings, error_name};

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

        assert_eq!(host.borrow().tree.get(id).unwrap().tag_name(), tag);
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

        assert_eq!(host.borrow().tree.get(id).unwrap().tag_name(), "a");
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
    #[case::second_colon_ends_the_name("a:b:c", "b")]
    #[case::third_colon_is_dropped("a:b:c:d", "b")]
    #[case::trailing_colon("a:b:", "b")]
    #[case::angle_in_local("a:b<", "b<")]
    fn create_element_ns_ends_the_local_name_at_a_second_colon(
        #[case] name: &str,
        #[case] local: &str,
    ) {
        let (engine, _host) = engine_with_bindings();

        let given: String = engine
            .eval(&format!(
                "__inca_native__.createElementNS('urn:x', '{name}')[1]"
            ))
            .unwrap();

        assert_eq!(given, local);
    }

    #[rstest]
    #[case::double_colon("a::b")]
    #[case::digit_local("a:1")]
    #[case::empty_prefix(":a")]
    #[case::empty_local("a:")]
    #[case::digit_first("1a")]
    #[case::space("a b")]
    #[case::empty("")]
    #[case::dash_first_local("a:-b")]
    #[case::space_in_prefix("a b:c")]
    #[case::angle_in_prefix("x>y:a")]
    #[case::slash_in_prefix("x/y:a")]
    #[case::nul_in_prefix("x\\u0000y:a")]
    fn create_element_ns_throws_invalid_character_error_for_a_bad_name(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(
                &engine,
                &format!("__inca_native__.createElementNS('urn:x', '{name}')")
            ),
            "InvalidCharacterError"
        );
    }

    #[rstest]
    #[case::parenthesis_after_a_letter("a(")]
    #[case::dot_after_a_letter("a.b")]
    #[case::dash_after_a_letter("a-b")]
    #[case::non_ascii("\u{c9}")]
    fn create_element_ns_accepts_a_name_without_a_prefix(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        let prefix: Option<String> = engine
            .eval(&format!(
                "__inca_native__.createElementNS(null, '{name}')[3]"
            ))
            .unwrap();

        assert_eq!(prefix, None);
    }

    #[rstest]
    #[case::digit_first("1:a")]
    #[case::dash_first("-x:a")]
    #[case::dot_first(".x:a")]
    #[case::equals_sign("x=y:a")]
    #[case::non_ascii("\u{e9}:a")]
    fn create_element_ns_accepts_any_prefix_without_a_forbidden_character(#[case] name: &str) {
        let (engine, _host) = engine_with_bindings();

        let prefix: Option<String> = engine
            .eval(&format!(
                "__inca_native__.createElementNS('urn:x', '{name}')[3]"
            ))
            .unwrap();

        assert_eq!(
            prefix.as_deref(),
            name.split_once(':').map(|(prefix, _)| prefix)
        );
    }

    #[rstest]
    #[case::prefix_without_namespace(None, "p:q")]
    #[case::digit_prefix_without_namespace(None, "1:a")]
    #[case::xmlns_name_without_namespace(None, "xmlns")]
    #[case::prefix_with_empty_namespace(Some(""), "p:q")]
    #[case::xml_prefix_in_another_namespace(Some("urn:x"), "xml:a")]
    #[case::xmlns_prefix_in_another_namespace(Some("urn:x"), "xmlns:a")]
    #[case::xmlns_name_in_another_namespace(Some("urn:x"), "xmlns")]
    #[case::xmlns_namespace_without_xmlns(Some(XMLNS_NS), "a")]
    fn create_element_ns_throws_namespace_error_for_a_prefix_that_does_not_fit(
        #[case] namespace: Option<&str>,
        #[case] name: &str,
    ) {
        let (engine, _host) = engine_with_bindings();
        let namespace = namespace.map_or("null".to_owned(), |text| format!("'{text}'"));

        assert_eq!(
            error_name(
                &engine,
                &format!("__inca_native__.createElementNS({namespace}, '{name}')")
            ),
            "NamespaceError"
        );
    }

    #[rstest]
    #[case::xml(Some(XML_NS), "xml:a")]
    #[case::xmlns(Some(XMLNS_NS), "xmlns:a")]
    #[case::xmlns_name(Some(XMLNS_NS), "xmlns")]
    fn create_element_ns_accepts_the_reserved_prefix_in_its_namespace(
        #[case] namespace: Option<&str>,
        #[case] name: &str,
    ) {
        let (engine, _host) = engine_with_bindings();
        let namespace = namespace.map_or("null".to_owned(), |text| format!("'{text}'"));

        assert_eq!(
            error_name(
                &engine,
                &format!("__inca_native__.createElementNS({namespace}, '{name}')")
            ),
            "none"
        );
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
