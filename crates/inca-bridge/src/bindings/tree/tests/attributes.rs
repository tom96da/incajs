// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::super::*;
use crate::test_support::{callback_ids, engine_with_bindings};
use rstest::rstest;

#[test]
fn root_node_id_returns_the_host_root() {
    let (engine, host) = engine_with_bindings();

    let root_id: NodeId = engine.eval("__inca_native__.rootNodeId();").unwrap();

    assert_eq!(root_id, host.borrow().root());
    assert_eq!(host.borrow().tree.get(root_id).unwrap().tag_name(), "div");
}

#[test]
fn root_node_id_is_stable_across_calls_and_tree_growth() {
    let (engine, _host) = engine_with_bindings();

    let same: bool = engine
        .eval(
            r"
            const first = __inca_native__.rootNodeId();
            __inca_native__.createNode('div');
            first === __inca_native__.rootNodeId();
            ",
        )
        .unwrap();

    assert!(
        same,
        "the root handle must outlive any node created after it"
    );
}

#[test]
fn get_attribute_returns_the_stored_value_in_its_own_type() {
    let (engine, _host) = engine_with_bindings();

    let same: bool = engine
        .eval(
            r"
            const n = __inca_native__;
            const node = n.createNode('div');
            n.setAttribute(node, 'label', 'hello');
            n.setAttribute(node, 'count', 3);
            n.setAttribute(node, 'visible', true);
            n.getAttribute(node, 'label') === 'hello' &&
                n.getAttribute(node, 'count') === 3 &&
                n.getAttribute(node, 'visible') === true;
            ",
        )
        .unwrap();

    assert!(same, "string, number and boolean must read back unchanged");
}

#[test]
fn get_attribute_reads_the_canonical_key() {
    let (engine, _host) = engine_with_bindings();

    let same: bool = engine
        .eval(
            r"
            const n = __inca_native__;
            const node = n.createNode('div');
            n.setAttribute(node, 'tabIndex', 2);
            n.getAttribute(node, 'tabindex') === 2 &&
                n.getAttribute(node, 'TABINDEX') === 2;
            ",
        )
        .unwrap();

    assert!(same, "attribute names are case-insensitive for tabindex");
}

#[test]
fn get_attribute_of_a_missing_key_is_null() {
    let (engine, _host) = engine_with_bindings();

    let is_null: bool = engine
        .eval(
            r"
            const n = __inca_native__;
            const node = n.createNode('div');
            n.getAttribute(node, 'never_set') === null;
            ",
        )
        .unwrap();

    assert!(is_null);
}

#[test]
fn get_attribute_on_an_unknown_node_throws_type_error() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            let caught = false;
            try {
                __inca_native__.getAttribute(999, 'label');
            } catch (e) {
                caught = e instanceof TypeError;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(caught, "an unknown node id must raise a TypeError");
}

#[rstest]
#[case::nan("NaN")]
#[case::fractional("1.5")]
#[case::negative("-1")]
#[case::infinity("Infinity")]
#[case::above_u32("2 ** 32")]
#[case::numeric_string("'1'")]
fn node_id_that_is_not_a_whole_u32_throws_type_error(#[case] bad: &str) {
    let (engine, _host) = engine_with_bindings();

    let script = format!(
        r"
        (() => {{ const node = __inca_native__.createNode('div');
        const rejects = (f) => {{
            try {{ f(); return false; }} catch (e) {{ return e instanceof TypeError; }}
        }};
        return [
            rejects(() => __inca_native__.setStyle({bad}, 'width', 1)),
            rejects(() => __inca_native__.removeStyle({bad}, 'width')),
            rejects(() => __inca_native__.getAttribute({bad}, 'label')),
            rejects(() => __inca_native__.removeAttribute({bad}, 'label')),
            rejects(() => __inca_native__.appendChild({bad}, node)),
            rejects(() => __inca_native__.appendChild(node, {bad})),
            rejects(() => __inca_native__.insertBefore(node, node, {bad})),
            rejects(() => __inca_native__.destroyNode({bad})),
            rejects(() => __inca_native__.holdNode({bad})),
            rejects(() => __inca_native__.releaseNode({bad})),
            rejects(() => __inca_native__.addEventListener({bad}, 'click')),
            rejects(() => __inca_native__.removeEventListener({bad}, 'click', 1)),
            rejects(() => __inca_native__.focusNode({bad})),
            rejects(() => __inca_native__.dispatchEvent({bad}, {{}})),
            rejects(() => __inca_native__.click({bad})),
        ].every(Boolean); }})()
        "
    );
    assert!(engine.eval::<bool>(&script).unwrap(), "{bad}");
}

