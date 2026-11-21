// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The native that inserts nodes and texts in one checked step.

use std::rc::Rc;

use rquickjs::{Array, Ctx, Exception, FromJs, Result as JsResult, Value};

use inca_node::{InsertArg, Inserted, NodeId, Placement};

use super::Natives;
use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;
use super::mutation::removed_subtrees;

/// `insertNodes`: the one native behind `append`, `prepend`, `before`, `after`, `replaceWith`,
/// `replaceChildren` and `remove`. Each moved or removed node drops the focus it held, moves the
/// Tab start, queues a focus re-check and resets the scroll offsets of its subtree.
pub(super) fn install_insert_nodes(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("insertNodes", insert_nodes)
}

/// One element of the node list: a number names a node and a string makes a new text node. Any
/// other value throws a `TypeError`.
struct JsInsertArg(InsertArg);

impl<'js> FromJs<'js> for JsInsertArg {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        if value.is_string() {
            return String::from_js(ctx, value).map(|text| Self(InsertArg::Text(text)));
        }
        if value.is_number() {
            return JsNodeId::from_js(ctx, value).map(|JsNodeId(id)| Self(InsertArg::Node(id)));
        }
        Err(Exception::throw_type(
            ctx,
            "an inserted node must be a node id or a string",
        ))
    }
}

/// The place argument: `"append"`, `"prepend"`, `"before"`, `"after"`, `"replaceWith"` or
/// `"replaceChildren"`. Any other string throws a `TypeError`.
struct JsPlacement(Placement);

impl<'js> FromJs<'js> for JsPlacement {
    fn from_js(ctx: &Ctx<'js>, value: Value<'js>) -> JsResult<Self> {
        let name = String::from_js(ctx, value)?;
        match name.as_str() {
            "append" => Ok(Self(Placement::Append)),
            "prepend" => Ok(Self(Placement::Prepend)),
            "before" => Ok(Self(Placement::Before)),
            "after" => Ok(Self(Placement::After)),
            "replaceWith" => Ok(Self(Placement::ReplaceWith)),
            "replaceChildren" => Ok(Self(Placement::ReplaceChildren)),
            _ => Err(Exception::throw_type(
                ctx,
                &format!("unknown insertion place: {name}"),
            )),
        }
    }
}

/// Plans, checks and applies the insertion of `nodes` at `placement` of `node_id`, and settles the
/// nodes that leave their place. Returns `None` without a change for `Before`, `After` and
/// `ReplaceWith` on a node without a parent.
pub(super) fn insert_planned(
    ctx: &Ctx<'_>,
    host: &Host,
    node_id: NodeId,
    placement: Placement,
    nodes: &[InsertArg],
) -> JsResult<Option<Inserted>> {
    let mut document = host.document.borrow_mut();
    let planned = document
        .tree
        .plan_insertion(node_id, placement, nodes)
        .map_err(|err| throw_tree_error(ctx, err))?;
    let Some(insertion) = planned else {
        return Ok(None);
    };
    let left = document.tree.removals(&insertion.leaving());
    let inserted = document
        .tree
        .apply_insertion(insertion)
        .map_err(|err| throw_tree_error(ctx, err))?;
    host.settle(&mut document, &left);
    Ok(Some(inserted))
}

