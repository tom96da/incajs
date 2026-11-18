// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The attribute members of `Element`, `id`, `className` and the getters of `innerHTML` and
//! `outerHTML`. An attribute reads as text, and a write stores text. A freed element reads `""`,
//! `null`, `false` or `[]` and ignores a write.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Result as JsResult, Value};

use inca_jsenv::class::{require, text};
use inca_jsenv::required_prototype;
use inca_node::{AttributeValue, HtmlScope, NodeId};

use super::{Shared, accessor, getter, method};
use crate::bindings::attributes::{
    attribute_value_to_js, check_attribute_name, drop_attribute, store_attribute,
};
use crate::bindings::convert::{or_null, throw_tree_error};
use crate::bindings::handle::ELEMENT;
use crate::bindings::host::Host;

/// The attribute `name` of the node `id` as text, `None` when the node or the attribute is
/// missing. The name is folded to the key the node gives it.
fn read_attribute(
    ctx: &Ctx<'_>,
    host: &Shared,
    id: NodeId,
    name: &str,
) -> JsResult<Option<String>> {
    let host = host.borrow();
    let Some(node) = host.tree.get(id) else {
        return Ok(None);
    };
    let key = node.attribute_key(name);
    node.attributes()
        .get(key.as_ref())
        .map(|value| text(ctx, &attribute_value_to_js(ctx, Some(value))?))
        .transpose()
}

/// Stores the text `value` under `name`; a freed node ignores it. The name is checked first.
fn write_attribute(
    ctx: &Ctx<'_>,
    host: &Shared,
    id: NodeId,
    name: &str,
    value: String,
) -> JsResult<()> {
    check_attribute_name(ctx, name)?;
    let mut host = host.borrow_mut();
    if host.tree.get(id).is_none() {
        return Ok(());
    }
    store_attribute(ctx, &mut host, id, name, AttributeValue::String(value))
}

/// Defines `id` or `className`, which read and write the attribute `attribute`.
fn define_reflected<'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<RefCell<Host>>,
    key: &str,
    attribute: &'static str,
) -> JsResult<()> {
    accessor(
        ctx,
        proto,
        host,
        key,
        &ELEMENT,
        move |ctx, host, id, _| Ok(read_attribute(ctx, host, id, attribute)?.unwrap_or_default()),
        move |ctx, host, id, _, value: Value<'js>| {
            write_attribute(ctx, host, id, attribute, text(ctx, &value)?)
        },
    )
}

/// The markup of the node `id`, `""` for a freed node.
fn markup(ctx: &Ctx<'_>, host: &Shared, id: NodeId, scope: HtmlScope) -> JsResult<String> {
    let host = host.borrow();
    if host.tree.get(id).is_none() {
        return Ok(String::new());
    }
    host.tree
        .serialize_html(id, scope)
        .map_err(|err| throw_tree_error(ctx, err))
}

