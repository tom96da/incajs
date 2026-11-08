// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tab order and tab-index reads.

use super::{
    App, AttributeValue, FocusRegistry, NodeId, PendingFocus, Start, TEXT_TAG, VirtualTree, Window,
    focusable_by_default, hidden_by_attribute, is_disabled_control, js_number_text,
};

/// Parses a `tabindex` value: optional leading ASCII whitespace, an optional
/// sign, then digits. Trailing characters are ignored. Returns `None` when the
/// digits are missing or exceed `i32`.
fn parse_tab_index(value: &str) -> Option<i32> {
    let value = value.trim_start_matches([' ', '\t', '\n', '\x0c', '\r']);
    let (negative, rest) = match value.as_bytes().first()? {
        b'-' => (true, &value[1..]),
        b'+' => (false, &value[1..]),
        _ => (false, value),
    };
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if digits == 0 {
        return None;
    }
    let magnitude: i64 = rest[..digits].parse().ok()?;
    i32::try_from(if negative { -magnitude } else { magnitude }).ok()
}

/// The target of a Tab press. `stops` lists every focusable node with its
/// `tabindex` in tree order. Positive values come first in ascending order,
/// then 0 in tree order, and negative values are outside the order. Past either
/// end the result is `None`. From a node outside the order the result is the
/// nearest stop after it (before it when `backward`) in tree order, or forward
/// with no stop after it the first stop. `current` `None` starts at the first
/// (or, `backward`, last) stop.
fn next_stop(stops: &[(NodeId, i32)], current: Option<NodeId>, backward: bool) -> Option<NodeId> {
    // (sort key, tree position, node)
    let mut ordered: Vec<(i32, usize, NodeId)> = stops
        .iter()
        .enumerate()
        .filter(|(_, (_, index))| *index >= 0)
        .map(|(pos, &(id, index))| (if index > 0 { index } else { i32::MAX }, pos, id))
        .collect();
    ordered.sort_by_key(|&(key, ..)| key);
    let at = current.and_then(|id| ordered.iter().position(|&(_, _, node)| node == id));
    let found = match (current, at) {
        (Some(_), Some(at)) => {
            if backward {
                at.checked_sub(1).and_then(|i| ordered.get(i))
            } else {
                ordered.get(at + 1)
            }
        }
        (Some(id), None) => match stops.iter().position(|&(node, _)| node == id) {
            Some(here) if backward => ordered.iter().filter(|s| s.1 < here).max_by_key(|s| s.1),
            Some(here) => ordered
                .iter()
                .filter(|s| s.1 > here)
                .min_by_key(|s| s.1)
                .or_else(|| ordered.first()),
            None if backward => ordered.last(),
            None => ordered.first(),
        },
        (None, _) if backward => ordered.last(),
        (None, _) => ordered.first(),
    };
    found.map(|&(_, _, id)| id)
}

/// The target of a Tab press while no node is focused, from a press on `node`, which sits at
/// `at` among `stops` in tree order. Forward, it is the first stop after `node`, or with none
/// after it the first stop of the order when `node` is outside the order. Backward, it is the
/// last stop up to `node` itself.
fn after_press(stops: &[(NodeId, i32)], node: NodeId, at: usize, backward: bool) -> Option<NodeId> {
    let own = stops.get(at).filter(|&&(id, _)| id == node);
    let split = at + usize::from(own.is_some());
    let in_order = |stop: &&(NodeId, i32)| stop.1 >= 0;
    if backward {
        return stops[..split]
            .iter()
            .rev()
            .find(in_order)
            .map(|&(id, _)| id);
    }
    match stops[split..].iter().find(in_order) {
        Some(&(id, _)) => Some(id),
        None if own.is_some_and(|&(_, index)| index >= 0) => None,
        None => next_stop(stops, None, false),
    }
}

