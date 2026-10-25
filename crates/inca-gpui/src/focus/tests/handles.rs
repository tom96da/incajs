// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui::test]
fn changing_tabindex_keeps_the_focused_handle(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.request_focus(id, true);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
    });
    let before = registry.handle(id).unwrap();

    tree.set_attribute(id, "tabindex", "-1").unwrap();
    registry.mark_tab_dirty(id);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    let after = registry.handle(id).unwrap();
    assert_eq!(before, after);
    assert!(transitions.is_empty());
    assert!(cx.update(|window, _| after.is_focused(window)));
}

#[gpui::test]
fn removing_tabindex_blurs_the_focused_node(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.request_focus(id, true);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
    });
    let held = registry.handle(id).unwrap();
    assert!(cx.update(|window, _| held.is_focused(window)));

    tree.remove_attribute(id, "tabindex").unwrap();
    registry.mark_tab_dirty(id);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert!(!cx.update(|window, _| held.is_focused(window)));
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].blurred, Some(id));
    assert_eq!(transitions[0].focused, None);
}

#[gpui::test]
fn removing_tabindex_drops_the_focus_handle(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.request_focus(id, true);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
    });
    let held = registry.handle(id).unwrap();
    assert!(cx.update(|window, _| held.is_focused(window)));

    tree.remove_attribute(id, "tabindex").unwrap();
    registry.mark_tab_dirty(id);
    let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert!(registry.handle(id).is_none());
}

#[gpui::test]
fn focus_on_an_untracked_node_is_ignored(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, id) = tree_with("div", None);
    let mut registry = FocusRegistry::default();
    registry.request_focus(id, true);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
    assert!(transitions.is_empty());
    assert!(registry.handle(id).is_none());
}

#[gpui::test]
fn removing_tabindex_blurs_a_node_a_click_focused_before_the_next_sync(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
    });
    let held = registry.handle(id).unwrap();
    // Focus moved outside the registry, as a click does.
    cx.update(|window, cx| held.focus(window, cx));

    tree.remove_attribute(id, "tabindex").unwrap();
    registry.mark_tab_dirty(id);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert!(!cx.update(|window, _| held.is_focused(window)));
    let events: Vec<_> = transitions.iter().map(|t| (t.blurred, t.focused)).collect();
    assert_eq!(events, [(None, Some(id)), (Some(id), None)]);
}

#[gpui::test]
fn forget_clears_a_queued_tab_check(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.forget(id);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
    });
    assert!(registry.handle(id).is_none());
}

#[gpui::test]
fn get_or_create_reuses_the_same_handle_for_one_node(cx: &mut TestAppContext) {
    let mut registry = FocusRegistry::default();
    let cx = cx.add_empty_window();

    let (first, second) = cx.update(|_, cx| {
        let first = registry.get_or_create(1, cx);
        let second = registry.get_or_create(1, cx);
        (first, second)
    });

    assert_eq!(first, second);
}

#[gpui::test]
fn forget_lets_a_later_call_mint_a_new_handle(cx: &mut TestAppContext) {
    let mut registry = FocusRegistry::default();
    let cx = cx.add_empty_window();

    let before = cx.update(|_, cx| registry.get_or_create(1, cx));
    registry.forget(1);
    let after = cx.update(|_, cx| registry.get_or_create(1, cx));

    assert_ne!(before, after);
    assert!(registry.handle(1).is_some());
}

#[gpui::test]
fn focused_node_names_the_node_holding_focus(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
        assert_eq!(registry.focused_node(window, cx), None);
        registry.request_focus(id, true);
        let _ = registry.apply_pending(&tree, window, cx);
        assert_eq!(registry.focused_node(window, cx), Some(id));
    });
}

#[gpui::test]
fn observe_takes_the_node_the_window_focuses(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
        registry.handle(id).unwrap().focus(window, cx);
        registry.observe(window, cx);
    });
    assert_eq!(registry.active_node(&tree), Some(id));
}

