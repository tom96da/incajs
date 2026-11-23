// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `CharacterData` and `Text` interfaces: the natives that write and split the data of a node,
//! and the members of `CharacterData` and `Text`. A freed node reads `""` and ignores a write.

use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, FromJs, Result as JsResult, Value};

use inca_jsenv::class::{require, text};
use inca_jsenv::required_prototype;
use inca_node::NodeId;

use crate::bindings::Host;
use crate::bindings::convert::{JsNodeId, saturated, throw_tree_error, unsigned};
use crate::bindings::members::{Shared, accessor, getter, method};
use crate::node::handle::{CHARACTER_DATA, TEXT};
use inca_document::DocumentState;

/// Replaces the data of the text node or comment `id`. A node that holds no data throws a
/// `TypeError`.
pub(super) fn set_character_data<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), data| {
        host.document
            .borrow_mut()
            .set_data(node_id, data)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

pub(super) fn split_text<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>) -> JsResult<NodeId> {
    move |ctx, JsNodeId(node_id), offset| {
        let mut host = host.document.borrow_mut();
        host.split_text(node_id, unsigned(offset))
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

/// The data of the node `id`, `""` for a freed node.
pub(super) fn data_of(host: &Shared, id: NodeId) -> String {
    host.document
        .borrow()
        .tree()
        .text_content(id)
        .unwrap_or_default()
}

/// The length of the data of the node `id` in UTF-16 code units.
fn data_len(host: &Shared, id: NodeId) -> u32 {
    saturated(data_of(host, id).encode_utf16().count())
}

/// Whether the host has the node `id`.
pub(super) fn has(host: &DocumentState, id: NodeId) -> bool {
    host.tree().get(id).is_some()
}

/// Writes the data of the node `id`; a freed node ignores it.
pub(super) fn write_data(ctx: &Ctx<'_>, host: &Shared, id: NodeId, data: String) -> JsResult<()> {
    let mut host = host.document.borrow_mut();
    if !has(&host, id) {
        return Ok(());
    }
    host.set_data(id, data)
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
    host.replace_data(id, offset, count, data)
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

/// Defines the members of `CharacterData` and `Text`.
pub(super) fn install_character_data(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
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
            host.tree()
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
pub(super) fn install_text(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    let proto = required_prototype(ctx, "Text")?;
    getter(ctx, &proto, host, "wholeText", &TEXT, |_, host, id, _| {
        Ok(host
            .document
            .borrow()
            .tree()
            .whole_text(id)
            .unwrap_or_default())
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{engine_with_bindings, error_name, member_error, member_json};

    #[rstest]
    #[case::split_text("splitText(t, 9)")]
    fn a_split_offset_beyond_the_data_throws_index_size_error(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        let name = error_name(
            &engine,
            &format!(
                r"
                const n = __inca_native__;
                const t = n.createNode('#text');
                n.setCharacterData(t, 'abc');
                n.{call};
                "
            ),
        );

        assert_eq!(name, "IndexSizeError");
    }

    #[rstest]
    #[case::text_content("n.textContent(parent)", "abcd")]
    #[case::split_text("n.textContent(n.splitText(t, 3))", "d")]
    fn a_text_native_reads_or_edits_the_data(#[case] call: &str, #[case] want: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const parent = n.createNode('div');
                const t = n.createNode('#text');
                n.setCharacterData(t, 'abcd');
                n.appendChild(parent, t);
                {call};
                "
            ))
            .unwrap();

        assert_eq!(got, want);
    }

    #[rstest]
    #[case::text("#text")]
    #[case::comment("#comment")]
    fn set_character_data_writes_the_data(#[case] tag: &str) {
        let (engine, _host) = engine_with_bindings();

        let read: String = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const node = n.createNode('{tag}');
                n.setCharacterData(node, 'note');
                n.textContent(node);
                "
            ))
            .unwrap();

        assert_eq!(read, "note");
    }

    #[test]
    fn set_character_data_throws_type_error_for_an_element() {
        let (engine, _host) = engine_with_bindings();

        let name = error_name(
            &engine,
            "__inca_native__.setCharacterData(__inca_native__.createNode('div'), 'x')",
        );

        assert_eq!(name, "TypeError");
    }

    #[test]
    fn set_character_data_keeps_the_data_when_it_is_given_a_non_string() {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(
                r"
                const n = __inca_native__;
                const node = n.createNode('#text');
                n.setCharacterData(node, 'a');
                let name = '';
                try {
                    n.setCharacterData(node, 1);
                } catch (error) {
                    name = error.name;
                }
                [name, n.textContent(node)].join(',');
                ",
            )
            .unwrap();

        assert_eq!(got, "TypeError,a");
    }

    #[rstest]
    #[case::split_text("splitText(c, 0)")]
    fn a_text_native_throws_type_error_for_a_comment(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        let name = error_name(
            &engine,
            &format!(
                r"
                const n = __inca_native__;
                const c = n.createNode('#comment');
                n.{call};
                "
            ),
        );

        assert_eq!(name, "TypeError");
    }

    #[test]
    fn split_text_puts_the_new_node_after_the_old_one() {
        let (engine, _host) = engine_with_bindings();

        let after: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const parent = n.createNode('div');
                const t = n.createNode('#text');
                n.setCharacterData(t, 'abcd');
                n.appendChild(parent, t);
                const tail = n.splitText(t, 2);
                n.linkedNode(t, 'next') === tail;
                ",
            )
            .unwrap();

        assert!(after);
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
