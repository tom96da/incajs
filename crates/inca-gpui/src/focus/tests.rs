// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::TestAppContext;
use rstest::rstest;

use super::*;

fn tree_with(tag: &str, tabindex: Option<AttributeValue>) -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node(tag).unwrap();
    if let Some(value) = tabindex {
        tree.set_attribute(id, "tabindex", value).unwrap();
    }
    (tree, id)
}

#[test]
fn tab_index_parsing_follows_the_leading_integer() {
    let cases: [(&str, Option<i32>); 14] = [
        (" 1.5", Some(1)),
        ("+1", Some(1)),
        ("-0", Some(0)),
        ("-1", Some(-1)),
        ("\t\n 7x", Some(7)),
        ("1e3", Some(1)),
        ("abc", None),
        ("", None),
        ("+", None),
        ("-", None),
        (".5", None),
        ("2147483648", None),
        ("-2147483649", None),
        ("2147483647", Some(i32::MAX)),
    ];
    let mismatches: Vec<_> = cases
        .iter()
        .filter(|(text, want)| parse_tab_index(text) != *want)
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

#[test]
fn tab_index_of_reads_strings_numbers_and_booleans() {
    let cases = [
        (AttributeValue::from("3"), Some(3)),
        (AttributeValue::Number(0.0), Some(0)),
        (AttributeValue::Number(-1.0), Some(-1)),
        (AttributeValue::Number(1.5), Some(1)),
        (AttributeValue::Number(1e21), Some(1)),
        (AttributeValue::Number(2.5e-7), Some(2)),
        (AttributeValue::Number(f64::NAN), None),
        (AttributeValue::Bool(true), None),
        (AttributeValue::Bool(false), None),
    ];
    for (value, want) in cases {
        let (tree, id) = tree_with("div", Some(value.clone()));
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{value:?}");
    }
    let (tree, id) = tree_with("div", None);
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    assert_eq!(FocusRegistry::tab_index_of(&tree, 999), None);
}

#[test]
fn a_text_node_and_a_display_none_node_have_no_tab_index() {
    let (tree, id) = tree_with("text", Some(AttributeValue::from("0")));
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    tree.set_style(id, "display", "none").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    tree.set_style(id, "display", "flex").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
}

#[rstest]
#[case::display("display")]
#[case::hidden("hidden")]
#[case::inert("inert")]
fn a_node_under_display_none_hidden_or_inert_has_no_tab_index(#[case] hide: &str) {
    let mut mismatches = Vec::new();
    let (mut tree, child) = tree_with("div", Some(AttributeValue::from("0")));
    let parent = tree.create_node("div").unwrap();
    tree.append_child(parent, child).unwrap();
    for (target, name) in [(child, "self"), (parent, "ancestor")] {
        if hide == "display" {
            tree.set_style(target, "display", "none").unwrap();
        } else {
            tree.set_attribute(target, hide, true).unwrap();
        }
        if FocusRegistry::tab_index_of(&tree, child).is_some() {
            mismatches.push(format!("{hide} on {name} keeps the tab index"));
        }
        if hide == "display" {
            tree.remove_style(target, "display").unwrap();
        } else {
            tree.remove_attribute(target, hide).unwrap();
        }
        if FocusRegistry::tab_index_of(&tree, child) != Some(0) {
            mismatches.push(format!("removing {hide} on {name} loses the tab index"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn hidden_with_an_own_display_or_until_found_keeps_the_tab_index() {
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    tree.set_attribute(id, "hidden", "").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    tree.set_style(id, "display", "flex").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
    tree.remove_style(id, "display").unwrap();
    tree.set_attribute(id, "hidden", "until-found").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
}

#[gpui::test]
fn mark_subtree_dirty_queues_nodes_that_can_hold_focus(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
    let parent = tree.create_node("div").unwrap();
    let plain = tree.create_node("div").unwrap();
    let button = tree.create_node("button").unwrap();
    let held = tree.create_node("div").unwrap();
    for child in [focusable, plain, button, held] {
        tree.append_child(parent, child).unwrap();
    }
    let mut registry = FocusRegistry::default();
    cx.update(|_, cx| registry.get_or_create(held, cx));
    registry.mark_subtree_dirty(&tree, parent);
    assert_eq!(registry.tab_dirty, HashSet::from([focusable, button, held]));
}

#[rstest]
#[case::absent(None, Some(0))]
#[case::positive(Some(AttributeValue::from("3")), Some(3))]
#[case::negative(Some(AttributeValue::from("-1")), Some(-1))]
#[case::not_a_number(Some(AttributeValue::from("abc")), Some(0))]
#[case::boolean(Some(AttributeValue::Bool(true)), Some(0))]
fn a_button_has_tab_index_0_unless_told_otherwise(
    #[case] value: Option<AttributeValue>,
    #[case] want: Option<i32>,
) {
    let (tree, id) = tree_with("button", value.clone());
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{value:?}");
}

#[rstest]
#[case::button("button", None)]
#[case::div("div", Some(2))]
fn a_disabled_button_has_no_tab_index_but_a_disabled_div_keeps_it(
    #[case] tag: &str,
    #[case] want: Option<i32>,
) {
    let (mut tree, id) = tree_with(tag, Some(AttributeValue::from("2")));
    tree.set_attribute(id, "disabled", true).unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{tag}");
    tree.remove_attribute(id, "disabled").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(2), "{tag}");
}

// Nodes 1 to 6 have tabindex 3, 1, 0, 0, -1 and 2.
const STOPS: [(NodeId, i32); 6] = [(1, 3), (2, 1), (3, 0), (4, 0), (5, -1), (6, 2)];

fn walk(stops: &[(NodeId, i32)], start: Option<NodeId>, backward: bool) -> Vec<Option<NodeId>> {
    let mut current = start;
    let mut moves = Vec::new();
    for _ in 0..stops.len() + 2 {
        let target = next_stop(stops, current, backward);
        moves.push(target);
        current = target;
    }
    moves
}

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
fn removing_tabindex_blurs_the_focused_node_and_drops_the_handle(cx: &mut TestAppContext) {
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

    assert!(registry.handle(id).is_none());
    assert!(!cx.update(|window, _| held.is_focused(window)));
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].blurred, Some(id));
    assert_eq!(transitions[0].focused, None);
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
