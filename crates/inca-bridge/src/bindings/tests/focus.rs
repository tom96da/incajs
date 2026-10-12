// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn only_focus_attributes_and_display_mark_a_node_for_a_focus_check() {
    let (engine, host) = engine_with_bindings();
    let node: u32 = engine
        .eval("globalThis.n = __inca_native__.createNode('div')")
        .unwrap();
    let dirty = |host: &Rc<RefCell<Host>>| std::mem::take(&mut host.borrow_mut().focus.tab_dirty);
    assert!(dirty(&host).is_empty());

    engine
        .eval::<()>(
            "__inca_native__.setAttribute(n, 'label', 'x'); \
             __inca_native__.removeAttribute(n, 'label'); \
             __inca_native__.setStyle(n, 'width', 5); \
             __inca_native__.removeStyle(n, 'width');",
        )
        .unwrap();
    assert!(dirty(&host).is_empty());

    // A subtree check queues only nodes that have a tabindex.
    for call in [
        "setAttribute(n, 'tabindex', '0')",
        "setAttribute(n, 'tabIndex', 0)",
        "setAttribute(n, 'disabled', true)",
        "removeAttribute(n, 'disabled')",
        "setStyle(n, 'display', 'none')",
        "removeStyle(n, 'display')",
        "setAttribute(n, 'hidden', true)",
        "removeAttribute(n, 'hidden')",
        "setAttribute(n, 'inert', true)",
        "removeAttribute(n, 'inert')",
        "removeAttribute(n, 'TABINDEX')",
    ] {
        engine
            .eval::<()>(&format!("__inca_native__.{call}"))
            .unwrap();
        assert!(dirty(&host).contains(&node), "{call}");
    }
    engine
        .eval::<()>("__inca_native__.setAttribute(n, 'tabIndex', 1)")
        .unwrap();
    let host = host.borrow();
    let attrs = host.tree.get(node).unwrap().attributes();
    assert_eq!(attrs.get("tabindex"), Some(&AttributeValue::Number(1.0)));
    assert!(!attrs.contains_key("tabIndex"));
}

#[test]
fn focused_node_follows_focus_node_and_blur_node() {
    let (engine, _host) = engine_with_bindings();

    let flags: Vec<bool> = engine
        .eval(
            r"
            const n = __inca_native__;
            const node = n.createNode('div');
            const other = n.createNode('div');
            const plain = n.createNode('div');
            n.setAttribute(node, 'tabindex', 0);
            n.setAttribute(other, 'tabindex', 0);
            const before = n.focusedNode() === null;
            n.focusNode(plain);
            const plainSkipped = n.focusedNode() === null;
            n.focusNode(node);
            const focused = n.focusedNode() === node;
            n.blurNode(other);
            const kept = n.focusedNode() === node;
            n.blurNode(node);
            [before, plainSkipped, focused, kept, n.focusedNode() === null];
            ",
        )
        .unwrap();

    assert_eq!(flags, [true; 5]);
}