/// Inserts `nodes` at `place` of `node_id` after checking the whole insertion. Returns
/// `[textIds, removedSubtrees]`: the ids of the new text nodes in order and, for each node that
/// left the tree, the ids of its subtree in pre-order with the node first. Returns `null` without
/// a change for `before`, `after` and `replaceWith` on a node without a parent. A failed check
/// throws and changes nothing. A removed node keeps its listeners.
fn insert_nodes<'js>(
    host: Rc<Host>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsPlacement, Vec<JsInsertArg>) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), JsPlacement(placement), nodes| {
        let nodes: Vec<InsertArg> = nodes.into_iter().map(|JsInsertArg(arg)| arg).collect();
        let Some(inserted) = insert_planned(&ctx, &host, node_id, placement, &nodes)? else {
            return Ok(Value::new_null(ctx.clone()));
        };
        let array = Array::new(ctx.clone())?;
        array.set(0, inserted.texts)?;
        array.set(
            1,
            removed_subtrees(&ctx, &host.document.borrow().tree, &inserted.removed)?,
        )?;
        Ok(array.into_value())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::rc::Rc;

    use inca_jsenv::Engine;
    use rstest::rstest;

    use crate::test_support::{LISTEN_JS, callback_ids, engine_with_bindings, error_name};
    use crate::{EventTargetId, Host};

    /// `p > (a, b, c)`, the parentless `loose` and the new element `e`. `order` reads the
    /// children of `p` by name.
    const SETUP: &str = r"
        globalThis.n = __inca_native__;
        const make = (tag) => n.createNode(tag);
        globalThis.p = make('div');
        globalThis.a = make('span');
        globalThis.b = make('span');
        globalThis.c = make('span');
        globalThis.e = make('i');
        globalThis.loose = make('span');
        n.appendChild(p, a);
        n.appendChild(p, b);
        n.appendChild(p, c);
        globalThis.listed = (call) => {
            const names = { [a]: 'a', [b]: 'b', [c]: 'c', [e]: 'e', [globalThis.k]: 'k', [globalThis.j]: 'j' };
            return JSON.stringify(call().map((ids) => ids.map((id) => names[id] ?? 'other')));
        };
        globalThis.order = () => {
            const names = ['a', 'b', 'c', 'e'];
            return JSON.stringify(n.childNodeIds(p).map((id) => names[[a, b, c, e].indexOf(id)]));
        };
        ";

    fn engine_with_family() -> (Engine, Rc<Host>) {
        let (engine, host) = engine_with_bindings();
        engine.eval::<()>(LISTEN_JS).unwrap();
        engine.eval::<()>(SETUP).unwrap();
        (engine, host)
    }

    #[test]
    fn insert_nodes_returns_the_new_text_ids_in_argument_order() {
        let (engine, _host) = engine_with_family();

        let data: String = engine
            .eval(
                r"
                const [texts] = n.insertNodes(p, 'append', ['x', a, 'y']);
                JSON.stringify(texts.map((id) => n.textContent(id)));
                ",
            )
            .unwrap();

        assert_eq!(data, r#"["x","y"]"#);
    }

    #[rstest]
    #[case::replace_children("n.insertNodes(p, 'replaceChildren', [])", r#"[["a"],["b"],["c"]]"#)]
    #[case::replace_with("n.insertNodes(b, 'replaceWith', [])", r#"[["b"]]"#)]
    fn insert_nodes_with_no_node_returns_the_subtree_of_each_removed_node(
        #[case] call: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine_with_family();

        let removed: String = engine.eval(&format!("listed(() => {call}[1])")).unwrap();

        assert_eq!(removed, expected);
    }

    #[rstest]
    #[case::grandchild_moved_up(
        "n.insertNodes(p, 'replaceChildren', [k])",
        r#"[["a","j"],["b"],["c"]]"#
    )]
    #[case::node_moved_into_the_place_of_the_target(
        "n.insertNodes(b, 'replaceWith', [a])",
        r#"[["b"]]"#
    )]
    fn a_node_the_call_inserts_is_not_in_a_removed_subtree(
        #[case] call: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine_with_family();
        engine
            .eval::<()>(
                r"
                globalThis.k = n.createNode('i');
                globalThis.j = n.createNode('u');
                n.appendChild(a, k);
                n.appendChild(a, j);
                ",
            )
            .unwrap();

        let removed: String = engine.eval(&format!("listed(() => {call}[1])")).unwrap();

        assert_eq!(removed, expected);
    }

    #[rstest]
    #[case::before("before")]
    #[case::after("after")]
    #[case::replace_with("replaceWith")]
    fn insert_nodes_returns_null_for_a_node_without_a_parent(#[case] place: &str) {
        let (engine, _host) = engine_with_family();

        let result: String = engine
            .eval(&format!(
                "JSON.stringify(n.insertNodes(loose, '{place}', ['s']))"
            ))
            .unwrap();

        assert_eq!(result, "null");
    }

    #[test]
    fn a_failed_plan_reaches_the_script_as_its_dom_error() {
        let (engine, _host) = engine_with_family();

        assert_eq!(
            error_name(&engine, "n.insertNodes(a, 'append', [p])"),
            "HierarchyRequestError"
        );
    }

    #[rstest]
    #[case::object("n.insertNodes(p, 'append', [{}])")]
    #[case::null("n.insertNodes(p, 'append', [null])")]
    #[case::fraction("n.insertNodes(p, 'append', [1.5])")]
    #[case::negative("n.insertNodes(p, 'append', [-1])")]
    #[case::unknown_place("n.insertNodes(p, 'sideways', [])")]
    fn insert_nodes_throws_a_type_error_for_a_bad_argument(#[case] call: &str) {
        let (engine, _host) = engine_with_family();

        assert_eq!(error_name(&engine, call), "TypeError");
    }

    #[rstest]
    #[case::append("p", "append")]
    #[case::before_a_node_without_a_parent("loose", "before")]
    fn a_lone_surrogate_throws_before_any_change(#[case] target: &str, #[case] place: &str) {
        let (engine, _host) = engine_with_family();

        let outcome: String = engine
            .eval(&format!(
                r"
                const first = n.createNode('i');
                let name = 'none';
                try {{
                    n.insertNodes({target}, '{place}', ['x', '\ud800']);
                }} catch (e) {{
                    name = e.name;
                }}
                JSON.stringify([name, n.childNodeIds(p).length, n.createNode('i') - first]);
                "
            ))
            .unwrap();

        assert_eq!(outcome, r#"["TypeError",3,1]"#);
    }

    #[test]
    fn replacing_a_node_with_no_node_keeps_its_listeners() {
        let (engine, host) = engine_with_family();
        let id: u32 = engine
            .eval(
                r"
                n.addEventListener(b, 'click', () => {});
                n.insertNodes(b, 'replaceWith', []);
                b;
                ",
            )
            .unwrap();

        let host = host.document.borrow();
        assert_eq!(
            callback_ids(&host.listeners, EventTargetId::Node(id), "click").len(),
            1
        );
    }

    #[rstest]
    #[case::append("p", "append", r#"["a","b","c","e"]"#)]
    #[case::prepend("p", "prepend", r#"["e","a","b","c"]"#)]
    #[case::before("b", "before", r#"["a","e","b","c"]"#)]
    #[case::after("b", "after", r#"["a","b","e","c"]"#)]
    #[case::replace_with("b", "replaceWith", r#"["a","e","c"]"#)]
    #[case::replace_children("p", "replaceChildren", r#"["e"]"#)]
    fn each_place_puts_the_node_where_its_name_says(
        #[case] target: &str,
        #[case] place: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine_with_family();

        let order: String = engine
            .eval(&format!(
                r"
                n.insertNodes({target}, '{place}', [e]);
                order();
                "
            ))
            .unwrap();

        assert_eq!(order, expected);
    }
}