impl FocusRegistry {
    /// Moves focus one tab stop forward, or `backward`. With no node focused,
    /// the move starts from the last press or focus (see
    /// [`Self::press_start`]). Past the last stop nothing holds focus, and the
    /// next Tab starts over at the first. A move that finds no stop forgets
    /// where it started. The transition is reported by the next
    /// [`Self::apply_pending`]. Returns the node that took focus. The move
    /// also drops the scroll requests of earlier focus calls.
    pub fn tab_move(
        &mut self,
        tree: &VirtualTree,
        root: NodeId,
        window: &mut Window,
        cx: &mut App,
        backward: bool,
    ) -> Option<NodeId> {
        let focused = self.focused_node(window, cx);
        let start = if focused.is_none() { self.start } else { None };
        let mut stops = Vec::new();
        let mut start_at = None;
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if start.is_some_and(|start| start.node() == id) {
                start_at = Some(stops.len());
            }
            if self.handles.contains_key(&id)
                && let Some(index) = Self::tab_index_of(tree, id)
            {
                stops.push((id, index));
            }
            stack.extend(node.children().iter().rev());
        }
        let target = match (start, start_at) {
            (Some(Start::Press { node, .. }), Some(at)) => after_press(&stops, node, at, backward),
            (Some(Start::Focus(node)), _) => next_stop(&stops, Some(node), backward),
            _ => next_stop(&stops, focused, backward),
        };
        if target.is_none() {
            self.start = None;
        }
        self.reveal.clear();
        self.retry.clear();
        if let Some((id, handle)) = target.and_then(|id| Some((id, self.handles.get(&id)?))) {
            handle.focus(window, cx);
            Some(id)
        } else {
            window.blur(cx);
            None
        }
    }

    /// Whether `node_id` can hold focus once this frame's tab check has run.
    /// A node in `tab_dirty` holds it when it has a tab index, and any other
    /// node holds it when it has a handle.
    pub(super) fn focusable_after_frame(&self, tree: &VirtualTree, node_id: NodeId) -> bool {
        if self.tab_dirty.contains(&node_id) {
            Self::tab_index_of(tree, node_id).is_some()
        } else {
            self.handles.contains_key(&node_id)
        }
    }

    /// Takes focus from `node_id` and from every node below it at once. No
    /// `blur` or `focusout` is reported, and requests to focus those nodes
    /// are dropped. The window's own focus is released by the next
    /// [`Self::apply_pending`].
    pub fn drop_focus_in(&mut self, tree: &VirtualTree, node_id: NodeId) {
        let inside = |id: NodeId| tree.ancestors(id).any(|ancestor| ancestor == node_id);
        self.pending.retain(
            |request| !matches!(request, PendingFocus::Focus { node, .. } if inside(*node)),
        );
        self.reveal.retain(|&id| !inside(id));
        self.retry.retain(|&id| !inside(id));
        if let Some(held) = self.observed.filter(|&id| inside(id)) {
            self.observed = None;
            self.blur_owed = Some(held);
        }
        if self.focused.is_some_and(inside) {
            self.focused = None;
        }
    }

    /// Records that `node_id`'s `tabindex` may have changed. The next
    /// [`Self::apply_pending`] re-reads it.
    pub fn mark_tab_dirty(&mut self, node_id: NodeId) {
        self.tab_dirty.insert(node_id);
    }

    /// Takes the nodes marked by [`Self::mark_tab_dirty`] and its subtree form.
    #[cfg(feature = "test-support")]
    pub fn take_tab_dirty(&mut self) -> std::collections::HashSet<NodeId> {
        std::mem::take(&mut self.tab_dirty)
    }

    /// Like [`Self::mark_tab_dirty`] for `node_id` and its descendants that
    /// have, or could take, a handle.
    pub fn mark_subtree_dirty(&mut self, tree: &VirtualTree, node_id: NodeId) {
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if self.handles.contains_key(&id)
                || focusable_by_default(node.tag_name())
                || node.attributes().contains_key("tabindex")
            {
                self.tab_dirty.insert(id);
            }
            stack.extend(node.children());
        }
    }

    /// The `tabindex` of `node_id`. A node not connected to the tree's document,
    /// a text node, a node that has an ancestor (or itself) that is a template or has
    /// `display: none`, `inert` or a hiding `hidden`, and a `disabled` button
    /// or input have none. A button or input whose `tabindex` is missing or
    /// unparsable has 0.
    pub(super) fn tab_index_of(tree: &VirtualTree, node_id: NodeId) -> Option<i32> {
        let node = tree.get(node_id)?;
        if !tree.is_connected(node_id) {
            return None;
        }
        let hidden = tree.ancestors(node_id).any(|id| {
            hidden_by_attribute(tree, id) || tree.get(id).is_some_and(|ancestor| {
                ancestor.is_template()
                    || ancestor.attributes().contains_key("inert")
                    || matches!(
                        tree.computed_style(id).get("display"),
                        Some(AttributeValue::String(value)) if value.eq_ignore_ascii_case("none")
                    )
            })
        });
        let by_default = focusable_by_default(node.tag_name());
        if node.tag_name() == TEXT_TAG || hidden || is_disabled_control(node) {
            return None;
        }
        let explicit = match node.attributes().get("tabindex") {
            Some(AttributeValue::String(value)) => parse_tab_index(value),
            Some(AttributeValue::Number(value)) => parse_tab_index(&js_number_text(*value)),
            Some(AttributeValue::Bool(_)) | None => None,
        };
        explicit.or(by_default.then_some(0))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use gpui::TestAppContext;
    use inca_test_macros::gpui_cases;
    use rstest::rstest;
    use std::collections::HashSet;

    // Nodes 1 to 6 have tabindex 3, 1, 0, 0, -1 and 2.
    /// A rooted tree with one `button`, disabled when `disabled`.
    fn rooted_button(disabled: bool) -> (VirtualTree, NodeId) {
        let (mut tree, _) = rooted_tree(0);
        let id = tree.create_node("button").unwrap();
        tree.append_child(ROOT, id).unwrap();
        if disabled {
            tree.set_attribute(id, "disabled", true).unwrap();
        }
        (tree, id)
    }

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
    fn a_button_under_a_template_has_no_tab_index() {
        let (mut tree, button) = rooted_button(false);
        let template = tree.create_node("template").unwrap();
        tree.append_child(ROOT, template).unwrap();
        tree.append_child(template, button).unwrap();

        assert_eq!(FocusRegistry::tab_index_of(&tree, button), None);
    }

    #[test]
    fn a_text_node_and_a_display_none_node_have_no_tab_index() {
        let (tree, id) = tree_with("#text", Some(AttributeValue::from("0")));
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        tree.set_style(id, "display", "none").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
        tree.set_style(id, "display", "flex").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
    }

    #[test]
    fn a_display_none_in_capitals_leaves_no_tab_index() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        tree.set_style(id, "display", "NONE").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
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

    /// A `<style>` element under the root with a rule that hides `.h`.
    fn hide_class_h(tree: &mut VirtualTree) {
        let style = tree.create_node("style").unwrap();
        tree.append_child(ROOT, style).unwrap();
        let css = tree.create_node("#text").unwrap();
        tree.set_attribute(css, "value", ".h { display: none }")
            .unwrap();
        tree.append_child(style, css).unwrap();
    }

    #[test]
    fn a_node_under_a_rule_display_none_has_no_tab_index() {
        let (mut tree, _) = rooted_tree(0);
        hide_class_h(&mut tree);
        let parent = tree.create_node("div").unwrap();
        tree.append_child(ROOT, parent).unwrap();
        tree.set_attribute(parent, "class", "h").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(parent, child).unwrap();
        tree.set_attribute(child, "tabindex", "0").unwrap();

        assert_eq!(FocusRegistry::tab_index_of(&tree, child), None);
    }

    #[test]
    fn a_node_with_a_rule_display_none_has_no_tab_index() {
        let (mut tree, _) = rooted_tree(0);
        hide_class_h(&mut tree);
        let node = tree.create_node("div").unwrap();
        tree.append_child(ROOT, node).unwrap();
        tree.set_attribute(node, "class", "h").unwrap();
        tree.set_attribute(node, "tabindex", "0").unwrap();

        assert_eq!(FocusRegistry::tab_index_of(&tree, node), None);
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
        let input = tree.create_node("input").unwrap();
        let held = tree.create_node("div").unwrap();
        for child in [focusable, plain, button, input, held] {
            tree.append_child(parent, child).unwrap();
        }
        let mut registry = FocusRegistry::default();
        cx.update(|_, cx| registry.get_or_create(held, cx));
        registry.mark_subtree_dirty(&tree, parent);
        assert_eq!(
            registry.tab_dirty,
            HashSet::from([focusable, button, input, held])
        );
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

    #[test]
    fn an_input_has_tab_index_0_without_a_tabindex() {
        let (tree, id) = tree_with("input", None);
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
    }

    #[rstest]
    #[case::button("button", None)]
    #[case::input("input", None)]
    #[case::div("div", Some(2))]
    fn a_disabled_attribute_drops_the_tab_index_of_a_control_only(
        #[case] tag: &str,
        #[case] want: Option<i32>,
    ) {
        let (mut tree, id) = tree_with(tag, Some(AttributeValue::from("2")));
        tree.set_attribute(id, "disabled", true).unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{tag}");
        tree.remove_attribute(id, "disabled").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(2), "{tag}");
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
        assert_eq!(next_stop(&stops, Some(3), false), Some(1));
        let stops = [(1, -1), (2, 1)];
        assert_eq!(next_stop(&stops, Some(1), true), None);
    }

    #[test]
    fn next_stop_from_an_unknown_node_starts_at_an_end() {
        assert_eq!(next_stop(&STOPS, Some(99), false), Some(2));
        assert_eq!(next_stop(&STOPS, Some(99), true), Some(4));
    }

    #[rstest]
    #[case::forward(false)]
    #[case::backward(true)]
    fn next_stop_is_none_without_a_reachable_stop(#[case] backward: bool) {
        assert_eq!(next_stop(&[], None, backward), None);
        assert_eq!(next_stop(&[(1, -1)], None, backward), None);
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
    fn after_a_drop_no_node_is_focused(cx: &mut TestAppContext) {
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
    }

    #[gpui::test]
    fn after_a_drop_tab_continues_after_the_dropped_node(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(3);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.drop_focus_in(&tree, ids[1]);

        let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, false));

        assert_eq!(moved, Some(ids[2]));
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

    #[gpui::test]
    fn focus_on_a_disabled_button_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = rooted_button(true);
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, false);

        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(transitions.is_empty());
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            None
        );
    }

    #[gpui::test]
    fn disabling_a_focused_button_reports_its_blur(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = rooted_button(false);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &[id], id);

        tree.set_attribute(id, "disabled", true).unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred, Some(id));
        assert_eq!(transitions[0].focused, None);
    }

    #[gpui::test]
    fn an_enabled_button_takes_focus_again(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = rooted_button(false);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &[id], id);
        tree.set_attribute(id, "disabled", true).unwrap();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        tree.remove_attribute(id, "disabled").unwrap();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, false);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred, None);
        assert_eq!(transitions[0].focused, Some(id));
    }
}
