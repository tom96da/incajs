// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The steps that follow a tree call that moved or removed nodes, and the release of freed nodes.

use inca_node::{NodeId, Removal, VirtualTree};

use crate::{DocumentState, FocusModel, View};

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

/// Runs the steps that follow a tree change that took `removals` out of the tree. The document
/// drops the focus, moves the Tab start and queues the focus re-check. Every node resets the scroll
/// offsets of its subtree in `view`.
pub fn settle(state: &mut DocumentState, view: &dyn View, removals: &[Removal]) {
    apply_removals(&mut state.focus, &state.tree, removals);
    for removal in removals {
        view.reset_scroll(&state.tree, removal.id);
    }
}

/// Drops the per-node state of the freed nodes `ids` in `view` and in `state`. Returns the callback
/// ids their listeners held.
pub fn free_ids(state: &mut DocumentState, view: &mut dyn View, ids: &[NodeId]) -> Vec<u32> {
    view.forget(ids);
    state.forget(ids)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::EventTargetId;
    use crate::tests::{RecordingView, rooted_tree};

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

    #[test]
    fn settle_applies_the_removals_then_resets_the_scroll_of_each_node() {
        let (tree, ids) = rooted_tree(2);
        let removals = tree.removals(&[ids[0], ids[1]]);
        let mut state = DocumentState {
            tree,
            ..DocumentState::default()
        };
        let view = RecordingView::default();

        settle(&mut state, &view, &removals);

        assert_eq!(*view.resets.borrow(), ids);
        assert!(state.focus.take_tab_dirty().contains(&ids[0]));
    }

    #[test]
    fn free_ids_forgets_the_nodes_in_the_view_and_in_the_state() {
        let mut state = DocumentState::default();
        let mut view = RecordingView::default();

        let callbacks = free_ids(&mut state, &mut view, &[3, 4]);

        assert_eq!(view.forgotten, [3, 4]);
        assert!(callbacks.is_empty());
    }

    #[test]
    fn free_ids_releases_the_listeners_of_the_freed_nodes_and_returns_their_callbacks() {
        let mut state = DocumentState::default();
        let mut view = RecordingView::default();
        let target = |id| EventTargetId::Node(id);
        state.listeners.insert(target(3), "click", 1, false, false);
        state.listeners.insert(target(4), "click", 2, false, false);
        state.listeners.insert(target(5), "click", 3, false, false);

        let mut callbacks = free_ids(&mut state, &mut view, &[3, 4]);
        callbacks.sort_unstable();

        assert_eq!(callbacks, [1, 2]);
        assert!(state.listeners.listeners_for(target(3), "click").is_empty());
        assert!(state.listeners.listeners_for(target(4), "click").is_empty());
        assert_eq!(state.listeners.listeners_for(target(5), "click").len(), 1);
    }

    #[test]
    fn free_ids_drops_the_queued_focus_check_of_a_freed_node() {
        let mut state = DocumentState::default();
        let mut view = RecordingView::default();
        state.focus.mark_tab_dirty(3);

        free_ids(&mut state, &mut view, &[3]);

        assert!(state.focus.take_tab_dirty().is_empty());
    }
}
