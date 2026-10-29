// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::test_support::{callback_ids, engine_with_bindings, error_name};
use rstest::rstest;

#[test]
fn remove_style_on_an_unknown_node_throws() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            const n = __inca_native__;
            let caught = false;
            try {
                n.removeStyle(999, 'gap');
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(caught);
}

/// Evaluates `script` against bindings whose style warnings are
/// collected, and returns them with the host.
fn warnings_from(script: &str) -> (Rc<RefCell<Vec<String>>>, Rc<RefCell<Host>>) {
    let (engine, host) = engine_with_bindings();
    let warnings = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&warnings);
    host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));
    engine.eval::<()>(script).unwrap();
    (warnings, host)
}

#[test]
fn set_style_warns_once_per_unknown_key() {
    let (warnings, _) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'flexDirections', 'column');
        ",
    );

    assert_eq!(
        *warnings.borrow(),
        ["node 1 (div): ignoring unknown style key `flexDirections`"]
    );
}

#[test]
fn a_bad_value_warns_once_for_a_node_and_key() {
    let (warnings, _) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'background', -1);
        __inca_native__.setStyle(n, 'background', -1);
        __inca_native__.setStyle(n, 'background', 0xff0000);
        __inca_native__.setStyle(n, 'background', -1);
        ",
    );

    assert_eq!(
        *warnings.borrow(),
        ["node 1 (div): ignoring invalid color for style key `background`"]
    );
}

#[test]
fn remove_style_does_not_make_a_key_warn_again() {
    let (warnings, _) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'bogus', 1);
        __inca_native__.removeStyle(n, 'bogus');
        __inca_native__.setStyle(n, 'bogus', 1);
        ",
    );

    assert_eq!(warnings.borrow().len(), 1, "{:?}", warnings.borrow());
}

#[test]
fn each_node_warns_for_the_same_bad_key() {
    let (warnings, _) = warnings_from(
        r"
        const a = __inca_native__.createNode('div');
        const b = __inca_native__.createNode('div');
        __inca_native__.setStyle(a, 'bogus', 1);
        __inca_native__.setStyle(b, 'bogus', 1);
        __inca_native__.setStyle(a, 'bogus', 1);
        ",
    );

    assert_eq!(
        *warnings.borrow(),
        [
            "node 1 (div): ignoring unknown style key `bogus`",
            "node 2 (div): ignoring unknown style key `bogus`",
        ]
    );
}

#[test]
fn each_spelling_of_a_bad_key_warns() {
    let (warnings, _) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'bogus-key', 1);
        __inca_native__.setStyle(n, 'bogusKey', 1);
        __inca_native__.setStyle(n, 'bogus-key', 1);
        __inca_native__.setStyle(n, 'bogusKey', 1);
        ",
    );

    assert_eq!(
        *warnings.borrow(),
        [
            "node 1 (div): ignoring unknown style key `bogus-key`",
            "node 1 (div): ignoring unknown style key `bogusKey`",
        ]
    );
}

#[test]
fn each_bad_key_of_a_node_warns_once() {
    let (warnings, _) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'bogus', 1);
        __inca_native__.setStyle(n, 'width', '50%');
        __inca_native__.setStyle(n, 'bogus', 1);
        __inca_native__.setStyle(n, 'width', '50%');
        ",
    );

    assert_eq!(
        *warnings.borrow(),
        [
            "node 1 (div): ignoring unknown style key `bogus`",
            "node 1 (div): ignoring unsupported value for style key `width`",
        ]
    );
}

#[test]
fn a_destroyed_node_forgets_its_style_warnings() {
    let (_, host) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'bogus', 1);
        __inca_native__.destroyNode(n);
        ",
    );

    assert!(host.borrow().style_warned.is_empty());
}

#[test]
fn a_failed_set_style_emits_no_warning() {
    let (warnings, host) = warnings_from(
        r"
        try {
            __inca_native__.setStyle(999, 'bogus', 1);
        } catch (e) {}
        const n = __inca_native__.createNode('div');
        try {
            __inca_native__.setStyle(n, 'bogus', { nested: true });
        } catch (e) {}
        ",
    );

    assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
    assert!(host.borrow().style_warned.is_empty());
}

#[test]
fn set_style_names_the_tag_of_the_node() {
    let (warnings, _) = warnings_from(
        r"
        const n = __inca_native__.createNode('text');
        __inca_native__.setStyle(n, 'bogus', 1);
        ",
    );

    assert_eq!(
        *warnings.borrow(),
        ["node 1 (text): ignoring unknown style key `bogus`"]
    );
}

#[test]
fn set_style_is_quiet_for_known_keys_and_box_keys() {
    let (warnings, _) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'display', 'flex');
        for (const k of ['padding', 'padding_x', 'margin_left', 'gap']) {
            __inca_native__.setStyle(n, k, 4);
        }
        __inca_native__.setStyle(n, 'text_color', '#fff');
        ",
    );

    assert!(warnings.borrow().is_empty(), "{:?}", warnings.borrow());
}

#[test]
fn a_stored_bad_style_does_not_warn_again_when_the_tree_is_read() {
    let (warnings, host) = warnings_from(
        r"
        const n = __inca_native__.createNode('div');
        __inca_native__.setStyle(n, 'bogus', 1);
        ",
    );
    assert_eq!(warnings.borrow().len(), 1);

    let host = host.borrow();
    for _ in 0..3 {
        inca_gpui::build_spec(&host.tree, 1).unwrap();
    }
    assert_eq!(warnings.borrow().len(), 1);
}

