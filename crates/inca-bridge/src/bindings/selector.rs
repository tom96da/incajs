// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult};

use inca_gpui::{
    NodeId, check_selector, closest_selector, matches_selector, query_selector, query_selector_all,
};

use super::Natives;
use super::convert::{JsNodeId, existing_node};
use super::host::Host;

/// `matches`, `closest`, `checkSelector`, `querySelector` and `querySelectorAll`: selector
/// matching over the retained tree.
pub(super) fn install_selector(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("matches", matches)?;
    n.set("closest", closest)?;
    n.set("checkSelector", check)?;
    n.set("querySelector", query)?;
    n.set("querySelectorAll", query_all)
}

fn check<'js>(_host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, String) -> JsResult<()> {
    move |ctx, selector| {
        check_selector(&selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

fn matches<'js>(host: Rc<RefCell<Host>>) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<bool> {
    move |ctx, JsNodeId(node_id), selector| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        matches_selector(&host.tree, node_id, &selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

fn closest<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Option<NodeId>> {
    move |ctx, JsNodeId(node_id), selector| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        closest_selector(&host.tree, node_id, &selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

fn query<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Option<NodeId>> {
    move |ctx, JsNodeId(node_id), selector| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        query_selector(&host.tree, node_id, &selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

fn query_all<'js>(
    host: Rc<RefCell<Host>>,
) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Vec<NodeId>> {
    move |ctx, JsNodeId(node_id), selector| {
        let host = host.borrow();
        existing_node(&ctx, &host.tree, node_id)?;
        query_selector_all(&host.tree, node_id, &selector)
            .map_err(|err| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::test_support::{engine_with_bindings, error_name};

    const TREE: &str = r"
        const n = __inca_native__;
        const outer = n.createNode('div');
        n.setAttribute(outer, 'class', 'card');
        const inner = n.createNode('span');
        n.appendChild(outer, inner);
        ";

    #[rstest]
    #[case::the_node_itself("outer", ".card", true)]
    #[case::a_node_below("inner", ".card", false)]
    fn matches_answers_for_a_node(
        #[case] node: &str,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                {TREE}
                n.matches({node}, '{selector}');
                "
            ))
            .unwrap();

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::an_ancestor(".card", true)]
    #[case::no_match("p", false)]
    fn closest_returns_a_node_id_or_null(#[case] selector: &str, #[case] found: bool) {
        let (engine, _host) = engine_with_bindings();

        let got: Option<u32> = engine
            .eval(&format!(
                r"
                {TREE}
                n.closest(inner, '{selector}');
                "
            ))
            .unwrap();
        let outer: u32 = engine.eval("outer").unwrap();

        assert_eq!(got, found.then_some(outer));
    }

    #[rstest]
    #[case::an_ancestor_is_no_result("outer", "div", false)]
    #[case::a_descendant("outer", "span", true)]
    #[case::no_match("outer", "p", false)]
    fn query_selector_returns_an_id_or_null(
        #[case] node: &str,
        #[case] selector: &str,
        #[case] found: bool,
    ) {
        let (engine, _host) = engine_with_bindings();

        let got: Option<u32> = engine
            .eval(&format!(
                r"
                {TREE}
                n.querySelector({node}, '{selector}');
                "
            ))
            .unwrap();
        let inner: u32 = engine.eval("inner").unwrap();

        assert_eq!(got, found.then_some(inner));
    }

    #[rstest]
    #[case::two_nodes("span, p", 2)]
    #[case::none("article", 0)]
    fn query_selector_all_returns_ids_in_order_or_an_empty_array(
        #[case] selector: &str,
        #[case] count: usize,
    ) {
        let (engine, _host) = engine_with_bindings();

        let got: Vec<u32> = engine
            .eval(&format!(
                r"
                {TREE}
                const third = n.createNode('p');
                n.appendChild(outer, third);
                n.querySelectorAll(outer, '{selector}');
                "
            ))
            .unwrap();
        let ids: Vec<u32> = engine.eval("[inner, third]").unwrap();

        assert_eq!(got, ids[..count]);
    }

    #[rstest]
    #[case::matches("matches(inner, 'div >')")]
    #[case::closest("closest(inner, '')")]
    #[case::check_selector("checkSelector('div >')")]
    #[case::query_selector("querySelector(outer, 'div >')")]
    #[case::query_selector_all("querySelectorAll(outer, '')")]
    fn a_parse_error_throws_a_syntax_error(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();
        engine.eval::<()>(TREE).unwrap();

        assert_eq!(
            error_name(&engine, &format!("__inca_native__.{call}")),
            "SyntaxError"
        );
    }

    #[rstest]
    #[case::matches("matches")]
    #[case::closest("closest")]
    #[case::query_selector("querySelector")]
    #[case::query_selector_all("querySelectorAll")]
    fn an_unknown_node_throws_a_type_error(#[case] binding: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, &format!("__inca_native__.{binding}(999, 'div')")),
            "TypeError"
        );
    }

    #[test]
    fn check_selector_returns_for_a_valid_selector() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval("__inca_native__.checkSelector('p') === undefined")
            .unwrap();

        assert!(got);
    }

    #[test]
    fn a_detached_node_still_matches() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                {TREE}
                n.appendChild(n.rootNodeId(), outer);
                n.removeChild(n.rootNodeId(), outer);
                n.matches(inner, 'div > span');
                "
            ))
            .unwrap();

        assert!(got);
    }
}
