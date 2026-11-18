// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Natives that read the identity, the links and the text of a node.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::convert::Coerced;
use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use inca_node::{Link, NodeId};

use super::Natives;
use super::convert::{Flag, JsNodeId, existing_node, or_null, throw_tree_error, unsigned};
use super::host::Host;

/// `linkedNode`, `childNodeIds`, the relation natives `rootOf`, `compareNodePosition` and
/// `isEqualNode`, and the text natives `textContent`, `setCharacterData` and `splitText`.
pub(super) fn install_node(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("linkedNode", linked_node)?;
    n.set("childNodeIds", child_node_ids)?;
    n.set("rootOf", root_of)?;
    n.set("compareNodePosition", compare_node_position)?;
    n.set("isEqualNode", is_equal_node)?;
    n.set("textContent", text_content)?;
    n.set("setCharacterData", set_character_data)?;
    n.set("splitText", split_text)
}

/// The node `link` names from the node `id`, or `null`. `link` is `"parent"`, `"first"`,
/// `"last"`, `"previous"` or `"next"`. With `elementOnly`, the nearest element in that direction.
fn linked_node<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Flag) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), link, element_only| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        let link = match link.as_str() {
            "parent" => Link::Parent,
            "first" => Link::First,
            "last" => Link::Last,
            "previous" => Link::Previous,
            "next" => Link::Next,
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
        let element_only = element_only.0.flatten() == Some(true);
        or_null(&ctx, host.tree.linked(node_id, link, element_only))
    }
}

/// The ids of the children of the node `id` in order, only the elements with `elementOnly`.
fn child_node_ids<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, Flag) -> JsResult<Vec<NodeId>> {
    move |ctx, JsNodeId(node_id), element_only| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        Ok(host
            .tree
            .child_ids(node_id, element_only.0.flatten() == Some(true)))
    }
}

fn root_of<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<NodeId> {
    move |ctx, JsNodeId(node_id)| {
        host.borrow()
            .tree
            .root_of(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn compare_node_position<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<u32> {
    move |ctx, JsNodeId(node_id), JsNodeId(other_id)| {
        host.borrow()
            .tree
            .compare_position(node_id, other_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn is_equal_node<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<bool> {
    move |ctx, JsNodeId(node_id), JsNodeId(other_id)| {
        host.borrow()
            .tree
            .is_equal(node_id, other_id)
            .map_err(|err| throw_tree_error(&ctx, err))
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

/// Replaces the data of the text node or comment `id`. A node that holds no data throws a
/// `TypeError`.
fn set_character_data<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<()> {
    move |ctx, JsNodeId(node_id), data| {
        host.borrow_mut()
            .tree
            .set_data(node_id, data)
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
    #[case::linked_node_of_elements("linkedNode(999, 'next', true)")]
    #[case::child_node_ids("childNodeIds(999)")]
    #[case::child_node_ids_of_elements("childNodeIds(999, true)")]
    #[case::text_content("textContent(999)")]
    #[case::set_character_data("setCharacterData(999, '')")]
    #[case::split_text("splitText(999, 0)")]
    #[case::root_of("rootOf(999)")]
    #[case::compare_unknown_node("compareNodePosition(999, __inca_native__.rootNodeId())")]
    #[case::compare_unknown_other("compareNodePosition(__inca_native__.rootNodeId(), 999)")]
    #[case::equal_unknown_node("isEqualNode(999, __inca_native__.rootNodeId())")]
    #[case::equal_unknown_other("isEqualNode(__inca_native__.rootNodeId(), 999)")]
    fn an_unknown_node_id_throws_type_error(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, &format!("__inca_native__.{call}")),
            "TypeError"
        );
    }

    /// `parent > (a, #text, #comment, b)`, each node a global.
    const MIXED: &str = r"
        const n = __inca_native__;
        const parent = n.createNode('div');
        const a = n.createNode('span');
        const text = n.createNode('#text');
        const comment = n.createNode('#comment');
        const b = n.createNode('span');
        n.appendChild(parent, a);
        n.appendChild(parent, text);
        n.appendChild(parent, comment);
        n.appendChild(parent, b);
        ";

    #[test]
    fn linked_node_with_the_element_flag_skips_a_text_and_a_comment() {
        let (engine, _host) = engine_with_bindings();

        let found: bool = engine
            .eval(&format!(
                r"
                {MIXED}
                n.linkedNode(a, 'next', true) === b;
                "
            ))
            .unwrap();

        assert!(found);
    }

    #[rstest]
    #[case::omitted("")]
    #[case::undefined(", undefined")]
    #[case::null(", null")]
    #[case::false_value(", false")]
    fn linked_node_without_the_element_flag_reads_any_node(#[case] flag: &str) {
        let (engine, _host) = engine_with_bindings();

        let found: bool = engine
            .eval(&format!(
                r"
                {MIXED}
                n.linkedNode(a, 'next'{flag}) === text;
                "
            ))
            .unwrap();

        assert!(found);
    }

    #[rstest]
    #[case::linked_node("linkedNode(a, 'next', 1)")]
    #[case::child_node_ids("childNodeIds(parent, 'yes')")]
    fn an_element_flag_that_is_no_boolean_throws_type_error(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        let name = error_name(&engine, &format!("{MIXED} n.{call}"));

        assert_eq!(name, "TypeError");
    }

    #[test]
    fn child_node_ids_with_the_element_flag_lists_the_elements() {
        let (engine, _host) = engine_with_bindings();

        let listed: bool = engine
            .eval(&format!(
                r"
                {MIXED}
                const ids = n.childNodeIds(parent, true);
                ids.length === 2 && ids[0] === a && ids[1] === b;
                "
            ))
            .unwrap();

        assert!(listed);
    }

    #[test]
    fn child_node_ids_without_the_element_flag_lists_every_child() {
        let (engine, _host) = engine_with_bindings();

        let count: u32 = engine
            .eval(&format!(
                r"
                {MIXED}
                n.childNodeIds(parent).length;
                "
            ))
            .unwrap();

        assert_eq!(count, 4);
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

    #[rstest]
    #[case::compare_position("n.compareNodePosition(top, leaf)", 20)]
    #[case::root_of("Number(n.rootOf(leaf) === top)", 1)]
    #[case::is_equal_to_a_childless_twin("Number(n.isEqualNode(top, twin))", 0)]
    #[case::is_equal_with_another_tag("Number(n.isEqualNode(leaf, twin))", 0)]
    fn a_relation_native_reports_the_tree_shape(#[case] call: &str, #[case] want: u32) {
        let (engine, _host) = engine_with_bindings();

        let read: u32 = engine
            .eval(&format!(
                r"
                const n = __inca_native__;
                const top = n.createNode('div');
                const leaf = n.createNode('span');
                const twin = n.createNode('div');
                n.appendChild(top, leaf);
                {call};
                "
            ))
            .unwrap();

        assert_eq!(read, want);
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

    #[rstest]
    #[case::split_text("splitText(t, 9)")]
    fn an_offset_beyond_the_data_throws_index_size_error(#[case] call: &str) {
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
}
