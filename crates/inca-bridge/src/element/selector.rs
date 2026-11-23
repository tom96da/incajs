// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Selectors: the natives `closest`, `querySelector` and `querySelectorAll`, and the member
//! `matches`, which gives `false` for a freed element and still checks the selector.

use std::rc::Rc;

use rquickjs::{Ctx, Exception, Result as JsResult, Value};

use inca_jsenv::class::{require, text};
use inca_node::NodeId;
use inca_style::{
    SelectorError, check_selector, closest_selector, matches_selector, query_selector,
    query_selector_all,
};

use crate::bindings::Host;
use crate::bindings::Natives;
use crate::bindings::convert::{JsNodeId, or_null};
use crate::bindings::members::method;
use crate::node::handle::ELEMENT;

/// `closest`, `querySelector` and `querySelectorAll`: selector
/// matching over the retained tree.
pub(crate) fn install_selector(n: &Natives<'_, '_>) -> JsResult<()> {
    n.set("closest", closest)?;
    n.set("querySelector", query)?;
    n.set("querySelectorAll", query_all)
}

fn closest<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), selector| {
        let mut host = host.document.borrow_mut();
        host.sync_focus_style();
        let syntax =
            |err: SelectorError| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string());
        if host.tree.get(node_id).is_none() {
            check_selector(&selector).map_err(syntax)?;
            return Ok(Value::new_null(ctx.clone()));
        }
        let found = closest_selector(&host.tree, node_id, &selector).map_err(syntax)?;
        or_null(&ctx, found)
    }
}

fn query<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Value<'js>> {
    move |ctx, JsNodeId(node_id), selector| {
        let mut host = host.document.borrow_mut();
        host.sync_focus_style();
        let syntax =
            |err: SelectorError| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string());
        if host.tree.get(node_id).is_none() {
            check_selector(&selector).map_err(syntax)?;
            return Ok(Value::new_null(ctx.clone()));
        }
        let found = query_selector(&host.tree, node_id, &selector).map_err(syntax)?;
        or_null(&ctx, found)
    }
}

fn query_all<'js>(host: Rc<Host>) -> impl Fn(Ctx<'js>, JsNodeId, String) -> JsResult<Vec<NodeId>> {
    move |ctx, JsNodeId(node_id), selector| {
        let mut host = host.document.borrow_mut();
        host.sync_focus_style();
        let syntax =
            |err: SelectorError| Exception::throw_dom(&ctx, "SyntaxError", &err.to_string());
        if host.tree.get(node_id).is_none() {
            check_selector(&selector).map_err(syntax)?;
            return Ok(Vec::new());
        }
        query_selector_all(&host.tree, node_id, &selector).map_err(syntax)
    }
}

