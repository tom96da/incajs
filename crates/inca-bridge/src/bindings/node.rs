// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Natives that read the identity and the links of a node.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use inca_gpui::NodeId;

use super::Natives;
use super::convert::{JsNodeId, existing_node, or_null};
use super::host::Host;

/// `linkedNode`, `childNodeIds` and `isConnected`.
pub(super) fn install_node(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("linkedNode", linked_node)?;
    n.set("childNodeIds", child_node_ids)?;
    n.set("isConnected", is_connected)
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
}
