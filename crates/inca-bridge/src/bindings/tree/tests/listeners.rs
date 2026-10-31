// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::super::*;
use crate::EventTargetId;
use crate::test_support::{LISTEN_JS, callback_ids, engine_with_bindings};

/// Builds `root > branch > leaf` with a click listener on each of the two lower nodes.
const BRANCH_WITH_LISTENERS: &str = r"
    const root = __inca_native__.rootNodeId();
    const branch = __inca_native__.createNode('div');
    const leaf = __inca_native__.createNode('div');
    __inca_native__.appendChild(root, branch);
    __inca_native__.appendChild(branch, leaf);
    __inca_native__.addEventListener(branch, 'click');
    __inca_native__.addEventListener(leaf, 'click');
    ";

#[test]
fn release_listeners_returns_the_callbacks_of_the_node_and_a_descendant() {
    let (engine, _host) = engine_with_bindings();

    let mut released: Vec<u32> = engine
        .eval(&format!(
            r"
            {BRANCH_WITH_LISTENERS}
            __inca_native__.releaseListeners(branch);
            "
        ))
        .unwrap();
    released.sort_unstable();

    assert_eq!(released, [0, 1]);
}

/// Releases the listeners of `branch` and returns `[branch, leaf]`.
fn release_branch(engine: &inca_jsenv::Engine) -> Vec<NodeId> {
    engine
        .eval(&format!(
            r"
            {BRANCH_WITH_LISTENERS}
            __inca_native__.setAttribute(leaf, 'id', 'x');
            __inca_native__.releaseListeners(branch);
            [branch, leaf];
            "
        ))
        .unwrap()
}

#[test]
fn release_listeners_drops_the_registrations() {
    let (engine, host) = engine_with_bindings();
    let ids = release_branch(&engine);

    let host = host.borrow();
    assert!(callback_ids(&host.listeners, EventTargetId::Node(ids[0]), "click").is_empty());
    assert!(callback_ids(&host.listeners, EventTargetId::Node(ids[1]), "click").is_empty());
}

#[test]
fn release_listeners_keeps_the_nodes_and_their_attributes() {
    let (engine, host) = engine_with_bindings();
    let ids = release_branch(&engine);

    let host = host.borrow();
    let leaf_node = host.tree.get(ids[1]).unwrap();
    assert_eq!(leaf_node.parent(), Some(ids[0]));
    assert!(leaf_node.attributes().contains_key("id"));
}

#[test]
fn release_listeners_of_an_unknown_id_returns_nothing() {
    let (engine, _host) = engine_with_bindings();

    let released: Vec<u32> = engine
        .eval("__inca_native__.releaseListeners(999);")
        .unwrap();

    assert!(released.is_empty());
}

#[test]
fn a_dispatched_event_reaches_no_listener_after_release_listeners() {
    let (engine, _host) = engine_with_bindings();
    engine.eval::<()>(LISTEN_JS).unwrap();

    let ran: bool = engine
        .eval(
            r"
            const node = __inca_native__.createNode('div');
            let ran = false;
            __inca_test_listen__(node, 'ping', () => {
                ran = true;
            });
            __inca_native__.releaseListeners(node);
            __inca_native__.dispatchEvent(node, new Event('ping'));
            ran;
            ",
        )
        .unwrap();

    assert!(!ran);
}