/// Defines `matches` on `proto`.
pub(super) fn install_matches<'js>(
    ctx: &Ctx<'js>,
    proto: &rquickjs::Object<'js>,
    host: &Rc<Host>,
) -> JsResult<()> {
    method(
        ctx,
        proto,
        host,
        "matches",
        1,
        &ELEMENT,
        |ctx, host, id, _, args| {
            require(ctx, "matches", args, 1)?;
            let selector = text(ctx, &args[0])?;
            let mut host = host.document.borrow_mut();
            host.sync_focus_style();
            let syntax = |err: inca_style::SelectorError| {
                Exception::throw_dom(ctx, "SyntaxError", &err.to_string())
            };
            if host.tree.get(id).is_none() {
                check_selector(&selector).map_err(syntax)?;
                return Ok(false);
            }
            matches_selector(&host.tree, id, &selector).map_err(syntax)
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use serde_json::json;

    use crate::test_support::{engine_with_bindings, error_name, member_error, member_json};

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
                n.nodeHandle({node})[0].matches('{selector}');
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
    #[case::closest("closest(inner, 'p')")]
    #[case::query_selector("querySelector(outer, 'p')")]
    fn a_query_without_a_match_returns_null(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                {TREE}
                n.{call} === null;
                "
            ))
            .unwrap();

        assert!(got);
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
    #[case::matches("nodeHandle(inner)[0].matches('div >')")]
    #[case::closest("closest(inner, '')")]
    #[case::query_selector("querySelector(outer, 'div >')")]
    #[case::query_selector_all("querySelectorAll(outer, '')")]
    fn a_parse_error_throws_a_syntax_error(#[case] call: &str) {
        let (engine, _host) = engine_with_bindings();
        engine.eval::<()>(TREE).unwrap();

        assert_eq!(error_name(&engine, &format!("n.{call}")), "SyntaxError");
    }

    #[rstest]
    #[case::closest("closest", "null")]
    #[case::query_selector("querySelector", "null")]
    #[case::query_selector_all("querySelectorAll", "[]")]
    fn a_freed_node_gives_the_empty_result(#[case] binding: &str, #[case] expected: &str) {
        let (engine, _host) = engine_with_bindings();

        let got: String = engine
            .eval(&format!(
                "JSON.stringify(__inca_native__.{binding}(999, 'div'))"
            ))
            .unwrap();

        assert_eq!(got, expected);
    }

    #[rstest]
    #[case::closest("closest")]
    #[case::query_selector("querySelector")]
    #[case::query_selector_all("querySelectorAll")]
    fn a_freed_node_still_checks_the_selector(#[case] binding: &str) {
        let (engine, _host) = engine_with_bindings();

        assert_eq!(
            error_name(&engine, &format!("__inca_native__.{binding}(999, 'div >')")),
            "SyntaxError"
        );
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
                n.nodeHandle(inner)[0].matches('div > span');
                "
            ))
            .unwrap();

        assert!(got);
    }

    const FIELD: &str = r"
        const n = __inca_native__;
        const card = n.createNode('div');
        const field = n.createNode('div');
        n.setAttribute(field, 'tabindex', 0);
        n.appendChild(card, field);
        n.appendChild(n.rootNodeId(), card);
        ";

    #[rstest]
    #[case::matches_after_focus(
        "n.focusNode(field); n.nodeHandle(field)[0].matches(':focus')",
        true
    )]
    #[case::matches_after_blur(
        "n.focusNode(field); n.blurNode(field); n.nodeHandle(field)[0].matches(':focus')",
        false
    )]
    #[case::closest_after_focus("n.focusNode(field); n.closest(field, ':focus') === field", true)]
    #[case::closest_after_blur(
        "n.focusNode(field); n.blurNode(field); n.closest(field, ':focus') === null",
        true
    )]
    #[case::query_selector_after_focus(
        "n.focusNode(field); n.querySelector(card, ':focus') === field",
        true
    )]
    #[case::query_selector_all_after_focus(
        "n.focusNode(field); n.querySelectorAll(card, ':focus').length === 1",
        true
    )]
    fn a_focus_request_shows_in_the_next_read_of_focus(
        #[case] script: &str,
        #[case] expected: bool,
    ) {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                {FIELD}
                {script};
                "
            ))
            .unwrap();

        assert_eq!(got, expected);
    }

    #[test]
    fn focus_within_matches_an_ancestor_of_the_focused_node() {
        let (engine, _host) = engine_with_bindings();

        let got: bool = engine
            .eval(&format!(
                r"
                {FIELD}
                n.focusNode(field);
                n.nodeHandle(card)[0].matches(':focus-within') && !n.nodeHandle(card)[0].matches(':focus');
                "
            ))
            .unwrap();

        assert!(got);
    }

    #[test]
    fn a_removed_focused_node_is_not_focused_when_attached_again() {
        let (engine, _host) = engine_with_bindings();

        let got: Vec<bool> = engine
            .eval(&format!(
                r"
                {FIELD}
                n.focusNode(field);
                n.removeChild(card, field);
                const removed = n.nodeHandle(field)[0].matches(':focus');
                n.appendChild(card, field);
                [removed, n.nodeHandle(field)[0].matches(':focus')];
                "
            ))
            .unwrap();

        assert_eq!(got, [false, false]);
    }

    #[test]
    fn matches_of_a_freed_node_gives_false() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            free(e);
            return e.matches('div');
            ",
        );

        assert_eq!(got, json!(false));
    }

    #[test]
    fn matches_of_a_freed_node_throws_for_a_bad_selector() {
        let got = member_error(
            r"
            const e = mk('div');
            free(e);
            e.matches('div >');
            ",
        );

        assert_eq!(got, "SyntaxError");
    }

    #[test]
    fn a_focus_request_shows_in_matches_in_the_same_tick() {
        let (got, _host) = member_json(
            r"
            const e = mk('div');
            e.setAttribute('tabindex', '0');
            e.focus();
            return e.matches(':focus');
            ",
        );

        assert_eq!(got, json!(true));
    }
}
