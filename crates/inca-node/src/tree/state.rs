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
    #[must_use]
    pub fn is_hovered(&self, id: NodeId) -> bool {
        self.interaction.hover.contains(&id) && self.is_connected(id)
    }

    /// Whether `id` is on the pressed path and is connected.
    #[must_use]
    pub fn is_active(&self, id: NodeId) -> bool {
        self.interaction.active.contains(&id) && self.is_connected(id)
    }

    /// Sets the focused node, or none. Returns whether the value changed.
    pub fn set_focus(&mut self, node: Option<NodeId>) -> bool {
        let changed = self.interaction.focus != node;
        self.interaction.focus = node;
        changed
    }

    /// Whether `id` is the focused node and is connected.
    #[must_use]
    pub fn is_focused(&self, id: NodeId) -> bool {
        self.interaction.focus == Some(id) && self.is_connected(id)
    }

    /// Whether `id` is the focused node or an ancestor of it, and the focused node is connected.
    #[must_use]
    pub fn is_focus_within(&self, id: NodeId) -> bool {
        self.interaction
            .focus
            .is_some_and(|focus| self.is_connected(focus) && self.ancestors(focus).any(|a| a == id))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::Fixture;

    /// `root > card > input`.
    struct Scene {
        f: Fixture,
        card: NodeId,
        input: NodeId,
    }

    fn scene() -> Scene {
        let mut f = Fixture::new();
        let card = f.add(f.root, "div", "card");
        let input = f.add(card, "input", "input");
        Scene { f, card, input }
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
}