#[test]
fn set_style_unknown_node_id_throws_catchable_exception() {
    let (engine, _host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            let caught = false;
            try {
                __inca_native__.setStyle(999, 'display', 'flex');
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
fn destroy_node_hands_back_the_callback_ids_of_its_subtree() {
    let (engine, _host) = engine_with_bindings();

    let released: Vec<u32> = engine
        .eval(
            r"
            const root = __inca_native__.rootNodeId();
            const branch = __inca_native__.createNode('div');
            const leaf = __inca_native__.createNode('text');
            __inca_native__.appendChild(root, branch);
            __inca_native__.appendChild(branch, leaf);
            __inca_native__.addEventListener(branch, 'click');
            __inca_native__.addEventListener(leaf, 'click');

            __inca_native__.destroyNode(branch);
            ",
        )
        .unwrap();
    let mut released = released;
    released.sort_unstable();

    assert_eq!(
        released,
        vec![0, 1],
        "a descendant's callback is unreachable from JS once its root is gone"
    );
}

#[test]
fn destroy_node_frees_its_subtree() {
    let (engine, host) = engine_with_bindings();

    let _: Vec<u32> = engine
        .eval(
            r"
            const root = __inca_native__.rootNodeId();
            const branch = __inca_native__.createNode('div');
            const leaf = __inca_native__.createNode('text');
            __inca_native__.appendChild(root, branch);
            __inca_native__.appendChild(branch, leaf);
            __inca_native__.addEventListener(branch, 'click');
            __inca_native__.addEventListener(leaf, 'click');

            __inca_native__.destroyNode(branch);
            ",
        )
        .unwrap();

    let host = host.borrow();
    assert!(host.tree.get(host.root()).unwrap().children().is_empty());
    assert!(callback_ids(&host.listeners, crate::EventTargetId::Node(1), "click").is_empty());
}

#[test]
fn destroy_node_frees_the_scroll_handles_of_the_subtree_only() {
    let (engine, host) = engine_with_bindings();
    let ids: Vec<NodeId> = engine
        .eval(
            r"
            const root = __inca_native__.rootNodeId();
            const branch = __inca_native__.createNode('div');
            const leaf = __inca_native__.createNode('div');
            const kept = __inca_native__.createNode('div');
            __inca_native__.appendChild(root, branch);
            __inca_native__.appendChild(branch, leaf);
            __inca_native__.appendChild(root, kept);
            [branch, leaf, kept]
            ",
        )
        .unwrap();
    let (branch, leaf, kept) = (ids[0], ids[1], ids[2]);
    for id in [branch, leaf, kept] {
        host.borrow().scroll_handle(id);
    }

    engine
        .eval::<()>(&format!("__inca_native__.destroyNode({branch})"))
        .unwrap();

    let host = host.borrow();
    let handles = host.scroll_handles.borrow();
    assert!(!handles.contains_key(&branch));
    assert!(
        !handles.contains_key(&leaf),
        "a descendant's handle goes with its ancestor"
    );
    assert!(
        handles.contains_key(&kept),
        "a surviving node keeps its handle"
    );
}

#[test]
fn destroy_node_is_idempotent() {
    let (engine, _host) = engine_with_bindings();

    let second_pass: Vec<u32> = engine
        .eval(
            r"
            const node = __inca_native__.createNode('div');
            __inca_native__.destroyNode(node);
            __inca_native__.destroyNode(node);
            ",
        )
        .unwrap();

    assert!(second_pass.is_empty());
}

#[test]
fn destroying_the_root_throws_a_catchable_exception() {
    let (engine, host) = engine_with_bindings();

    let caught: bool = engine
        .eval(
            r"
            let caught = false;
            try {
                __inca_native__.destroyNode(__inca_native__.rootNodeId());
            } catch (e) {
                caught = true;
            }
            caught;
            ",
        )
        .unwrap();

    assert!(caught, "the root belongs to the host, not to the app");
    let host = host.borrow();
    assert!(host.tree.get(host.root()).is_some());
}

#[test]
fn moving_the_root_throws_a_catchable_exception() {
    let (engine, host) = engine_with_bindings();

    let caught: u32 = engine
        .eval(
            r"
            const root = __inca_native__.rootNodeId();
            const n = __inca_native__.createNode('div');
            let caught = 0;
            try {
                __inca_native__.appendChild(n, root);
            } catch (e) {
                if (e instanceof TypeError) caught += 1;
            }
            try {
                __inca_native__.insertBefore(n, root, null);
            } catch (e) {
                if (e instanceof TypeError) caught += 1;
            }
            __inca_native__.destroyNode(n);
            caught;
            ",
        )
        .unwrap();

    assert_eq!(caught, 2, "the root belongs to the host, not to the app");
    let host = host.borrow();
    let root = host.tree.get(host.root()).unwrap();
    assert_eq!(root.parent(), None);
}

#[test]
fn a_cycle_throws_hierarchy_request_error() {
    let (engine, _host) = engine_with_bindings();
    engine
        .eval::<()>(
            r"
            globalThis.a = __inca_native__.createNode('div');
            globalThis.c = __inca_native__.createNode('div');
            __inca_native__.appendChild(a, c);
            ",
        )
        .unwrap();

    assert_eq!(
        error_name(&engine, "__inca_native__.appendChild(c, a)"),
        "HierarchyRequestError"
    );
}

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
