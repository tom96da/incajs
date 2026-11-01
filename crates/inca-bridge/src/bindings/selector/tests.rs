// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

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
fn matches_answers_for_a_node(#[case] node: &str, #[case] selector: &str, #[case] expected: bool) {
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
#[case::matches("matches(inner, 'div >')")]
#[case::closest("closest(inner, '')")]
#[case::check_selector("checkSelector('div >')")]
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