#[gpui::test]
fn a_pending_focus_on_a_node_with_a_handle_is_active(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(&tree, window, cx);
    });
    registry.request_focus(id, true);
    assert_eq!(registry.active_node(&tree), Some(id));
}

#[test]
fn a_pending_blur_of_the_observed_node_clears_it() {
    let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.observed = Some(id);
    registry.request_blur(id);
    assert_eq!(registry.active_node(&tree), None);
}

#[test]
fn a_pending_blur_of_another_node_keeps_the_observed_one() {
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let other = tree.create_node("div").unwrap();
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.observed = Some(id);
    registry.request_blur(other);
    assert_eq!(registry.active_node(&tree), Some(id));
}

#[test]
fn a_pending_focus_on_an_unfocusable_node_changes_nothing() {
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let plain = tree.create_node("div").unwrap();
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.observed = Some(id);
    registry.request_focus(plain, true);
    assert_eq!(registry.active_node(&tree), Some(id));
}

#[test]
fn a_tab_dirty_node_is_focusable_only_with_a_tab_index() {
    let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
    let plain = tree.create_node("div").unwrap();
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(focusable);
    registry.mark_tab_dirty(plain);
    registry.request_focus(plain, true);
    assert_eq!(registry.active_node(&tree), None);
    registry.request_focus(focusable, true);
    assert_eq!(registry.active_node(&tree), Some(focusable));
}

#[test]
fn a_tab_dirty_node_that_loses_its_tab_index_drops_out_of_active() {
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.observed = Some(id);
    assert_eq!(registry.active_node(&tree), Some(id));
    tree.remove_attribute(id, "tabindex").unwrap();
    assert_eq!(registry.active_node(&tree), None);
}

/// Parking the window focus on the root container reports no transition,
/// maps to no node and does not flip per frame.
#[gpui::test]
fn the_parked_focus_fires_no_focus_event(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, _root) = tree_with("div", None);
    let mut registry = FocusRegistry::default();
    cx.update(|window, cx| {
        for _ in 0..2 {
            let transitions = registry.apply_pending(&tree, window, cx);
            assert!(transitions.is_empty());
            assert!(window.focused(cx).is_some());
            assert_eq!(registry.focused_node(window, cx), None);
        }
    });
}

/// A root with `tabindex` takes focus like any node, and the parked focus
/// leaves it alone.
#[gpui::test]
fn a_root_with_tabindex_still_focuses_normally(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, root) = tree_with("div", Some(AttributeValue::from("0")));
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(root);
    cx.update(|window, cx| {
        assert!(registry.apply_pending(&tree, window, cx).is_empty());
        registry.request_focus(root, true);
        let transitions = registry.apply_pending(&tree, window, cx);
        assert_eq!(transitions.len(), 1);
        assert_eq!(registry.focused_node(window, cx), Some(root));
        assert!(registry.apply_pending(&tree, window, cx).is_empty());
    });
}

#[gpui::test]
fn a_disconnected_focused_node_loses_focus_without_a_transition(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, ids) = rooted_tree(1);
    let id = ids[0];
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, id);
    let held = registry.handle(id).unwrap();
    assert!(cx.update(|window, _| held.is_focused(window)));

    tree.remove_child(ROOT, id).unwrap();
    registry.mark_tab_dirty(id);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert!(transitions.is_empty());
    assert!(registry.handle(id).is_none());
    assert!(!cx.update(|window, _| held.is_focused(window)));
    assert_eq!(
        cx.update(|window, cx| registry.focused_node(window, cx)),
        None
    );
}

#[gpui::test]
fn a_connected_node_losing_tabindex_reports_the_blur(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, ids) = rooted_tree(1);
    let id = ids[0];
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, id);

    tree.remove_attribute(id, "tabindex").unwrap();
    registry.mark_tab_dirty(id);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].blurred, Some(id));
}

