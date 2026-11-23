// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The interaction-state pseudo-classes: `:focus`, `:focus-within`, `:hover` and `:active` read
//! the state the tree keeps. The tests of these sit in the same file.

use super::{Node, PseudoClass};

impl Node<'_> {
    /// Whether the element is in the interaction state `class`. Any other pseudo-class is false.
    pub(super) fn is_in_interaction_state(&self, class: &PseudoClass) -> bool {
        let id = self.node.id();
        match class {
            PseudoClass::Hover => self.tree.is_hovered(id),
            PseudoClass::Active => self.tree.is_active(id),
            PseudoClass::Focus => self.tree.is_focused(id),
            PseudoClass::FocusWithin => self.tree.is_focus_within(id),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use inca_node::tree::{AttributeValue, NodeId};
    use rstest::rstest;

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

    fn width(f: &Fixture, node: NodeId) -> Option<AttributeValue> {
        f.cache.computed_style(&f.tree, node).get("width").cloned()
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
