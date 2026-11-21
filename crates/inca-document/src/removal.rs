// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The focus steps that follow a tree call that moved or removed nodes.

use inca_node::{Removal, VirtualTree};

use crate::FocusModel;

/// Applies the removals a tree call made. A node that had a parent drops the focus it held and
/// moves the Tab start. Every node queues a focus re-check.
pub fn apply_removals(focus: &mut FocusModel, tree: &VirtualTree, removals: &[Removal]) {
    for removal in removals {
        if removal.attached {
            focus.drop_focus_in(tree, removal.id);
            focus.start_leaves(tree, removal.id, removal.before);
        }
        focus.mark_subtree_dirty(tree, removal.id);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::rooted_tree;

    #[test]
    fn an_attached_node_that_leaves_moves_the_tab_start_to_its_before() {
        let (mut tree, ids) = rooted_tree(2);
        let mut focus = FocusModel::default();
        focus.press_start(ids[1], 0, None);
        let removals = tree.removals(&[ids[1]]);
        tree.remove_child(0, ids[1]).unwrap();

        apply_removals(&mut focus, &tree, &removals);

        assert_eq!(focus.start_node(), Some(ids[0]));
    }

    #[test]
    fn an_attached_node_that_leaves_drops_the_focus_it_held() {
        let (mut tree, ids) = rooted_tree(2);
        let mut focus = FocusModel::default();
        focus.observe_node(Some(ids[1]));
        let removals = tree.removals(&[ids[1]]);
        tree.remove_child(0, ids[1]).unwrap();

        apply_removals(&mut focus, &tree, &removals);

        assert_eq!(focus.held_node(Some(ids[1])), None);
    }

    #[test]
    fn a_first_node_that_leaves_clears_the_tab_start() {
        let (mut tree, ids) = rooted_tree(2);
        let mut focus = FocusModel::default();
        focus.press_start(ids[0], 0, None);
        let removals = tree.removals(&[ids[0]]);
        tree.remove_child(0, ids[0]).unwrap();

        apply_removals(&mut focus, &tree, &removals);

        assert_eq!(focus.start_node(), None);
    }

    #[test]
    fn a_detached_node_leaves_the_tab_start_alone() {
        let (mut tree, _) = rooted_tree(1);
        let loose = tree.create_node("div").unwrap();
        let inner = tree.create_node("div").unwrap();
        tree.append_child(loose, inner).unwrap();
        let mut focus = FocusModel::default();
        focus.press_start(inner, 0, None);
        let removals = tree.removals(&[loose]);

        apply_removals(&mut focus, &tree, &removals);

        assert_eq!(focus.start_node(), Some(inner));
    }

    #[test]
    fn a_removed_node_queues_a_focus_recheck() {
        let (tree, ids) = rooted_tree(1);
        let mut focus = FocusModel::default();
        let removals = tree.removals(&[ids[0]]);

        apply_removals(&mut focus, &tree, &removals);

        assert!(focus.take_tab_dirty().contains(&ids[0]));
    }
}
