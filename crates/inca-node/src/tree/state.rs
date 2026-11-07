// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The interaction state that the rules of a `<style>` element read.

use std::collections::HashSet;

use super::{NodeId, VirtualTree};

/// The state a script or the user puts on nodes. The tree reads it and never prunes it.
#[derive(Debug, Default)]
pub(super) struct InteractionState {
    focus: Option<NodeId>,
    /// The ids on the path from the deepest hovered container to the root.
    hover: HashSet<NodeId>,
    /// The ids on the pressed path.
    active: HashSet<NodeId>,
}

/// Replaces `set` with `path`. Returns whether the set changed.
fn replace(set: &mut HashSet<NodeId>, path: &[NodeId]) -> bool {
    let next: HashSet<NodeId> = path.iter().copied().collect();
    let changed = *set != next;
    *set = next;
    changed
}

impl VirtualTree {
    /// Sets the hover path: the ids of the deepest hovered container and its ancestors. An empty
    /// path clears. Returns whether the set changed.
    pub fn set_hover_path(&mut self, path: &[NodeId]) -> bool {
        replace(&mut self.interaction.hover, path)
    }

    /// Sets the pressed path, like [`Self::set_hover_path`].
    pub fn set_active_path(&mut self, path: &[NodeId]) -> bool {
        replace(&mut self.interaction.active, path)
    }

    /// Whether `id` is on the hover path and is connected.
    pub(crate) fn is_hovered(&self, id: NodeId) -> bool {
        self.interaction.hover.contains(&id) && self.is_connected(id)
    }

    /// Whether `id` is on the pressed path and is connected.
    pub(crate) fn is_active(&self, id: NodeId) -> bool {
        self.interaction.active.contains(&id) && self.is_connected(id)
    }

    /// Sets the focused node, or none. Returns whether the value changed.
    pub fn set_focus(&mut self, node: Option<NodeId>) -> bool {
        let changed = self.interaction.focus != node;
        self.interaction.focus = node;
        changed
    }

    /// Whether `id` is the focused node and is connected.
    pub(crate) fn is_focused(&self, id: NodeId) -> bool {
        self.interaction.focus == Some(id) && self.is_connected(id)
    }

    /// Whether `id` is the focused node or an ancestor of it, and the focused node is connected.
    pub(crate) fn is_focus_within(&self, id: NodeId) -> bool {
        self.interaction
            .focus
            .is_some_and(|focus| self.is_connected(focus) && self.ancestors(focus).any(|a| a == id))
    }

    /// Whether a connected `<style>` rule names `:focus` or `:focus-within`.
    #[must_use]
    pub fn uses_focus_state(&self) -> bool {
        self.sheet_flags().focus
    }

    /// Whether a connected `<style>` rule names `:hover` or `:active`.
    #[must_use]
    pub fn uses_pointer_state(&self) -> bool {
        self.sheet_flags().pointer
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{Fixture, text};

    /// `root > (card > input, other)`.
    struct Scene {
        f: Fixture,
        card: NodeId,
        input: NodeId,
        other: NodeId,
    }

    fn scene() -> Scene {
        let mut f = Fixture::new();
        let card = f.add(f.root, "div", "card");
        f.attr(card, "class", "card");
        let input = f.add(card, "input", "input");
        let other = f.add(f.root, "div", "other");
        Scene {
            f,
            card,
            input,
            other,
        }
    }

    /// Puts the input and its ancestors on the path of `class` (`:hover` or `:active`).
    fn put_on_path(s: &mut Scene, class: &str) {
        let path: Vec<_> = s.f.tree.ancestors(s.input).collect();
        match class {
            ":hover" => s.f.tree.set_hover_path(&path),
            _ => s.f.tree.set_active_path(&path),
        };
    }

    fn width(f: &Fixture, node: NodeId) -> Option<crate::AttributeValue> {
        f.tree.computed_style(node).get("width").cloned()
    }

    #[test]
    fn focus_matches_the_focused_node() {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));

