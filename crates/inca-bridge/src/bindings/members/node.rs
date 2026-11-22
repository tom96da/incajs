// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members of `Node`, `CharacterData` and `Text` that need only the node id. A freed node
//! reads `""` where the member gives data or text, and keeps the kind it was built with.

use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, FromJs, Result as JsResult, Value};

use inca_jsenv::class::{require, text};
use inca_jsenv::required_prototype;
use inca_node::{NodeId, NodeKind};

use super::{Shared, accessor, getter, method};
use crate::bindings::convert::{or_null, saturated, throw_tree_error, unsigned};
use crate::bindings::handle::{CHARACTER_DATA, NODE, TEXT};
use crate::bindings::host::{DocumentState, Host};

/// The `nodeType` of a kind.
fn node_type(kind: NodeKind) -> u32 {
    match kind {
        NodeKind::Element => 1,
        NodeKind::Text => 3,
        NodeKind::Comment => 8,
        NodeKind::Document => 9,
        NodeKind::Fragment => 11,
    }
}

/// The data of the node `id`, `""` for a freed node.
fn data_of(host: &Shared, id: NodeId) -> String {
    host.document
        .borrow()
        .tree
        .text_content(id)
        .unwrap_or_default()
}

/// The length of the data of the node `id` in UTF-16 code units.
fn data_len(host: &Shared, id: NodeId) -> u32 {
    saturated(data_of(host, id).encode_utf16().count())
}

/// Whether the host has the node `id`.
fn has(host: &DocumentState, id: NodeId) -> bool {
    host.tree.get(id).is_some()
}

/// Writes the data of the node `id`; a freed node ignores it.
fn write_data(ctx: &Ctx<'_>, host: &Shared, id: NodeId, data: String) -> JsResult<()> {
    let mut host = host.document.borrow_mut();
    if !has(&host, id) {
        return Ok(());
    }
    host.tree
        .set_data(id, data)
        .map_err(|err| throw_tree_error(ctx, err))
}

/// Replaces `count` units of the data from `offset` with `data`; a freed node ignores the call.
fn replace(
    ctx: &Ctx<'_>,
    host: &Shared,
    id: NodeId,
    (offset, count): (u32, u32),
    data: &str,
) -> JsResult<()> {
    let mut host = host.document.borrow_mut();
    if !has(&host, id) {
        return Ok(());
    }
    host.tree
        .replace_data(id, offset, count, data)
        .map_err(|err| throw_tree_error(ctx, err))
}

/// The argument at `index` as an unsigned 32-bit integer.
fn unsigned_at<'js>(ctx: &Ctx<'js>, args: &[Value<'js>], index: usize) -> JsResult<u32> {
    let value = args
        .get(index)
        .cloned()
        .unwrap_or_else(|| Value::new_undefined(ctx.clone()));
    Ok(unsigned(Coerced::<i32>::from_js(ctx, value)?))
}

/// The text of the argument at `index`.
fn text_at<'js>(ctx: &Ctx<'js>, args: &[Value<'js>], index: usize) -> JsResult<String> {
    text(ctx, &args[index])
}

/// The text of a value of a nullable text member: `null` and `undefined` give `""`.
fn nullable_text<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<String> {
    if value.is_null() || value.is_undefined() {
        return Ok(String::new());
    }
    text(ctx, value)
}

/// Defines the members of `Node`.
fn install_node(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let proto = required_prototype(ctx, "Node")?;
    getter(ctx, &proto, host, "nodeType", &NODE, |_, _, _, kind| {
        Ok(node_type(kind))
    })?;
    getter(ctx, &proto, host, "nodeName", &NODE, |_, host, id, kind| {
        Ok(match kind {
            NodeKind::Element => host
                .document
                .borrow()
                .tree
                .get(id)
                .map_or_else(String::new, |node| node.tag_name().into_owned()),
            NodeKind::Text => "#text".to_owned(),
            NodeKind::Comment => "#comment".to_owned(),
            NodeKind::Document => "#document".to_owned(),
            NodeKind::Fragment => "#document-fragment".to_owned(),
        })
    })?;
    accessor(
        ctx,
        &proto,
        host,
        "nodeValue",
        &NODE,
        |ctx, host, id, kind| {
            let data =
                matches!(kind, NodeKind::Text | NodeKind::Comment).then(|| data_of(host, id));
            or_null(ctx, data)
        },
        |ctx, host, id, kind, value| {
            let data = nullable_text(ctx, &value)?;
            if matches!(kind, NodeKind::Text | NodeKind::Comment) {
                write_data(ctx, host, id, data)?;
            }
            Ok(())
        },
    )?;
    getter(
        ctx,
        &proto,
        host,
        "textContent",
        &NODE,
        |ctx, host, id, kind| or_null(ctx, (kind != NodeKind::Document).then(|| data_of(host, id))),
    )?;
    getter(ctx, &proto, host, "isConnected", &NODE, |_, host, id, _| {
        let host = host.document.borrow();
        Ok(has(&host, id) && host.tree.in_document(id))
    })?;
    method(
        ctx,
        &proto,
        host,
        "hasChildNodes",
        0,
        &NODE,
        |_, host, id, _, _| {
            Ok(host
                .document
                .borrow()
                .tree
                .get(id)
                .is_some_and(|node| !node.children().is_empty()))
        },
    )
}

