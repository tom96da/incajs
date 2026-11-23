// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tree changes that create, copy, move, remove and free nodes and that write their data and
//! the hover and pressed paths, each with the steps that follow it, and the release of freed
//! nodes.

use inca_node::{InsertArg, Inserted, NodeId, Placement, Removal, TreeError};

use crate::{DocumentState, Released, View};

impl DocumentState {
    /// Creates a node named `local_name`. See
    /// [`VirtualTree::create_node`](inca_node::VirtualTree::create_node).
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::create_node`](inca_node::VirtualTree::create_node).
    pub fn create_node(&mut self, local_name: impl Into<String>) -> Result<NodeId, TreeError> {
        self.tree.create_node(local_name)
    }

    /// Creates the element `local_name` in `namespace` with `prefix`. See
    /// [`VirtualTree::create_element_ns`](inca_node::VirtualTree::create_element_ns).
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::create_element_ns`](inca_node::VirtualTree::create_element_ns).
    pub fn create_element_ns(
        &mut self,
        namespace: Option<&str>,
        prefix: Option<&str>,
        local_name: impl Into<String>,
    ) -> Result<NodeId, TreeError> {
        self.tree.create_element_ns(namespace, prefix, local_name)
    }

    /// Copies `node_id`, with its descendants when `deep`, and returns the copy. An HTML `input`
    /// copy takes the typed text of its original.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::clone_subtree`](inca_node::VirtualTree::clone_subtree).
    pub fn clone_node(&mut self, node_id: NodeId, deep: bool) -> Result<NodeId, TreeError> {
        let made = self.tree.clone_subtree(node_id, deep)?;
        for &(original, copy) in &made {
            let is_input = self
                .tree
                .get(copy)
                .is_some_and(|node| node.is_html("input"));
            let typed = self.input(original).map(|input| input.value().to_owned());
            if let (true, Some(typed)) = (is_input, typed) {
                self.set_input_value(copy, &typed);
            }
        }
        made.first()
            .map(|&(_, copy)| copy)
            .ok_or(TreeError::NodeNotFound(node_id))
    }

    /// Keeps `node_id` from the sweep while it has no parent. See
    /// [`VirtualTree::hold`](inca_node::VirtualTree::hold).
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::hold`](inca_node::VirtualTree::hold).
    pub fn hold(&mut self, node_id: NodeId) -> Result<(), TreeError> {
        self.tree.hold(node_id)
    }

    /// Ends the hold of `node_id`. See [`VirtualTree::release`](inca_node::VirtualTree::release).
    pub fn release(&mut self, node_id: NodeId) {
        self.tree.release(node_id);
    }

    /// Replaces the data of the text node or comment `node_id`. See
    /// [`VirtualTree::set_data`](inca_node::VirtualTree::set_data).
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::set_data`](inca_node::VirtualTree::set_data).
    pub fn set_data(&mut self, node_id: NodeId, data: impl Into<String>) -> Result<(), TreeError> {
        self.tree.set_data(node_id, data)
    }

    /// Replaces `count` units of the data of `node_id` from `offset` with `data`. See
    /// [`VirtualTree::replace_data`](inca_node::VirtualTree::replace_data).
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::replace_data`](inca_node::VirtualTree::replace_data).
    pub fn replace_data(
        &mut self,
        node_id: NodeId,
        offset: u32,
        count: u32,
        data: &str,
    ) -> Result<(), TreeError> {
        self.tree.replace_data(node_id, offset, count, data)
    }

    /// Splits the text node `node_id` at `offset` and returns the new node with the rest. See
    /// [`VirtualTree::split_text`](inca_node::VirtualTree::split_text).
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::split_text`](inca_node::VirtualTree::split_text).
    pub fn split_text(&mut self, node_id: NodeId, offset: u32) -> Result<NodeId, TreeError> {
        self.tree.split_text(node_id, offset)
    }

    /// Stores the hover path: the deepest hovered container and its ancestors. An empty path
    /// clears it. Returns whether the set changed.
    pub fn set_hover_path(&mut self, path: &[NodeId]) -> bool {
        self.tree.set_hover_path(path)
    }

    /// Stores the pressed path, like [`Self::set_hover_path`].
    pub fn set_active_path(&mut self, path: &[NodeId]) -> bool {
        self.tree.set_active_path(path)
    }

    /// Appends `node_id` to the children of `parent_id` and settles the nodes that move. Returns
    /// the position of the first inserted node.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::insert_before`](inca_node::VirtualTree::insert_before).
    pub fn append_child(
        &mut self,
        view: &dyn View,
        parent_id: NodeId,
        node_id: NodeId,
    ) -> Result<usize, TreeError> {
        self.insert_before(view, parent_id, node_id, None)
    }

    /// Inserts `node_id` under `parent_id` before `reference_id`, or last without one, and settles
    /// the nodes that move. Returns the position of the first inserted node.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::insert_before`](inca_node::VirtualTree::insert_before).
    pub fn insert_before(
        &mut self,
        view: &dyn View,
        parent_id: NodeId,
        node_id: NodeId,
        reference_id: Option<NodeId>,
    ) -> Result<usize, TreeError> {
        let (position, left) = self.tree.insert_before(parent_id, node_id, reference_id)?;
        settle(self, view, &left);
        Ok(position)
    }

    /// Takes `child_id` out of `parent_id` and settles it.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::remove_child`](inca_node::VirtualTree::remove_child).
    pub fn remove_child(
        &mut self,
        view: &dyn View,
        parent_id: NodeId,
        child_id: NodeId,
    ) -> Result<(), TreeError> {
        let left = self.tree.remove_child(parent_id, child_id)?;
        settle(self, view, &left);
        Ok(())
    }

    /// Puts `node_id` in the place of `child_id` under `parent_id` and settles the nodes that
    /// leave their place.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::replace_child`](inca_node::VirtualTree::replace_child).
    pub fn replace_child(
        &mut self,
        view: &dyn View,
        parent_id: NodeId,
        node_id: NodeId,
        child_id: NodeId,
    ) -> Result<(), TreeError> {
        let (_, left) = self.tree.replace_child(parent_id, node_id, child_id)?;
        settle(self, view, &left);
        Ok(())
    }

    /// Checks and applies the insertion of `nodes` at `placement` of `node_id`, and settles the
    /// nodes that leave their place. Returns `None` without a change for `Before`, `After` and
    /// `ReplaceWith` on a node without a parent.
    ///
    /// # Errors
    ///
    /// The error of the plan or of the insertion. A failed plan changes nothing.
    pub fn insert_nodes(
        &mut self,
        view: &dyn View,
        node_id: NodeId,
        placement: Placement,
        nodes: &[InsertArg],
    ) -> Result<Option<Inserted>, TreeError> {
        let Some(insertion) = self.tree.plan_insertion(node_id, placement, nodes)? else {
            return Ok(None);
        };
        let inserted = self.tree.apply_insertion(insertion)?;
        settle(self, view, &inserted.removals);
        Ok(Some(inserted))
    }

    /// Frees `node_id` with its subtree, moves the Tab start off it and drops the state of the
    /// freed nodes in `view` and in the document, their listeners included.
    pub fn destroy_node(&mut self, view: &mut dyn View, node_id: NodeId) -> Released {
        let before = self.tree.preceding(node_id);
        self.move_tab_start(node_id, before);
        let freed = self.tree.destroy_node(node_id);
        let callbacks = free_ids(self, view, &freed);
        self.released(callbacks, [])
    }

    /// Frees every orphan with its subtree and drops the state of the freed nodes in `view` and in
    /// the document, their listeners included. Returns the freed ids with what their listeners
    /// leave to the JS side, or `None` when nothing was freed.
    pub fn sweep(&mut self, view: &mut dyn View) -> Option<(Vec<NodeId>, Released)> {
        let nodes = self.tree.sweep();
        if nodes.is_empty() {
            return None;
        }
        let callbacks = free_ids(self, view, &nodes);
        Some((nodes, self.released(callbacks, [])))
    }

    /// Drops the document state of freed nodes and returns the callback ids their listeners held.
    pub(crate) fn forget(&mut self, ids: &[NodeId]) -> Vec<u32> {
        for &id in ids {
            self.warnings.forget(id);
            self.inputs.remove(&id);
            self.focus.forget(id);
            self.reveals.forget(id);
        }
        self.listeners.release_nodes(ids)
    }
}

