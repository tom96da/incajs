// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The attributes of an element: the natives that read and write them, and the attribute members
//! of `Element`, `id` and `className`. A member reads an attribute as text, and a write stores
//! text. A freed element reads `""`, `null`, `false` or `[]` from a member and ignores a write.

use std::rc::Rc;

use rquickjs::{Ctx, Exception, IntoJs, Result as JsResult, Value};

use inca_jsenv::class::{require, text};
use inca_jsenv::required_prototype;
use inca_node::{AttributeValue, NodeId, valid_attribute_name};

use crate::bindings::Host;
use crate::bindings::convert::{JsNodeId, existing_node, or_null, throw_tree_error};
use crate::bindings::members::{Shared, accessor, method};
use crate::node::handle::ELEMENT;
use inca_document::DocumentState;

pub(super) fn set_attribute<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Value<'js>) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key, raw| {
        let value = attribute_value_from_js(&ctx, &raw)?;
        check_attribute_name(&ctx, &key)?;
        store_attribute(&ctx, &mut host.document.borrow_mut(), node_id, &key, value)
    }
}

/// Throws `InvalidCharacterError` for a name that is no attribute name.
pub(super) fn check_attribute_name(ctx: &Ctx<'_>, name: &str) -> JsResult<()> {
    if valid_attribute_name(name) {
        return Ok(());
    }
    Err(Exception::throw_dom(
        ctx,
        "InvalidCharacterError",
        "the attribute name is not valid. pass a name such as data-id",
    ))
}

/// Stores the attribute `name` of the node `node_id`: folds the key, writes the style text for
/// `style` and prints its warnings, and queues the focus re-check the key needs.
pub(super) fn store_attribute(
    ctx: &Ctx<'_>,
    host: &mut DocumentState,
    node_id: NodeId,
    name: &str,
    value: AttributeValue,
) -> JsResult<()> {
    host.set_attribute(node_id, name, value)
        .map_err(|err| throw_tree_error(ctx, err))?;
    Ok(())
}

/// Removes the attribute `name` of the node `node_id` and queues the focus re-check the key needs.
pub(super) fn drop_attribute(
    ctx: &Ctx<'_>,
    host: &mut DocumentState,
    node_id: NodeId,
    name: &str,
) -> JsResult<()> {
    host.remove_attribute(node_id, name)
        .map_err(|err| throw_tree_error(ctx, err))?;
    Ok(())
}

pub(super) fn get_attribute<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), key| {
        let host = host.document.borrow();
        let key = folded_key(&ctx, &host, node_id, &key)?;
        let node = existing_node(&ctx, host.tree(), node_id)?;
        attribute_value_to_js(&ctx, node.attributes().get(&key))
    }
}

pub(super) fn remove_attribute<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), key| {
        drop_attribute(&ctx, &mut host.document.borrow_mut(), node_id, &key)
    }
}

/// The key `name` has on the node `node_id`.
pub(super) fn folded_key(
    ctx: &Ctx<'_>,
    host: &DocumentState,
    node_id: NodeId,
    name: &str,
) -> JsResult<String> {
    Ok(existing_node(ctx, host.tree(), node_id)?
        .attribute_key(name)
        .into_owned())
}

pub(super) fn attribute_names<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Vec<String>> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.document.borrow();
        let node = existing_node(&ctx, host.tree(), node_id)?;
        Ok(node.attributes().keys().cloned().collect())
    }
}

pub(super) fn attribute_value_from_js<'js>(
    ctx: &Ctx<'js>,
    value: &Value<'js>,
) -> JsResult<AttributeValue> {
    if value.is_string() {
        return value.get::<String>().map(AttributeValue::String);
    }
    if value.is_number() {
        return value.get::<f64>().map(AttributeValue::Number);
    }
    if value.is_bool() {
        return value.get::<bool>().map(AttributeValue::Bool);
    }
    Err(Exception::throw_type(
        ctx,
        "value must be a string, number, or boolean",
    ))
}

/// A missing key converts to `null`.
pub(super) fn attribute_value_to_js<'js>(
    ctx: &Ctx<'js>,
    value: Option<&AttributeValue>,
) -> JsResult<Value<'js>> {
    let value = value
        .map(|value| match value {
            AttributeValue::String(s) => s.as_str().into_js(ctx),
            AttributeValue::Number(n) => n.into_js(ctx),
            AttributeValue::Bool(b) => b.into_js(ctx),
        })
        .transpose()?;
    or_null(ctx, value)
}