/// Defines the members of `CharacterData` and `Text`.
fn install_character_data(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let proto = required_prototype(ctx, "CharacterData")?;
    accessor(
        ctx,
        &proto,
        host,
        "data",
        &CHARACTER_DATA,
        |_, host, id, _| Ok(data_of(host, id)),
        |ctx, host, id, _, value| {
            let data = if value.is_null() {
                String::new()
            } else {
                text(ctx, &value)?
            };
            write_data(ctx, host, id, data)
        },
    )?;
    getter(
        ctx,
        &proto,
        host,
        "length",
        &CHARACTER_DATA,
        |_, host, id, _| Ok(data_len(host, id)),
    )?;
    method(
        ctx,
        &proto,
        host,
        "substringData",
        2,
        &CHARACTER_DATA,
        |ctx, host, id, _, args| {
            require(ctx, "substringData", args, 2)?;
            let (offset, count) = (unsigned_at(ctx, args, 0)?, unsigned_at(ctx, args, 1)?);
            let host = host.document.borrow();
            if !has(&host, id) {
                return Ok(String::new());
            }
            host.tree
                .substring_data(id, offset, count)
                .map_err(|err| throw_tree_error(ctx, err))
        },
    )?;
    install_edits(ctx, &proto, host)
}

/// Defines the members of `CharacterData` that change the data.
fn install_edits<'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<Host>,
) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "appendData",
        1,
        &CHARACTER_DATA,
        |ctx, host, id, _, args| {
            require(ctx, "appendData", args, 1)?;
            let data = text_at(ctx, args, 0)?;
            replace(ctx, host, id, (data_len(host, id), 0), &data)
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "insertData",
        2,
        &CHARACTER_DATA,
        |ctx, host, id, _, args| {
            require(ctx, "insertData", args, 2)?;
            let offset = unsigned_at(ctx, args, 0)?;
            let data = text_at(ctx, args, 1)?;
            replace(ctx, host, id, (offset, 0), &data)
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "deleteData",
        2,
        &CHARACTER_DATA,
        |ctx, host, id, _, args| {
            require(ctx, "deleteData", args, 2)?;
            let range = (unsigned_at(ctx, args, 0)?, unsigned_at(ctx, args, 1)?);
            replace(ctx, host, id, range, "")
        },
    )?;
    method(
        ctx,
        proto,
        host,
        "replaceData",
        3,
        &CHARACTER_DATA,
        |ctx, host, id, _, args| {
            require(ctx, "replaceData", args, 3)?;
            let range = (unsigned_at(ctx, args, 0)?, unsigned_at(ctx, args, 1)?);
            let data = text_at(ctx, args, 2)?;
            replace(ctx, host, id, range, &data)
        },
    )?;
    Ok(())
}

/// Defines `wholeText` of `Text`.
fn install_text(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let proto = required_prototype(ctx, "Text")?;
    getter(ctx, &proto, host, "wholeText", &TEXT, |_, host, id, _| {
        Ok(host
            .document
            .borrow()
            .tree
            .whole_text(id)
            .unwrap_or_default())
    })
}

