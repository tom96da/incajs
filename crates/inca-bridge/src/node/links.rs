// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Node` interface: the natives that follow, list and compare the links of a node and read
//! its text, and the members of `Node`. A freed node reads `""` where a member gives data or text,
//! and keeps the kind it was built with.

use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use inca_jsenv::class::text;
use inca_jsenv::required_prototype;
use inca_node::{Link, NodeId, NodeKind};

use super::character_data::{
    data_of, has, install_character_data, install_text, set_character_data, split_text, write_data,
};
use crate::bindings::convert::{Flag, JsNodeId, existing_node, or_null, throw_tree_error};
use crate::bindings::members::{accessor, getter, method};
use crate::bindings::{Host, Natives};
use crate::node::handle::NODE;

/// `linkedNode`, `childNodeIds`, the relation natives `rootOf`, `compareNodePosition` and
/// `isEqualNode`, and the text natives `textContent`, `setCharacterData` and `splitText`.
pub(crate) fn install_node(n: &Natives<'_, '_>) -> JsResult<()> {
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
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, String, Flag) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), link, element_only| {
        let host = host.document.borrow();
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
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, Flag) -> JsResult<Vec<NodeId>> {
    move |ctx, JsNodeId(node_id), element_only| {
        let host = host.document.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        Ok(host
            .tree
            .child_ids(node_id, element_only.0.flatten() == Some(true)))
    }
}

fn root_of<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<NodeId> {
    move |ctx, JsNodeId(node_id)| {
        host.document
            .borrow()
            .tree
            .root_of(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn compare_node_position<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<u32> {
    move |ctx, JsNodeId(node_id), JsNodeId(other_id)| {
        host.document
            .borrow()
            .tree
            .compare_position(node_id, other_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn is_equal_node<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId, JsNodeId) -> JsResult<bool> {
    move |ctx, JsNodeId(node_id), JsNodeId(other_id)| {
        host.document
            .borrow()
            .tree
            .is_equal(node_id, other_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

fn text_content<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId) -> JsResult<String> {
    move |ctx, JsNodeId(node_id)| {
        let host = host.document.borrow();
        host.tree
            .text_content(node_id)
            .map_err(|err| throw_tree_error(&ctx, err))
    }
}

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

/// The text of a value of a nullable text member: `null` and `undefined` give `""`.
fn nullable_text<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> JsResult<String> {
    if value.is_null() || value.is_undefined() {
        return Ok(String::new());
    }
    text(ctx, value)
}

/// Defines the members of `Node`.
fn install_node_members(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
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

/// Defines the node members on the prototypes of `Node`, `CharacterData` and `Text`.
pub(crate) fn install(ctx: &Ctx<'_>, host: &Rc<Host>) -> JsResult<()> {
    install_node_members(ctx, host)?;
    install_character_data(ctx, host)?;
    install_text(ctx, host)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde_json::json;

    use crate::test_support::{engine_with_bindings, error_name, member_error, member_json};

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
}
