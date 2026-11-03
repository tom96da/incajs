// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Natives that read the identity, the links and the text of a node.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use inca_gpui::NodeId;

use super::Natives;
use super::convert::{JsNodeId, existing_node, or_null, throw_tree_error};
use super::host::Host;

/// `linkedNode`, `childNodeIds`, `isConnected` and the text natives `textContent`, `wholeText`,
/// `substringData`, `replaceData` and `splitText`.
pub(super) fn install_node(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("linkedNode", linked_node)?;
    n.set("childNodeIds", child_node_ids)?;
    n.set("isConnected", is_connected)?;
    n.set("textContent", text_content)?;
    n.set("wholeText", whole_text)?;
    n.set("substringData", substring_data)?;
    n.set("replaceData", replace_data)?;
    n.set("splitText", split_text)
}

/// The node `link` names from the node `id`, or `null`. `link` is `"parent"`, `"first"`,
/// `"last"`, `"previous"` or `"next"`.
fn linked_node<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), link| {
        let host = host.borrow();
        let node = existing_node(&ctx, &host.tree, node_id)?;
        let found = match link.as_str() {
            "parent" => node.parent(),
            "first" => host.tree.first_child(node_id),
            "last" => host.tree.last_child(node_id),
            "previous" => host.tree.previous_sibling(node_id),
            "next" => host.tree.next_sibling(node_id),
            other => {
                return Err(Exception::throw_type(
                    &ctx,
                    &format!(
                        concat!(
                            "unknown node link: {other}. ",
                            "pass \"parent\", \"first\", \"last\", \"previous\" or \"next\""
                        ),
                        other = other,
                    ),
                ));
            }
        };
        or_null(&ctx, found)
    }
}

fn child_node_ids<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<Vec<NodeId>> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        Ok(existing_node(&ctx, &host.tree, node_id)?
            .children()
            .to_vec())
    }
}

fn is_connected<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<bool> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        Ok(host.tree.is_connected(node_id))
    }
}