/// Runs the steps that follow a tree change that took `removals` out of the tree. A node that had
/// a parent drops the focus it held with the reveals a focus asked for, and moves the Tab start.
/// Every node queues a focus re-check and resets the scroll offsets of its subtree in `view`.
pub(crate) fn settle(state: &mut DocumentState, view: &dyn View, removals: &[Removal]) {
    for removal in removals {
        if removal.attached {
            state.drop_focus_in(removal.id);
            state.move_tab_start(removal.id, removal.before);
        }
        state.mark_focus_dirty(removal.id);
        view.reset_scroll(&state.tree, removal.id);
    }
}

/// Drops the per-node state of the freed nodes `ids` in `view` and in `state`. Returns the callback
/// ids their listeners held.
pub(crate) fn free_ids(state: &mut DocumentState, view: &mut dyn View, ids: &[NodeId]) -> Vec<u32> {
    view.forget(ids);
    state.forget(ids)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{RecordingView, rooted_tree};
    use crate::{Align, EventTargetId, Reveal, RevealOrigin};
    use inca_node::VirtualTree;

    /// Settles `removals` on a document over `tree` whose focus `prepare` sets up first.
    fn settled(
        tree: VirtualTree,
        removals: &[Removal],
        prepare: impl FnOnce(&mut DocumentState),
    ) -> DocumentState {
        let mut state = DocumentState::from_tree(tree);
        prepare(&mut state);
        settle(&mut state, &RecordingView::default(), removals);
        state
    }

    #[test]
    fn an_attached_node_that_leaves_moves_the_tab_start_to_its_before() {
        let (mut tree, ids) = rooted_tree(2);
        let removals = tree.removals(&[ids[1]]);
        tree.remove_child(0, ids[1]).unwrap();

        let state = settled(tree, &removals, |state| state.press_start(ids[1], 0, None));

        assert_eq!(state.focus().start_node(), Some(ids[0]));
    }

    #[test]
    fn an_attached_node_that_leaves_drops_the_focus_it_held() {
        let (mut tree, ids) = rooted_tree(2);
        let removals = tree.removals(&[ids[1]]);
        tree.remove_child(0, ids[1]).unwrap();

        let state = settled(tree, &removals, |state| state.observe_focus(Some(ids[1])));

        assert_eq!(state.focus().held_node(Some(ids[1])), None);
    }

    #[test]
    fn a_first_node_that_leaves_clears_the_tab_start() {
        let (mut tree, ids) = rooted_tree(2);
        let removals = tree.removals(&[ids[0]]);
        tree.remove_child(0, ids[0]).unwrap();

        let state = settled(tree, &removals, |state| state.press_start(ids[0], 0, None));

        assert_eq!(state.focus().start_node(), None);
    }

    #[test]
    fn a_detached_node_leaves_the_tab_start_alone() {
        let (mut tree, _) = rooted_tree(1);
        let loose = tree.create_node("div").unwrap();
        let inner = tree.create_node("div").unwrap();
        tree.append_child(loose, inner).unwrap();
        let removals = tree.removals(&[loose]);

        let state = settled(tree, &removals, |state| state.press_start(inner, 0, None));

        assert_eq!(state.focus().start_node(), Some(inner));
    }

    #[test]
    fn a_removed_node_queues_a_focus_recheck() {
        let (tree, ids) = rooted_tree(1);
        let removals = tree.removals(&[ids[0]]);

        let mut state = settled(tree, &removals, |_| {});

        assert!(state.take_tab_dirty().contains(&ids[0]));
    }

    #[test]
    fn an_attached_node_that_leaves_drops_the_reveals_its_focus_asked_for() {
        let (mut tree, ids) = rooted_tree(1);
        let removals = tree.removals(&[ids[0]]);
        tree.remove_child(0, ids[0]).unwrap();

        let mut state = settled(tree, &removals, |state| {
            state.reveals.queue(Reveal::focus(ids[0]));
            state.queue_reveal(ids[0], Align::Start, Align::Start);
        });

        let origins: Vec<_> = state
            .take_reveals()
            .fresh
            .iter()
            .map(|r| r.origin)
            .collect();
        assert_eq!(origins, [RevealOrigin::Script]);
    }

    #[test]
    fn settle_applies_the_removals_then_resets_the_scroll_of_each_node() {
        let (tree, ids) = rooted_tree(2);
        let removals = tree.removals(&[ids[0], ids[1]]);
        let mut state = DocumentState::from_tree(tree);
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
        state.add_plain_listener(target(3), "click", 1);
        state.add_plain_listener(target(4), "click", 2);
        state.add_plain_listener(target(5), "click", 3);

        let mut callbacks = free_ids(&mut state, &mut view, &[3, 4]);
        callbacks.sort_unstable();

        assert_eq!(callbacks, [1, 2]);
        assert!(
            state
                .listeners()
                .listeners_for(target(3), "click")
                .is_empty()
        );
        assert!(
            state
                .listeners()
                .listeners_for(target(4), "click")
                .is_empty()
        );
        assert_eq!(state.listeners().listeners_for(target(5), "click").len(), 1);
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
