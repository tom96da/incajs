// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn next_stop_orders_positive_values_then_zeros_in_tree_order() {
    assert_eq!(
        walk(&STOPS, None, false),
        [
            Some(2),
            Some(6),
            Some(1),
            Some(3),
            Some(4),
            None,
            Some(2),
            Some(6)
        ]
    );
    assert_eq!(
        walk(&STOPS, None, true),
        [
            Some(4),
            Some(3),
            Some(1),
            Some(6),
            Some(2),
            None,
            Some(4),
            Some(3)
        ]
    );
}

#[test]
fn next_stop_from_a_negative_stop_continues_in_tree_order() {
    assert_eq!(next_stop(&STOPS, Some(5), false), Some(6));
    assert_eq!(next_stop(&STOPS, Some(5), true), Some(4));
    let stops = [(1, 3), (2, -1), (3, 0), (4, 1)];
    assert_eq!(next_stop(&stops, Some(2), false), Some(3));
    assert_eq!(next_stop(&stops, Some(2), true), Some(1));
    let stops = [(1, 1), (2, 0), (3, -1)];
    assert_eq!(next_stop(&stops, Some(3), false), None);
    let stops = [(1, -1), (2, 1)];
    assert_eq!(next_stop(&stops, Some(1), true), None);
}

#[test]
fn next_stop_starts_at_an_end_from_an_unknown_node_and_ignores_empty_lists() {
    assert_eq!(next_stop(&STOPS, Some(99), false), Some(2));
    assert_eq!(next_stop(&STOPS, Some(99), true), Some(4));
    for backward in [false, true] {
        assert_eq!(next_stop(&[], None, backward), None);
        assert_eq!(next_stop(&[(1, -1)], None, backward), None);
    }
}

#[gpui::test]
fn tab_move_returns_the_node_it_focused_and_none_past_the_end(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
        assert_eq!(registry.tab_move(&tree, id, window, cx, false), Some(id));
        let _ = registry.apply_pending(&tree, window, cx);
        assert_eq!(registry.tab_move(&tree, id, window, cx, false), None);
    });
}

#[gpui::test]
fn a_tab_move_drops_the_scroll_requests_of_earlier_focus_calls(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, a) = tree_with("div", Some(AttributeValue::from("0")));
    let b = tree.create_node("div").unwrap();
    tree.set_attribute(b, "tabindex", "0").unwrap();
    tree.append_child(a, b).unwrap();
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(a);
    registry.mark_tab_dirty(b);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
        registry.request_focus(a, true);
        let _ = registry.apply_pending(&tree, window, cx);
        registry.retry_reveal(a);
        assert_eq!(registry.tab_move(&tree, a, window, cx, false), Some(b));
        assert_eq!(registry.take_reveals(), (vec![], vec![]));
    });
}

#[gpui::test]
fn the_last_focus_request_decides_what_scrolls(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, a) = tree_with("div", Some(AttributeValue::from("0")));
    let b = tree.create_node("div").unwrap();
    tree.set_attribute(b, "tabindex", "0").unwrap();
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(a);
    registry.mark_tab_dirty(b);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
        registry.request_focus(a, true);
        registry.request_focus(b, true);
        let _ = registry.apply_pending(&tree, window, cx);
        assert_eq!(registry.take_reveals(), (vec![], vec![b]));
        registry.request_focus(a, true);
        registry.request_focus(b, false);
        let _ = registry.apply_pending(&tree, window, cx);
        assert_eq!(registry.take_reveals(), (vec![], vec![]));
    });
}

#[gpui::test]
fn after_a_drop_no_node_is_focused_and_tab_starts_at_the_first_stop(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(2);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, ids[1]);
    cx.update(|window, cx| registry.observe(window, cx));
    registry.drop_focus_in(&tree, ids[1]);

    assert_eq!(
        cx.update(|window, cx| registry.focused_node(window, cx)),
        None
    );
    let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, false));

    assert_eq!(moved, Some(ids[0]));
}

#[gpui_cases]
#[gpui::test]
#[case::forward(false, 2)]
#[case::backward(true, 0)]
fn a_tab_move_from_a_negative_stop_continues_in_tree_order(
    cx: &mut TestAppContext,
    backward: bool,
    want: usize,
) {
    let cx = cx.add_empty_window();
    let (mut tree, ids) = rooted_tree(3);
    tree.set_attribute(ids[1], "tabindex", "-1").unwrap();
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, ids[1]);

    let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, backward));

    assert_eq!(moved, Some(ids[want]));
}

#[gpui::test]
fn a_tab_move_past_the_last_stop_leaves_no_node_focused(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(2);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, ids[1]);

    let focused = cx.update(|window, cx| {
        let _ = registry.tab_move(&tree, ROOT, window, cx, false);
        registry.focused_node(window, cx)
    });

    assert_eq!(focused, None);
}
