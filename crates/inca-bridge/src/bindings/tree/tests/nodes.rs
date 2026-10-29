// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::super::*;
use crate::test_support::{callback_ids, engine_with_bindings, error_name};

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