        assert!(s.f.matches(s.input, ":focus"));
    }

    #[test]
    fn focus_does_not_match_the_ancestor_of_the_focused_node() {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));

        assert!(!s.f.matches(s.card, ":focus"));
    }

    #[rstest]
    #[case::the_node(|s: &Scene| s.input, true)]
    #[case::its_parent(|s: &Scene| s.card, true)]
    #[case::the_root(|s: &Scene| s.f.root, true)]
    #[case::a_sibling_branch(|s: &Scene| s.other, false)]
    fn focus_within_matches_the_focused_node_and_its_ancestors(
        #[case] node: fn(&Scene) -> NodeId,
        #[case] expected: bool,
    ) {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));

        assert_eq!(s.f.matches(node(&s), ":focus-within"), expected);
    }

    #[rstest]
    #[case::a_first_node(None, Some(1), true)]
    #[case::the_same_node(Some(1), Some(1), false)]
    #[case::another_node(Some(1), Some(2), true)]
    #[case::none_again(None, None, false)]
    fn set_focus_reports_whether_the_value_changed(
        #[case] before: Option<NodeId>,
        #[case] after: Option<NodeId>,
        #[case] expected: bool,
    ) {
        let mut tree = VirtualTree::new();
        tree.set_focus(before);

        assert_eq!(tree.set_focus(after), expected);
    }

    #[test]
    fn focus_visible_never_matches_the_focused_node() {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));

        assert!(!s.f.matches(s.input, ":focus-visible"));
    }

    #[rstest]
    #[case::focus_on_the_node(|s: &Scene| s.input, ":focus")]
    #[case::focus_within_on_the_node(|s: &Scene| s.input, ":focus-within")]
    #[case::focus_within_on_the_ancestor(|s: &Scene| s.card, ":focus-within")]
    fn a_focused_node_that_is_detached_stops_matching(
        #[case] node: fn(&Scene) -> NodeId,
        #[case] selector: &str,
    ) {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));
        s.f.tree.remove_child(s.f.root, s.card).unwrap();

        assert!(!s.f.matches(node(&s), selector));
    }

    #[rstest]
    #[case::the_same_parent(|s: &Scene| s.card)]
    #[case::another_parent(|s: &Scene| s.other)]
    fn a_detached_focused_node_matches_again_once_attached(#[case] parent: fn(&Scene) -> NodeId) {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));
        s.f.tree.remove_child(s.card, s.input).unwrap();
        let detached = s.f.matches(s.input, ":focus");
        s.f.tree.insert_before(parent(&s), s.input, None).unwrap();

        assert!(!detached);
        assert!(s.f.matches(s.input, ":focus"));
    }

    #[test]
    fn a_focused_node_moved_by_append_child_keeps_matching() {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));
        s.f.tree.append_child(s.other, s.input).unwrap();

        assert!(s.f.matches(s.input, ":focus"));
        assert!(s.f.matches(s.other, ":focus-within"));
    }

    #[test]
    fn a_focused_node_of_a_tree_without_a_root_matches() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("input").unwrap();
        tree.set_focus(Some(node));

        assert!(tree.is_focused(node));
        assert!(tree.is_focus_within(node));
    }

    #[test]
    fn a_destroyed_focused_node_is_not_focused_under_a_root() {
        let mut s = scene();
        s.f.tree.set_focus(Some(s.input));
        s.f.tree.destroy_node(s.input);

        assert!(!s.f.tree.is_focused(s.input));
        assert!(!s.f.tree.is_focus_within(s.card));
    }

    #[rstest]
    #[case::focus(":focus")]
    #[case::focus_within(":focus-within")]
    #[case::negated(":not(:focus)")]
    #[case::inside_is(":is(.a:focus)")]
    #[case::nested_in_has("div:has(:focus)")]
    fn uses_focus_state_is_true_when_a_rule_names_the_state(#[case] selector: &str) {
        let mut s = scene();
        s.f.sheet(&format!("{selector} {{ width: 1px }}"));

        assert!(s.f.tree.uses_focus_state());
    }

    #[rstest]
    #[case::no_rule("")]
    #[case::no_state(".a { width: 1px }")]
    #[case::an_invalid_rule(":focus:bogus { width: 1px }")]
    #[case::host_context(":host-context(a:focus) { width: 1px }")]
    #[case::host_context_in_is(":is(:host-context(a:focus b), p) { width: 1px }")]
    fn uses_focus_state_is_false_when_no_rule_names_the_state(#[case] css: &str) {
        let mut s = scene();
        s.f.sheet(css);

        assert!(!s.f.tree.uses_focus_state());
    }

    #[test]
    fn uses_focus_state_is_false_once_the_style_element_is_removed() {
        let mut s = scene();
        let (style, _) = s.f.sheet(":focus { width: 1px }");
        let named = s.f.tree.uses_focus_state();
        s.f.tree.remove_child(s.f.root, style).unwrap();

        assert!(named);
        assert!(!s.f.tree.uses_focus_state());
    }

    #[test]
    fn uses_focus_state_follows_an_edit_of_the_sheet_text() {
        let mut s = scene();
        let (_, text_node) = s.f.sheet(".a { width: 1px }");
        let before = s.f.tree.uses_focus_state();
        s.f.attr(text_node, "value", ":focus { width: 1px }");

        assert!(!before);
        assert!(s.f.tree.uses_focus_state());
    }

    #[test]
    fn a_rule_reads_the_focus_within_of_an_ancestor_live() {
        let mut s = scene();
        s.f.sheet(".card:focus-within .title { width: 5px }");
        let title = s.f.add(s.card, "span", "title");
        s.f.attr(title, "class", "title");
        let before = width(&s.f, title);
        s.f.tree.set_focus(Some(s.input));
        let during = width(&s.f, title);
        s.f.tree.set_focus(None);

        assert_eq!(before, None);
        assert_eq!(during, Some(text("5px")));
        assert_eq!(width(&s.f, title), None);
    }

    #[test]
    fn a_focus_rule_applies_only_while_the_node_is_focused() {
        let mut s = scene();
        s.f.sheet("input:focus { width: 1px }");
        let before = width(&s.f, s.input);
        s.f.tree.set_focus(Some(s.input));
        let during = width(&s.f, s.input);
        s.f.tree.set_focus(None);

        assert_eq!(before, None);
        assert_eq!(during, Some(text("1px")));
        assert_eq!(width(&s.f, s.input), None);
    }

    #[rstest]
    #[case::hover(":hover")]
    #[case::active(":active")]
    fn a_pointer_state_matches_the_node_and_its_ancestors(#[case] class: &str) {
        let mut s = scene();
        put_on_path(&mut s, class);

        assert!(s.f.matches(s.input, class));
        assert!(s.f.matches(s.card, class));
        assert!(s.f.matches(s.f.root, class));
    }

    #[rstest]
    #[case::hover(":hover")]
    #[case::active(":active")]
    fn a_pointer_state_does_not_match_a_sibling_branch(#[case] class: &str) {
        let mut s = scene();
        put_on_path(&mut s, class);

        assert!(!s.f.matches(s.other, class));
    }

    #[rstest]
    #[case::hover_set_active_read(":hover", ":active")]
    #[case::active_set_hover_read(":active", ":hover")]
    fn a_pointer_state_does_not_answer_for_the_other_one(#[case] set: &str, #[case] read: &str) {
        let mut s = scene();
        put_on_path(&mut s, set);

        assert!(!s.f.matches(s.input, read));
    }

    #[test]
    fn a_negated_hover_matches_an_unhovered_node() {
        let s = scene();

        assert!(s.f.matches(s.input, ":not(:hover)"));
    }

    #[test]
    fn a_negated_hover_does_not_match_a_hovered_node() {
        let mut s = scene();
        put_on_path(&mut s, ":hover");

        assert!(!s.f.matches(s.input, ":not(:hover)"));
    }

    #[test]
    fn a_hover_name_is_read_case_insensitively() {
        let mut s = scene();
        put_on_path(&mut s, ":hover");

        assert!(s.f.matches(s.input, ":HOVER"));
    }

    #[rstest]
    #[case::a_first_path(&[], &[1, 2], true)]
    #[case::the_same_path(&[1, 2], &[1, 2], false)]
    #[case::the_same_ids_reordered(&[1, 2], &[2, 1], false)]
    #[case::another_path(&[1, 2], &[1, 3], true)]
    #[case::cleared(&[1, 2], &[], true)]
    #[case::empty_again(&[], &[], false)]
    fn the_path_setters_report_whether_the_set_changed(
        #[case] before: &[NodeId],
        #[case] after: &[NodeId],
        #[case] expected: bool,
        #[values(VirtualTree::set_hover_path, VirtualTree::set_active_path)] set: fn(
            &mut VirtualTree,
            &[NodeId],
        )
            -> bool,
    ) {
        let mut tree = VirtualTree::new();
        set(&mut tree, before);

        assert_eq!(set(&mut tree, after), expected);
    }

    #[rstest]
    #[case::hover(":hover")]
    #[case::active(":active")]
    fn a_detached_pointer_node_stops_matching_while_its_ancestors_keep_matching(
        #[case] class: &str,
    ) {
        let mut s = scene();
        put_on_path(&mut s, class);
        s.f.tree.remove_child(s.card, s.input).unwrap();

        assert!(!s.f.matches(s.input, class));
        assert!(s.f.matches(s.card, class));
    }

    #[rstest]
    #[case::hover(":hover")]
    #[case::active(":active")]
    fn a_detached_pointer_node_matches_again_once_attached(#[case] class: &str) {
        let mut s = scene();
        put_on_path(&mut s, class);
        s.f.tree.remove_child(s.card, s.input).unwrap();
        let detached = s.f.matches(s.input, class);
        s.f.tree.insert_before(s.card, s.input, None).unwrap();

        assert!(!detached);
        assert!(s.f.matches(s.input, class));
    }

    #[rstest]
    #[case::hover(":hover")]
    #[case::active(":active")]
    #[case::negated(":not(:hover)")]
    #[case::inside_is(":is(.a:hover)")]
    #[case::nested_in_has("div:has(:active)")]
    fn uses_pointer_state_is_true_when_a_rule_names_the_state(#[case] selector: &str) {
        let mut s = scene();
        s.f.sheet(&format!("{selector} {{ width: 1px }}"));

        assert!(s.f.tree.uses_pointer_state());
    }

    #[rstest]
    #[case::no_rule("")]
    #[case::a_focus_rule(":focus { width: 1px }")]
    #[case::host_context(":host-context(a:hover) { width: 1px }")]
    fn uses_pointer_state_is_false_when_no_rule_names_the_state(#[case] css: &str) {
        let mut s = scene();
        s.f.sheet(css);

        assert!(!s.f.tree.uses_pointer_state());
    }

    #[rstest]
    #[case::two_rules(":focus { width: 1px } :hover { width: 2px }")]
    #[case::one_selector(":focus:hover { width: 1px }")]
    fn sheet_flags_keep_both_kinds(#[case] css: &str) {
        let mut s = scene();
        s.f.sheet(css);

        assert!(s.f.tree.uses_focus_state());
        assert!(s.f.tree.uses_pointer_state());
    }

    #[test]
    fn a_hover_rule_applies_only_while_the_node_is_hovered() {
        let mut s = scene();
        s.f.sheet("input:hover { width: 1px }");
        let before = width(&s.f, s.input);
        put_on_path(&mut s, ":hover");
        let during = width(&s.f, s.input);
        s.f.tree.set_hover_path(&[]);

        assert_eq!(before, None);
        assert_eq!(during, Some(text("1px")));
        assert_eq!(width(&s.f, s.input), None);
    }
}