/// Defines the attribute members on `Element.prototype`.
pub(super) fn install(ctx: &Ctx<'_>, host: &Rc<RefCell<Host>>) -> JsResult<()> {
    let proto = required_prototype(ctx, "Element")?;
    define_reflected(ctx, &proto, host, "id", "id")?;
    define_reflected(ctx, &proto, host, "className", "class")?;
    method(
        ctx,
        &proto,
        host,
        "getAttribute",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            require(ctx, "getAttribute", args, 1)?;
            let name = text(ctx, &args[0])?;
            or_null(ctx, read_attribute(ctx, host, id, &name)?)
        },
    )?;
    method(
        ctx,
        &proto,
        host,
        "setAttribute",
        2,
        &ELEMENT,
        |ctx, host, id, _, args| {
            require(ctx, "setAttribute", args, 2)?;
            let name = text(ctx, &args[0])?;
            let value = text(ctx, &args[1])?;
            write_attribute(ctx, host, id, &name, value)
        },
    )?;
    method(
        ctx,
        &proto,
        host,
        "hasAttribute",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            require(ctx, "hasAttribute", args, 1)?;
            let name = text(ctx, &args[0])?;
            Ok(read_attribute(ctx, host, id, &name)?.is_some())
        },
    )?;
    method(
        ctx,
        &proto,
        host,
        "removeAttribute",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            require(ctx, "removeAttribute", args, 1)?;
            let name = text(ctx, &args[0])?;
            let mut host = host.borrow_mut();
            if host.tree.get(id).is_none() {
                return Ok(());
            }
            drop_attribute(ctx, &mut host, id, &name)
        },
    )?;
    method(
        ctx,
        &proto,
        host,
        "getAttributeNames",
        0,
        &ELEMENT,
        |_, host, id, _, _| {
            Ok(host.borrow().tree.get(id).map_or_else(Vec::new, |node| {
                node.attributes().keys().cloned().collect::<Vec<_>>()
            }))
        },
    )?;
    getter(
        ctx,
        &proto,
        host,
        "innerHTML",
        &ELEMENT,
        |ctx, host, id, _| markup(ctx, host, id, HtmlScope::Inner),
    )?;
    getter(
        ctx,
        &proto,
        host,
        "outerHTML",
        &ELEMENT,
        |ctx, host, id, _| markup(ctx, host, id, HtmlScope::Outer),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{SVG, member_error, member_json};

    #[rstest]
    #[case::get_attribute("e.getAttribute('a')", json!(null))]
    #[case::has_attribute("e.hasAttribute('a')", json!(false))]
    #[case::get_attribute_names("e.getAttributeNames()", json!([]))]
    #[case::id("e.id", json!(""))]
    #[case::class_name("e.className", json!(""))]
    #[case::inner_html("e.innerHTML", json!(""))]
    #[case::outer_html("e.outerHTML", json!(""))]
    fn a_freed_node_reads_the_default(#[case] read: &str, #[case] expected: serde_json::Value) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            e.setAttribute('a', 'b');
            free(e);
            return {read};
            "
        ));

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::set_attribute("e.setAttribute('a', 'b')")]
    #[case::remove_attribute("e.removeAttribute('a')")]
    #[case::id("e.id = 'x'")]
    #[case::class_name("e.className = 'y'")]
    fn a_write_to_a_freed_node_is_ignored(#[case] write: &str) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            free(e);
            {write};
            return [e.getAttribute('a'), e.id, e.className];
            "
        ));

        assert_eq!(got, json!([null, "", ""]));
    }

    #[rstest]
    #[case::number(
        "n.setAttribute(e.__node_id, 'count', 3)",
        "e.getAttribute('count')",
        "3"
    )]
    #[case::id("n.setAttribute(e.__node_id, 'id', 4)", "e.id", "4")]
    #[case::boolean("n.setAttribute(e.__node_id, 'class', true)", "e.className", "true")]
    fn an_attribute_stored_as_a_non_text_value_reads_as_text(
        #[case] store: &str,
        #[case] read: &str,
        #[case] expected: &str,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            {store};
            return {read};
            "
        ));

        assert_eq!(got, json!(expected));
    }

    #[test]
    fn the_attribute_names_list_in_the_order_they_were_set() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('b', '1');
            e.setAttribute('a', '2');
            return e.getAttributeNames();
            ",
        );

        assert_eq!(got, json!(["b", "a"]));
    }

    #[rstest]
    #[case::id("e.id")]
    #[case::class_name("e.className")]
    fn an_element_without_the_attribute_reads_the_empty_text(#[case] read: &str) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            return {read};
            "
        ));

        assert_eq!(got, json!(""));
    }

    #[test]
    fn a_missing_argument_throws_the_host_message() {
        let (got, _host) = member_json(
            r"
            try {
                mk('div').getAttribute();
            } catch (e) {
                return e.message;
            }
            ",
        );

        assert_eq!(
            got,
            json!("too few arguments for getAttribute. pass at least 1")
        );
    }

    #[rstest]
    #[case::set_attribute("e.setAttribute('a', 3)", "a", "3")]
    #[case::id_setter("e.id = 7", "id", "7")]
    fn the_host_setters_store_text(
        #[case] write: &str,
        #[case] name: &str,
        #[case] expected: &str,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const e = mk('div');
            {write};
            return n.getAttribute(e.__node_id, '{name}');
            "
        ));

        assert_eq!(got, json!(expected));
    }

    #[test]
    fn a_symbol_given_to_the_id_setter_throws_a_type_error() {
        assert_eq!(member_error("mk('div').id = Symbol();"), "TypeError");
    }

    #[rstest]
    #[case::live("undefined")]
    #[case::freed("free(e)")]
    fn an_invalid_attribute_name_throws_invalid_character(#[case] prepare: &str) {
        assert_eq!(
            member_error(&format!(
                r"
                const e = mk('div');
                {prepare};
                e.setAttribute('x y', '');
                "
            )),
            "InvalidCharacterError"
        );
    }

    #[test]
    fn an_html_element_folds_the_attribute_name() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('CLASS', 'a');
            const before = [e.getAttribute('class'), e.getAttributeNames()];
            e.removeAttribute('CLASS');
            return [before, e.hasAttribute('class')];
            ",
        );

        assert_eq!(got, json!([["a", ["class"]], false]));
    }

    #[test]
    fn an_svg_element_keeps_the_case_of_the_attribute_name() {
        let (got, _host) = member_json(&format!(
            r"
            const s = h(n.createElementNS('{SVG}', 'g')[0]);
            s.setAttribute('viewBox', 'x');
            return s.getAttributeNames();
            "
        ));

        assert_eq!(got, json!(["viewBox"]));
    }

    #[test]
    fn the_style_attribute_writes_the_style_props() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('style', 'width:1px');
            return [n.getAttribute(e.__node_id, 'style'), n.styleOf(e.__node_id).width];
            ",
        );

        assert_eq!(got, json!(["width: 1px;", "1px"]));
    }

    #[test]
    fn a_tabindex_set_through_the_member_focuses_on_a_focus_call() {
        let (got, host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('tabindex', '0');
            e.focus();
            return e.__node_id;
            ",
        );

        let id = u32::try_from(got.as_u64().unwrap()).unwrap();
        let tab_dirty = host.borrow_mut().focus.take_tab_dirty();
        assert!(tab_dirty.contains(&id));
    }

    #[test]
    fn removing_disabled_queues_the_recheck() {
        let (got, host) = member_json(
            r"
            const e = mk('button');
            e.setAttribute('disabled', '');
            e.removeAttribute('disabled');
            return e.__node_id;
            ",
        );

        let id = u32::try_from(got.as_u64().unwrap()).unwrap();
        assert!(host.borrow_mut().focus.take_tab_dirty().contains(&id));
    }

    #[test]
    fn the_markup_getters_serialize_the_node() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('a', 'b');
            n.appendChild(e.__node_id, n.createNode('span'));
            return [e.innerHTML, e.outerHTML];
            ",
        );

        assert_eq!(
            got,
            json!(["<span></span>", "<div a=\"b\"><span></span></div>"])
        );
    }
}
