// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tab order and tab-index reads.

use super::{FocusModel, FocusSurface, NodeId, PendingFocus, Start, StyleCache, VirtualTree};
use crate::TabIndex;
use inca_node::{AttributeValue, is_disabled_control};
use inca_style::hidden_by_attribute;

impl FocusModel {
    /// Moves focus one tab stop forward, or `backward`. With no node focused,
    /// the move starts from the last press or focus (see
    /// [`Self::press_start`]). Past the last stop nothing holds focus, and the
    /// next Tab starts over at the first. A move that finds no stop forgets
    /// where it started. The transition is reported by the next
    /// [`Self::apply_pending`]. Returns the node that took focus. The move
    /// also drops the scroll requests of earlier focus calls. From a node that no longer takes
    /// focus, the move goes to the nearest stop after its place in tree order (before it when
    /// `backward`), or forward with none after it to the first stop.
    pub fn tab_move(
        &mut self,
        tree: &VirtualTree,
        cache: &StyleCache,
        root: NodeId,
        surface: &mut impl FocusSurface,
        backward: bool,
    ) -> Option<NodeId> {
        let focused = self.held_node(surface.focused());
        let start = if focused.is_none() { self.start } else { None };
        let mut stops = Vec::new();
        let from = start.map(Start::node).or(focused);
        let mut from_at = None;
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if from == Some(id) {
                from_at = Some(stops.len());
            }
            if self.tracked.contains(&id)
                && let Some(index) = Self::tab_index_of(tree, cache, id)
            {
                stops.push((id, index));
            }
            stack.extend(node.children().iter().rev());
        }
        let target = match (start, from.zip(from_at)) {
            (Some(Start::Press { .. }), Some((node, at))) => {
                after_press(&stops, node, at, backward)
            }
            (_, Some((node, at))) if stops.get(at).is_none_or(|stop| stop.0 != node) => {
                after_press(&stops, node, at, backward)
            }
            (Some(Start::Focus(node)), _) => next_stop(&stops, Some(node), backward),
            _ => next_stop(&stops, focused, backward),
        };
        if target.is_none() {
            self.start = None;
        }
        self.reveal.clear();
        self.retry.clear();
        if let Some(id) = target {
            surface.focus(id);
        } else {
            surface.blur();
        }
        target
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
}

impl FocusModel {
    /// Whether `node_id` can hold focus once this frame's tab check has run.
    /// A node in `tab_dirty` holds it when it has a tab index, and any other
    /// node holds it when it has a handle.
    pub(super) fn focusable_after_frame(
        &self,
        tree: &VirtualTree,
        cache: &StyleCache,
        node_id: NodeId,
    ) -> bool {
        if self.tab_dirty.contains(&node_id) {
            Self::tab_index_of(tree, cache, node_id).is_some()
        } else {
            self.tracked.contains(&node_id)
        }
    }

    /// Records that `node_id`'s `tabindex` may have changed. The next
    /// [`Self::apply_pending`] re-reads it.
    pub fn mark_tab_dirty(&mut self, node_id: NodeId) {
        self.tab_dirty.insert(node_id);
    }

    /// Takes the nodes marked by [`Self::mark_tab_dirty`] and its subtree form.
    #[cfg(any(test, feature = "test-support"))]
    pub fn take_tab_dirty(&mut self) -> std::collections::HashSet<NodeId> {
        std::mem::take(&mut self.tab_dirty)
    }

    /// Like [`Self::mark_tab_dirty`] for `node_id` and its descendants that
    /// have, or could take, a handle.
    pub fn mark_subtree_dirty(&mut self, tree: &VirtualTree, node_id: NodeId) {
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if self.tracked.contains(&id)
                || node.is_focusable_by_default()
                || node.attributes().contains_key("tabindex")
            {
                self.tab_dirty.insert(id);
            }
            stack.extend(node.children());
        }
    }

    /// The `tabindex` of `node_id`. A node not connected to the tree's document,
    /// a text node, a node that has an ancestor (or itself) that is a template or has
    /// `display: none`, `inert` or a hiding `hidden`, and a `disabled` HTML button
    /// or input have none. An HTML button or input, or a link, whose `tabindex` is missing or
    /// unparsable has 0.
    pub(super) fn tab_index_of(
        tree: &VirtualTree,
        cache: &StyleCache,
        node_id: NodeId,
    ) -> Option<i32> {
        let node = tree.get(node_id)?;
        if !tree.is_connected(node_id) {
            return None;
        }
        let hidden = tree.ancestors(node_id).any(|id| {
            hidden_by_attribute(tree, cache, id) || tree.get(id).is_some_and(|ancestor| {
                ancestor.is_template()
                    || ancestor.attributes().contains_key("inert")
                    || matches!(
                        cache.computed_style(tree, id).get("display"),
                        Some(AttributeValue::String(value)) if value.eq_ignore_ascii_case("none")
                    )
            })
        });
        let by_default = node.is_focusable_by_default();
        if !node.is_element() || hidden || is_disabled_control(node) {
            return None;
        }
        tree.explicit_tab_index(node_id).or(by_default.then_some(0))
    }
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
    let mut ordered: Vec<((bool, i32), usize, NodeId)> = stops
        .iter()
        .enumerate()
        .filter(|(_, (_, index))| *index >= 0)
        // A single integer key gives 0 and the largest positive index the same key.
        .map(|(pos, &(id, index))| ((index == 0, index), pos, id))
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