/// Defines the node members on the prototypes of `Node`, `CharacterData` and `Text`.
pub(super) fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    install_node(ctx, host)?;
    install_character_data(ctx, host)?;
    install_text(ctx, host)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{member_error, member_json};

    #[rstest]
    #[case::data("t.data", json!(""))]
    #[case::length("t.length", json!(0))]
    #[case::whole_text("t.wholeText", json!(""))]
    #[case::node_value("t.nodeValue", json!(""))]
    #[case::text_content("t.textContent", json!(""))]
    #[case::substring("t.substringData(0, 1)", json!(""))]
    #[case::node_type("t.nodeType", json!(3))]
    #[case::node_name("t.nodeName", json!("#text"))]
    #[case::element_node_value("e.nodeValue", json!(null))]
    #[case::element_text_content("e.textContent", json!(""))]
    #[case::is_connected("e.isConnected", json!(false))]
    #[case::has_child_nodes("e.hasChildNodes()", json!(false))]
    fn a_freed_node_reads_the_default(#[case] read: &str, #[case] expected: serde_json::Value) {
        let (got, _host) = member_json(&format!(
            r"
            const t = txt('ab');
            const e = mk('div');
            free(t);
            free(e);
            return {read};
            "
        ));

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::data("t.data = 'x'")]
    #[case::node_value("t.nodeValue = 'y'")]
    #[case::append_data("t.appendData('z')")]
    #[case::delete_data("t.deleteData(0, 1)")]
    fn a_write_to_a_freed_text_node_is_ignored(#[case] write: &str) {
        let (got, _host) = member_json(&format!(
            r"
            const t = txt('ab');
            free(t);
            {write};
            return t.data;
            "
        ));

        assert_eq!(got, json!(""));
    }

    #[rstest]
    #[case::text("txt('ab')", json!(["ab", "ab"]))]
    #[case::comment("h(n.createNode('#comment'))", json!(["", ""]))]
    #[case::element("mk('div')", json!([null, ""]))]
    #[case::fragment("h(n.createNode('#fragment'))", json!([null, ""]))]
    #[case::document("h(n.documentNodeId())", json!([null, null]))]
    fn node_value_and_text_content_follow_the_kind(
        #[case] make: &str,
        #[case] expected: serde_json::Value,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const x = {make};
            return [x.nodeValue, x.textContent];
            "
        ));

        assert_eq!(got, expected);
    }

    #[test]
    fn the_text_content_of_an_element_is_the_text_below_it() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            const t = n.createNode('#text');
            n.setCharacterData(t, 'ab');
            n.appendChild(e.__node_id, t);
            return e.textContent;
            ",
        );

        assert_eq!(got, json!("ab"));
    }

    #[test]
    fn a_node_value_write_to_a_text_node_sets_its_data() {
        let (got, _host) = member_json(
            r"
            const t = txt('a');
            t.nodeValue = null;
            return t.data;
            ",
        );

        assert_eq!(got, json!(""));
    }

    #[test]
    fn a_node_value_write_to_an_element_changes_nothing() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.nodeValue = 'x';
            return e.nodeValue;
            ",
        );

        assert_eq!(got, json!(null));
    }

    #[test]
    fn a_symbol_written_to_the_node_value_of_an_element_throws_before_the_kind_decides() {
        assert_eq!(member_error("mk('div').nodeValue = Symbol();"), "TypeError");
    }

    #[rstest]
    #[case::null("null", json!(""))]
    #[case::undefined("undefined", json!("undefined"))]
    fn a_data_write_reads_back_as_the_text_of_the_value(
        #[case] value: &str,
        #[case] expected: serde_json::Value,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const t = txt('a');
            t.data = {value};
            return t.data;
            "
        ));

        assert_eq!(got, expected);
    }

    #[test]
    fn is_connected_follows_the_path_to_the_document() {
        let (got, _host) = member_json(
            r"
            const a = loose('div');
            const b = mk('div');
            const copy = h(n.cloneNode(n.documentNodeId(), true));
            const inner = h(n.childNodeIds(copy.__node_id)[0]);
            const before = b.isConnected;
            n.removeChild(n.rootNodeId(), b.__node_id);
            return [a.isConnected, before, b.isConnected, inner.isConnected];
            ",
        );

        assert_eq!(got, json!([false, true, false, true]));
    }

    #[test]
    fn has_child_nodes_reads_the_children() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            const before = e.hasChildNodes();
            n.appendChild(e.__node_id, n.createNode('span'));
            return [before, e.hasChildNodes()];
            ",
        );

        assert_eq!(got, json!([false, true]));
    }

    #[rstest]
    #[case::append("t.appendData('X')", json!("abcdX"))]
    #[case::insert("t.insertData(1, 'X')", json!("aXbcd"))]
    #[case::delete("t.deleteData(1, 2)", json!("ad"))]
    #[case::replace("t.replaceData(1, 2, 'XY')", json!("aXYd"))]
    fn the_character_data_methods_edit_the_data(
        #[case] edit: &str,
        #[case] expected: serde_json::Value,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const t = txt('abcd');
            {edit};
            return t.data;
            "
        ));

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::substring("t.substringData(1, 2)", json!("bc"))]
    #[case::offset_as_unsigned("t.substringData(2 ** 32 + 1, 1)", json!("b"))]
    fn the_character_data_reads_follow_the_data(
        #[case] read: &str,
        #[case] expected: serde_json::Value,
    ) {
        let (got, _host) = member_json(&format!(
            r"
            const t = txt('abcd');
            return {read};
            "
        ));

        assert_eq!(got, expected);
    }

    #[test]
    fn the_length_counts_utf16_units() {
        let (got, _host) = member_json(
            r"
            const t = txt('a\u{1F600}');
            return t.length;
            ",
        );

        assert_eq!(got, json!(3));
    }

    #[test]
    fn the_whole_text_joins_the_adjacent_text_nodes() {
        let (got, _host) = member_json(
            r"
            const a = txt('ab');
            txt('cd');
            return a.wholeText;
            ",
        );

        assert_eq!(got, json!("abcd"));
    }

    #[rstest]
    #[case::substring("t.substringData(9, 0)")]
    #[case::replace("t.replaceData(9, 0, '')")]
    fn an_offset_beyond_the_data_throws_index_size_error(#[case] call: &str) {
        assert_eq!(
            member_error(&format!(
                r"
                const t = txt('abc');
                {call};
                "
            )),
            "IndexSizeError"
        );
    }

    #[test]
    fn a_missing_argument_throws_the_host_message() {
        let (got, _host) = member_json(
            r"
            try {
                txt('a').insertData(0);
            } catch (e) {
                return e.message;
            }
            ",
        );

        assert_eq!(
            got,
            json!("too few arguments for insertData. pass at least 2")
        );
    }
}