#[test]
fn a_normal_node_id_and_an_absent_anchor_still_work() {
    let (engine, _host) = engine_with_bindings();

    let ok: bool = engine
        .eval(
            r"
            const n = __inca_native__;
            const p = n.createNode('div');
            const c = n.createNode('div');
            n.setStyle(c, 'width', 1);
            n.insertBefore(p, c, undefined);
            n.destroyNode(c);
            true;
            ",
        )
        .unwrap();

    assert!(ok);
}

#[test]
fn unknown_node_id_throws_catchable_exception() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            let caught = false;
            try {
                __inca_native__.appendChild(999, 1000);
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(
        caught,
        "an unknown node id must raise a catchable exception"
    );
}

#[test]
fn non_primitive_attribute_value_throws_catchable_exception() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            const node = __inca_native__.createNode('div');
            let caught = false;
            try {
                __inca_native__.setAttribute(node, 'bad', { nested: true });
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(
        caught,
        "a non-primitive attribute value must raise a catchable exception"
    );
}

#[test]
fn remove_attribute_on_an_unknown_node_throws() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            const n = __inca_native__;
            let caught = false;
            try {
                n.removeAttribute(999, 'x');
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(caught);
}

#[test]
fn insert_before_unknown_ids_raise_catchable_exception() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            const parent = __inca_native__.createNode('div');
            const child = __inca_native__.createNode('span');
            let caught = false;
            try {
                __inca_native__.insertBefore(parent, child, 999);
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(
        caught,
        "an unknown anchor id must raise a catchable exception"
    );
}

#[test]
fn hold_node_throws_type_error_for_an_unknown_id() {
    let (engine, _host) = engine_with_bindings();

    let type_error: bool = engine
        .eval(
            r"
            let type_error = false;
            try {
                __inca_native__.holdNode(999);
            } catch (e) {
                type_error = e instanceof TypeError;
            }
            type_error;
            ",
        )
        .unwrap();

    assert!(type_error);
}

#[test]
fn release_node_ignores_an_unknown_id() {
    let (engine, _host) = engine_with_bindings();

    let threw: bool = engine
        .eval(
            r"
            let threw = false;
            try {
                __inca_native__.releaseNode(999);
            } catch (e) {
                threw = true;
            }
            threw;
            ",
        )
        .unwrap();

    assert!(!threw);
}

#[test]
fn a_held_node_survives_a_sweep_until_it_is_released() {
    let (engine, host) = engine_with_bindings();
    let node: NodeId = engine
        .eval("const n = __inca_native__.createNode('div'); __inca_native__.holdNode(n); n;")
        .unwrap();

    assert!(host.borrow_mut().sweep_orphans().is_none());
    assert!(host.borrow().tree.get(node).is_some());

    engine
        .eval::<()>(&format!("__inca_native__.releaseNode({node})"))
        .unwrap();
    let (nodes, _) = host.borrow_mut().sweep_orphans().unwrap();
    assert_eq!(nodes, [node]);
}

#[test]
fn sweep_orphans_releases_the_listeners_of_the_swept_subtree() {
    let (engine, host) = engine_with_bindings();
    let ids: Vec<u32> = engine
        .eval(
            r"
            const n = __inca_native__;
            const root = n.rootNodeId();
            const kept = n.createNode('div');
            n.appendChild(root, kept);
            n.addEventListener(kept, 'click');
            const branch = n.createNode('div');
            const leaf = n.createNode('div');
            n.appendChild(branch, leaf);
            n.addEventListener(branch, 'click');
            n.addEventListener(leaf, 'click');
            [kept, branch, leaf];
            ",
        )
        .unwrap();
    let (kept, branch, leaf) = (ids[0], ids[1], ids[2]);

    let (mut nodes, mut callbacks) = host.borrow_mut().sweep_orphans().unwrap();
    nodes.sort_unstable();
    callbacks.sort_unstable();

    assert_eq!(nodes, [branch, leaf]);
    assert_eq!(callbacks, [1, 2]);
    let host = host.borrow();
    assert!(callback_ids(&host.listeners, crate::EventTargetId::Node(branch), "click").is_empty());
    assert!(callback_ids(&host.listeners, crate::EventTargetId::Node(leaf), "click").is_empty());
    assert_eq!(
        callback_ids(&host.listeners, crate::EventTargetId::Node(kept), "click"),
        [0]
    );
}

#[test]
fn sweep_orphans_forgets_the_queued_focus_work_of_a_swept_node() {
    let (engine, host) = engine_with_bindings();
    let node: NodeId = engine.eval("__inca_native__.createNode('div')").unwrap();
    host.borrow_mut().focus.mark_tab_dirty(node);

    host.borrow_mut().sweep_orphans().unwrap();

    assert!(host.borrow_mut().focus.take_tab_dirty().is_empty());
}