/// The target of a Tab press from `node`, which sits at `at` among `stops` in tree order and is a
/// pressed node or a node outside the order. Forward, it is the first stop after `node`, or with
/// none after it the first stop of the order when `node` is outside the order. Backward, it is the
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{FakeSurface, ROOT, rooted_tree, tree_with};
    use inca_style::{remove_style, set_style};
    use rstest::rstest;
    use std::collections::HashSet;

    // Nodes 1 to 6 have tabindex 3, 1, 0, 0, -1 and the largest value.
    const STOPS: [(NodeId, i32); 6] = [(1, 3), (2, 1), (3, 0), (4, 0), (5, -1), (6, i32::MAX)];

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
                Some(1),
                Some(6),
                Some(3),
                Some(4),
                None,
                Some(2),
                Some(1)
            ]
        );
        assert_eq!(
            walk(&STOPS, None, true),
            [
                Some(4),
                Some(3),
                Some(6),
                Some(1),
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

    #[test]
    fn a_button_under_a_template_has_no_tab_index() {
        let (mut tree, button) = rooted_button(false);
        let template = tree.create_node("template").unwrap();
        tree.append_child(ROOT, template).unwrap();
        tree.append_child(template, button).unwrap();

        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), button),
            None
        );
    }

    #[test]
    fn a_text_node_and_a_display_none_node_have_no_tab_index() {
        let (tree, id) = tree_with("#text", Some(AttributeValue::from("0")));
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        set_style(&mut tree, id, "display", "none").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
        set_style(&mut tree, id, "display", "flex").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
    }

    #[test]
    fn a_comment_with_a_tabindex_has_no_tab_index() {
        let (tree, id) = tree_with("#comment", Some(AttributeValue::from("0")));
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
    }

    #[test]
    fn a_display_none_in_capitals_leaves_no_tab_index() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        set_style(&mut tree, id, "display", "NONE").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
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
                set_style(&mut tree, target, "display", "none").unwrap();
            } else {
                tree.set_attribute(target, hide, true).unwrap();
            }
            if FocusModel::tab_index_of(&tree, &StyleCache::new(), child).is_some() {
                mismatches.push(format!("{hide} on {name} keeps the tab index"));
            }
            if hide == "display" {
                remove_style(&mut tree, target, "display").unwrap();
            } else {
                tree.remove_attribute(target, hide).unwrap();
            }
            if FocusModel::tab_index_of(&tree, &StyleCache::new(), child) != Some(0) {
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
        tree.set_data(css, ".h { display: none }").unwrap();
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

        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), child),
            None
        );
    }

    #[test]
    fn a_node_with_a_rule_display_none_has_no_tab_index() {
        let (mut tree, _) = rooted_tree(0);
        hide_class_h(&mut tree);
        let node = tree.create_node("div").unwrap();
        tree.append_child(ROOT, node).unwrap();
        tree.set_attribute(node, "class", "h").unwrap();
        tree.set_attribute(node, "tabindex", "0").unwrap();

        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), node),
            None
        );
    }

    #[test]
    fn hidden_with_an_own_display_or_until_found_keeps_the_tab_index() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        tree.set_attribute(id, "hidden", "").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            None
        );
        set_style(&mut tree, id, "display", "flex").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
        remove_style(&mut tree, id, "display").unwrap();
        tree.set_attribute(id, "hidden", "until-found").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
    }

    const SVG: &str = "http://www.w3.org/2000/svg";

    #[rstest]
    #[case::button("button")]
    #[case::input("input")]
    fn an_svg_control_has_no_default_tab_index(#[case] tag: &str) {
        let (mut tree, _) = rooted_tree(0);
        let id = tree.create_element_ns(Some(SVG), None, tag).unwrap();
        tree.append_child(ROOT, id).unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            None,
            "{tag}"
        );
    }

    #[test]
    fn an_svg_template_ancestor_leaves_the_tab_index_of_its_subtree() {
        let (mut tree, button) = rooted_button(false);
        let template = tree.create_element_ns(Some(SVG), None, "template").unwrap();
        tree.append_child(ROOT, template).unwrap();
        tree.append_child(template, button).unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), button),
            Some(0)
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
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            want,
            "{value:?}"
        );
    }

    #[test]
    fn an_input_has_tab_index_0_without_a_tabindex() {
        let (tree, id) = tree_with("input", None);
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            Some(0)
        );
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
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            want,
            "{tag}"
        );
        tree.remove_attribute(id, "disabled").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), id),
            Some(2),
            "{tag}"
        );
    }

    #[test]
    fn tab_index_of_reads_the_default_focusability_of_the_kind() {
        let (mut tree, _) = rooted_tree(0);
        let with_href = tree.create_node("a").unwrap();
        let without = tree.create_node("a").unwrap();
        tree.append_child(ROOT, with_href).unwrap();
        tree.append_child(ROOT, without).unwrap();
        tree.set_attribute(with_href, "href", "u").unwrap();
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), with_href),
            Some(0)
        );
        assert_eq!(
            FocusModel::tab_index_of(&tree, &StyleCache::new(), without),
            None
        );
    }

    #[test]
    fn mark_subtree_dirty_queues_nodes_that_can_hold_focus() {
        let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
        let parent = tree.create_node("div").unwrap();
        let plain = tree.create_node("div").unwrap();
        let button = tree.create_node("button").unwrap();
        let input = tree.create_node("input").unwrap();
        let held = tree.create_node("div").unwrap();
        for child in [focusable, plain, button, input, held] {
            tree.append_child(parent, child).unwrap();
        }
        let mut registry = FocusModel::default();
        registry.tracked.insert(held);
        registry.mark_subtree_dirty(&tree, parent);
        assert_eq!(
            registry.tab_dirty,
            HashSet::from([focusable, button, input, held])
        );
    }

    /// Three tab stops under the root, all with a handle, and a model that applied them.
    fn three_stops() -> (VirtualTree, [NodeId; 3], FocusModel, FakeSurface) {
        let (tree, ids) = rooted_tree(3);
        let mut model = FocusModel::default();
        let mut surface = FakeSurface::default();
        for &id in &ids {
            model.mark_tab_dirty(id);
        }
        let _ = model.apply_pending(&tree, &StyleCache::new(), &mut surface);
        (tree, [ids[0], ids[1], ids[2]], model, surface)
    }

    fn tab(
        (tree, _, model, surface): &mut (VirtualTree, [NodeId; 3], FocusModel, FakeSurface),
        backward: bool,
    ) -> Option<NodeId> {
        model.tab_move(tree, &StyleCache::new(), ROOT, surface, backward)
    }

    #[rstest]
    #[case::forward(false, 0)]
    #[case::backward(true, 2)]
    fn tab_from_no_focus_takes_the_first_or_last_stop(#[case] backward: bool, #[case] stop: usize) {
        let mut stops = three_stops();
        let expected = stops.1[stop];

        assert_eq!(tab(&mut stops, backward), Some(expected));
        assert_eq!(stops.3.focus, Some(expected));
    }

    #[rstest]
    #[case::forward(false, 2)]
    #[case::backward(true, 0)]
    fn tab_from_a_focused_stop_takes_its_neighbour(#[case] backward: bool, #[case] stop: usize) {
        let mut stops = three_stops();
        stops.3.focus = Some(stops.1[1]);
        let expected = stops.1[stop];

        assert_eq!(tab(&mut stops, backward), Some(expected));
    }

    #[rstest]
    #[case::forward_takes_the_next_stop(false, 2)]
    #[case::backward_takes_the_pressed_stop(true, 1)]
    fn tab_after_a_press_on_a_stop_continues_from_it(#[case] backward: bool, #[case] stop: usize) {
        let mut stops = three_stops();
        let pressed = stops.1[1];
        stops.2.press_start(pressed, ROOT, None);
        let expected = stops.1[stop];

        assert_eq!(tab(&mut stops, backward), Some(expected));
    }

    #[test]
    fn tab_past_the_last_stop_blurs_and_forgets_where_tab_started() {
        let mut stops = three_stops();
        stops.3.focus = Some(stops.1[2]);
        stops.2.press_start(stops.1[2], ROOT, None);

        assert_eq!(tab(&mut stops, false), None);
        assert_eq!(stops.3.focus, None);
        assert_eq!(stops.2.start_node(), None);
    }

    #[test]
    fn tab_skips_a_stop_without_a_handle() {
        let mut stops = three_stops();
        stops.3.handles.remove(&stops.1[0]);
        stops.2.tracked.remove(&stops.1[0]);
        let expected = stops.1[1];

        assert_eq!(tab(&mut stops, false), Some(expected));
    }

    #[test]
    fn a_tab_move_drops_the_scroll_requests_of_earlier_focus_calls() {
        let mut stops = three_stops();
        stops.2.request_focus(stops.1[0], true);
        let _ = stops
            .2
            .apply_pending(&stops.0, &StyleCache::new(), &mut stops.3);

        let _ = tab(&mut stops, false);

        assert_eq!(stops.2.take_reveals(), (vec![], vec![]));
    }
}
