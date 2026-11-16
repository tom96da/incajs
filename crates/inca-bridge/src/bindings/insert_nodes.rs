// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The native that inserts nodes and texts in one checked step.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, FromJs, Result as JsResult, Value};

use inca_node::{InsertArg, NodeId, Placement};

use super::Natives;
use super::convert::{JsNodeId, throw_tree_error};
use super::host::Host;
use super::mutation::pair;

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

/// Inserts `nodes` at `place` of `node_id` after checking the whole insertion. Returns
/// `[textIds, removedIds]`: the ids of the new text nodes in order and the ids of the nodes that
/// left the tree. Returns `null` without a change for `before`, `after` and `replaceWith` on a
/// node without a parent. A failed check throws and changes nothing. A removed node keeps its
/// listeners.
fn insert_nodes<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, JsPlacement, Vec<JsInsertArg>) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), JsPlacement(placement), nodes| {
        let nodes: Vec<InsertArg> = nodes.into_iter().map(|JsInsertArg(arg)| arg).collect();
        let mut host = host.borrow_mut();
        let planned = host
            .tree
            .plan_insertion(node_id, placement, &nodes)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        let Some(insertion) = planned else {
            return Ok(Value::new_null(ctx.clone()));
        };
        let left = host.leaving(&insertion.leaving());
        let inserted = host
            .tree
            .apply_insertion(insertion)
            .map_err(|err| throw_tree_error(&ctx, err))?;
        host.settle(&left);
        let ids: &[NodeId] = &inserted.texts;
        Ok(pair(&ctx, ids, &inserted.removed)?.into_value())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
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
        globalThis.order = () => {
            const names = ['a', 'b', 'c', 'e'];
            return JSON.stringify(n.childNodeIds(p).map((id) => names[[a, b, c, e].indexOf(id)]));
        };
        ";

    fn engine_with_family() -> (Engine, Rc<RefCell<Host>>) {
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
    #[case::replace_children("n.insertNodes(p, 'replaceChildren', [])", "[0,1,2]")]
    #[case::replace_with("n.insertNodes(b, 'replaceWith', [])", "[1]")]
    fn insert_nodes_with_no_node_returns_the_removed_ids(
        #[case] call: &str,
        #[case] expected: &str,
    ) {
        let (engine, _host) = engine_with_family();

        let removed: String = engine
            .eval(&format!(
                "JSON.stringify({call}[1].map((id) => [a, b, c].indexOf(id)))"
            ))
            .unwrap();

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

        let host = host.borrow();
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