fn text_content<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<String> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        host.tree
            .text_content(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn whole_text<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<String> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.borrow();
        host.tree
            .whole_text(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

/// An offset or a count: the argument converts to an unsigned 32-bit integer.
fn unsigned(Coerced(value): Coerced<i32>) -> u32 {
    value.cast_unsigned()
}

fn substring_data<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>, Coerced<i32>) -> JsResult<String> {
    move |ctx, JsNodeId(node_id), offset, count| {
        let host = host.borrow();
        host.tree
            .substring_data(node_id, unsigned(offset), unsigned(count))
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn replace_data<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>, Coerced<i32>, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), offset, count, data| {
        let mut host = host.borrow_mut();
        host.tree
            .replace_data(node_id, unsigned(offset), unsigned(count), &data)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn split_text<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Coerced<i32>) -> JsResult<NodeId> {
    move |ctx, JsNodeId(node_id), offset| {
        let mut host = host.borrow_mut();
        host.tree
            .split_text(node_id, unsigned(offset))
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{engine_with_bindings, error_name};

    #[rstest]
    #[case::other_name("'wide'")]
    #[case::other_case("'Parent'")]
    #[case::empty("''")]
    fn linked_node_throws_type_error_for_an_unknown_link(#[case] link: &str) {
        let (engine, _host) = engine_with_bindings();

        let name = error_name(
            &engine,
            &format!("__inca_native__.linkedNode(__inca_native__.createNode('div'), {link})"),
        );

        assert_eq!(name, "TypeError");
    }

    #[rstest]
    #[case::linked_node("linkedNode(999, 'parent')")]
    #[case::child_node_ids("childNodeIds(999)")]
    #[case::is_connected("isConnected(999)")]
    #[case::text_content("textContent(999)")]
    #[case::whole_text("wholeText(999)")]
    #[case::substring_data("substringData(999, 0, 0)")]
    #[case::replace_data("replaceData(999, 0, 0, '')")]
    #[case::split_text("splitText(999, 0)")]
    fn an_unknown_node_id_throws_type_error(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, &format!("__inca_native__.{call}")),
            "TypeError"
        );
    }

    #[rstest]
    #[case::parent("mid", "parent", "top")]
    #[case::first("mid", "first", "leaf")]
    #[case::last("mid", "last", "other")]
    #[case::previous("other", "previous", "leaf")]
    #[case::next("leaf", "next", "other")]
    fn linked_node_follows_the_link(#[case] node: &str, #[case] link: &str, #[case] want: &str) {
        let (engine, _host) = engine_with_bindings();

        let same: bool = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const top = n.createNode('div');
                const mid = n.createNode('div');
                const leaf = n.createNode('div');
                const other = n.createNode('div');
                n.appendChild(top, mid);
                n.appendChild(mid, leaf);
                n.appendChild(mid, other);
                const ids = {{ top, mid, leaf, other }};
                n.linkedNode(ids['{node}'], '{link}') === ids['{want}'];
                "
            ))
            .unwrap();

        assert!(same);
    }

    #[test]
    fn linked_node_is_null_when_there_is_no_node_to_reach() {
        let (engine, _host) = engine_with_bindings();

        let all_null: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const lone = n.createNode('div');
                ['parent', 'first', 'last', 'previous', 'next'].every(
                    (link) => n.linkedNode(lone, link) === null,
                );
                ",
            )
            .unwrap();

        assert!(all_null);
    }

    #[test]
    fn child_node_ids_lists_the_children_in_order() {
        let (engine, _host) = engine_with_bindings();

        let ordered: bool = engine
            .eval(
                r"
                const n = __inca_native__;
                const parent = n.createNode('div');
                const a = n.createNode('div');
                const b = n.createNode('div');
                n.appendChild(parent, a);
                n.appendChild(parent, b);
                const ids = n.childNodeIds(parent);
                ids.length === 2 && ids[0] === a && ids[1] === b;
                ",
            )
            .unwrap();

        assert!(ordered);
    }

    #[test]
    fn child_node_ids_is_empty_for_a_leaf() {
        let (engine, _host) = engine_with_bindings();

        let empty: bool = engine
            .eval("__inca_native__.childNodeIds(__inca_native__.createNode('div')).length === 0;")
            .unwrap();

        assert!(empty);
    }

    #[test]
    fn is_connected_follows_the_path_to_the_root() {
        let (engine, _host) = engine_with_bindings();

        let reads: Vec<bool> = engine
            .eval(
                r"
                const n = __inca_native__;
                const mid = n.createNode('div');
                const leaf = n.createNode('div');
                n.appendChild(mid, leaf);
                const detached = n.isConnected(leaf);
                n.appendChild(n.rootNodeId(), mid);
                [detached, n.isConnected(leaf), n.isConnected(n.rootNodeId())];
                ",
            )
            .unwrap();

        assert_eq!(reads, [false, true, true]);
    }

    #[rstest]
    #[case::substring_data("substringData(t, 9, 0)")]
    #[case::replace_data("replaceData(t, 9, 0, '')")]
    #[case::split_text("splitText(t, 9)")]
    fn an_offset_beyond_the_data_throws_index_size_error(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        let name = error_name(
            &engine,
            &format!(
                r"
                const n = __inca_native__;
                const t = n.createNode('#text');
                n.setAttribute(t, 'value', 'abc');
                n.{call};
                "
            ),
        );

        assert_eq!(name, "IndexSizeError");
    }

    #[rstest]
    #[case::text_content("n.textContent(parent)", "abcd")]
    #[case::whole_text("n.wholeText(t)", "abcd")]
    #[case::substring_data("n.substringData(t, 1, 2)", "bc")]
    #[case::replace_data("n.replaceData(t, 1, 2, 'XY'); n.getAttribute(t, 'value')", "aXYd")]
    #[case::split_text("n.getAttribute(n.splitText(t, 3), 'value')", "d")]
    fn a_text_native_reads_or_edits_the_data(#[case] call: &str, #[case] want: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const parent = n.createNode('div');
                const t = n.createNode('#text');
                n.setAttribute(t, 'value', 'abcd');
                n.appendChild(parent, t);
                {call};
                "
            ))
            .unwrap();

        assert_eq!(got, want);
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
                n.setAttribute(t, 'value', 'abcd');
                n.appendChild(parent, t);
                const tail = n.splitText(t, 2);
                n.linkedNode(t, 'next') === tail;
                ",
            )
            .unwrap();

        assert!(after);
    }

    #[test]
    fn an_offset_converts_to_an_unsigned_integer() {
        let (engine, _host) = engine_with_bindings();

        let read: String = engine
            .eval(
                r"
                const n = __inca_native__;
                const t = n.createNode('#text');
                n.setAttribute(t, 'value', 'abc');
                n.substringData(t, 2 ** 32 + 1, 1);
                ",
            )
            .unwrap();

        assert_eq!(read, "b");
    }
}