/// The attribute `name` of the node `id` as text, `None` when the node or the attribute is
/// missing. The name is folded to the key the node gives it.
fn read_attribute(
    ctx: &Ctx<'_>,
    host: &Shared,
    id: NodeId,
    name: &str,
) -> JsResult<Option<String>> {
    let host = host.document.borrow();
    let Some(node) = host.tree().get(id) else {
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
    let mut host = host.document.borrow_mut();
    if host.tree().get(id).is_none() {
        return Ok(());
    }
    store_attribute(ctx, &mut host, id, name, AttributeValue::String(value))
}

/// Defines `id` or `className`, which read and write the attribute `attribute`.
fn define_reflected<'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<Host>,
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

/// Defines `id`, `className` and the attribute methods on `Element.prototype`.
pub(super) fn install_members(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
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
            let mut host = host.document.borrow_mut();
            if host.tree().get(id).is_none() {
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
            Ok(host
                .document
                .borrow()
                .tree()
                .get(id)
                .map_or_else(Vec::new, |node| {
                    node.attributes().keys().cloned().collect::<Vec<_>>()
                }))
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{SVG, engine_with_bindings, error_name, member_error, member_json};
    use rstest::rstest;
    use serde_json::json;

    #[test]
    fn get_attribute_returns_the_stored_value_in_its_own_type() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'label', 'hello');
                n.setAttribute(node, 'count', 3);
                n.setAttribute(node, 'visible', true);
                n.getAttribute(node, 'label') === 'hello' &&
                    n.getAttribute(node, 'count') === 3 &&
                    n.getAttribute(node, 'visible') === true;
                ",
            )
            .unwrap();

        assert!(same, "string, number and boolean must read back unchanged");
    }

    #[test]
    fn an_attribute_name_is_case_insensitive_on_set_and_get() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'ID', 'x');
                n.getAttribute(node, 'id') === 'x' && n.getAttribute(node, 'Id') === 'x';
                ",
            )
            .unwrap();

        assert!(same);
    }

    #[test]
    fn set_attribute_throws_invalid_character_for_an_invalid_name() {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(
                &engine,
                "__inca_native__.setAttribute(__inca_native__.createNode('div'), 'a b', 'x')",
            ),
            "InvalidCharacterError"
        );
    }

    #[test]
    fn set_attribute_with_an_invalid_name_writes_nothing() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                try {
                    n.setAttribute(node, 'a b', 'x');
                } catch (e) {
                }
                n.attributeNames(node).length === 0;
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[test]
    fn an_attribute_name_keeps_its_case_outside_the_html_namespace() {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createElementNS('{SVG}', 'svg')[0];
                n.setAttribute(node, 'viewBox', '0 0 1 1');
                n.getAttribute(node, 'viewBox') === '0 0 1 1' && n.getAttribute(node, 'viewbox') === null;
                "
            ))
            .unwrap();

        assert!(same);
    }

    #[test]
    fn remove_attribute_leaves_the_key_of_another_case_outside_the_html_namespace() {
        let (engine, _host) = engine_with_bindings();

        let kept: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createElementNS('{SVG}', 'svg')[0];
                n.setAttribute(node, 'viewBox', '0 0 1 1');
                n.removeAttribute(node, 'viewbox');
                n.getAttribute(node, 'viewBox') === '0 0 1 1';
                "
            ))
            .unwrap();

        assert!(kept);
    }

    #[test]
    fn remove_attribute_deletes_the_key_it_is_given_outside_the_html_namespace() {
        let (engine, _host) = engine_with_bindings();

        let gone: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createElementNS('{SVG}', 'svg')[0];
                n.setAttribute(node, 'viewBox', '0 0 1 1');
                n.removeAttribute(node, 'viewBox');
                n.getAttribute(node, 'viewBox') === null;
                "
            ))
            .unwrap();

        assert!(gone);
    }

    #[test]
    fn attribute_names_returns_the_keys_in_insertion_order() {
        let (engine, _host) = engine_with_bindings();

        let names: Vec<String> = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('div');
                n.setAttribute(node, 'id', 'x');
                n.setAttribute(node, 'Title', 't');
                n.setAttribute(node, 'a', 1);
                n.attributeNames(node);
                ",
            )
            .unwrap();

        assert_eq!(names, ["id", "title", "a"]);
    }

    #[test]
    fn attribute_names_on_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            crate::test_support::error_name(&engine, "__inca_native__.attributeNames(999)"),
            "TypeError"
        );
    }

    #[test]
    fn get_attribute_on_an_unknown_node_throws_type_error() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                let caught = false;
                try {
                    __inca_native__.getAttribute(999, 'label');
                } catch (e) {
                    caught = e instanceof TypeError;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(caught, "an unknown node id must raise a TypeError");
    }

    #[test]
    fn non_primitive_attribute_value_throws_catchable_exception() {
        let (engine, _host) = engine_with_bindings();

        let caught: bool = engine
            .eval(
                r"
                const node = __inca_native__.createNode('div');
                let caught = false;
                try {
                    __inca_native__.setAttribute(node, 'bad', { nested: true });
                } catch (e) {
                    caught = true;
                }
                caught;
                ",
            )
            .unwrap();

        assert!(
            caught,
            "a non-primitive attribute value must raise a catchable exception"
        );
    }

    #[rstest]
    #[case::get_attribute("e.getAttribute('a')", json!(null))]
    #[case::has_attribute("e.hasAttribute('a')", json!(false))]
    #[case::get_attribute_names("e.getAttributeNames()", json!([]))]
    #[case::id("e.id", json!(""))]
    #[case::class_name("e.className", json!(""))]
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
    fn a_tabindex_set_through_the_member_queues_the_focus_recheck() {
        let (got, host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('tabindex', '0');
            return e.__node_id;
            ",
        );

        let id = u32::try_from(got.as_u64().unwrap()).unwrap();
        let tab_dirty = host.document.borrow_mut().take_tab_dirty();
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
        assert!(host.document.borrow_mut().take_tab_dirty().contains(&id));
    }
}