#[gpui::test]
fn a_pending_focus_takes_effect_after_a_silent_drop(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, ids) = rooted_tree(2);
    let (gone, other) = (ids[0], ids[1]);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, gone);

    tree.remove_child(ROOT, gone).unwrap();
    registry.mark_tab_dirty(gone);
    registry.request_focus(other, false);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert_eq!(
        cx.update(|window, cx| registry.focused_node(window, cx)),
        Some(other)
    );
    let events: Vec<_> = transitions.iter().map(|t| (t.blurred, t.focused)).collect();
    assert_eq!(events, [(None, Some(other))]);
}

#[gpui::test]
fn dropping_focus_is_silent(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(2);
    let gone = ids[0];
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, gone);
    cx.update(|window, cx| registry.observe(window, cx));
    registry.request_focus(gone, false);

    registry.drop_focus_in(&tree, gone);

    assert_eq!(registry.active_node(&tree), None);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
    assert!(transitions.is_empty());
}

#[gpui::test]
fn dropping_focus_releases_the_window_at_the_next_apply(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(2);
    let (gone, other) = (ids[0], ids[1]);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, gone);
    cx.update(|window, cx| registry.observe(window, cx));
    registry.request_focus(gone, false);

    registry.drop_focus_in(&tree, gone);

    let held = registry.handle(gone).unwrap();
    let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
    assert!(!cx.update(|window, _| held.is_focused(window)));
    assert!(registry.handle(other).is_some());
}

#[gpui::test]
fn dropping_focus_in_an_unrelated_subtree_keeps_the_focus(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(2);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, ids[0]);
    cx.update(|window, cx| registry.observe(window, cx));

    registry.drop_focus_in(&tree, ids[1]);

    assert_eq!(registry.active_node(&tree), Some(ids[0]));
    let held = registry.handle(ids[0]).unwrap();
    let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
    assert!(cx.update(|window, _| held.is_focused(window)));
}

#[gpui::test]
fn a_node_focused_elsewhere_after_a_drop_keeps_its_focus(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(2);
    let (gone, other) = (ids[0], ids[1]);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, gone);
    cx.update(|window, cx| registry.observe(window, cx));
    registry.drop_focus_in(&tree, gone);
    // Focus moved outside the registry, as a click does.
    let held = registry.handle(other).unwrap();
    cx.update(|window, cx| held.focus(window, cx));

    let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert!(cx.update(|window, _| held.is_focused(window)));
}

#[gpui::test]
fn a_dropped_node_is_not_revealed_on_the_next_frame(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(2);
    let (gone, kept) = (ids[0], ids[1]);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, gone);
    registry.request_focus(gone, true);
    let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
    registry.retry_reveal(gone);
    registry.retry_reveal(kept);

    registry.drop_focus_in(&tree, gone);

    assert_eq!(registry.take_reveals(), (vec![kept], vec![]));
}

#[gpui::test]
fn a_drop_on_an_inactive_window_leaves_nothing_owed_after_the_frame(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, ids) = rooted_tree(1);
    let id = ids[0];
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &ids, id);
    cx.update(|window, cx| registry.observe(window, cx));
    registry.set_window_blurred(true);
    registry.drop_focus_in(&tree, id);

    let first = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
    registry.set_window_blurred(false);
    let held = registry.handle(id).unwrap();
    cx.update(|window, cx| held.focus(window, cx));
    let second = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert!(first.is_empty());
    let events: Vec<_> = second.iter().map(|t| (t.blurred, t.focused)).collect();
    assert_eq!(events, [(None, Some(id))]);
    assert!(cx.update(|window, _| held.is_focused(window)));
}

#[test]
fn a_transition_names_its_nodes() {
    let lost = FocusTransition::window_activation(3, false);
    assert_eq!((lost.blurred(), lost.focused()), (Some(3), None));
    let regained = FocusTransition::window_activation(3, true);
    assert_eq!((regained.blurred(), regained.focused()), (None, Some(3)));
}
