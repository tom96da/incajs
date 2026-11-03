// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Attaching, moving and detaching nodes.

use super::{NodeId, TreeError, VirtualTree};

impl VirtualTree {
    /// Appends `child_id` to `parent_id`'s children. Thin wrapper over
    /// [`insert_before`](Self::insert_before) with no anchor.
    ///
    /// # Errors
    ///
    /// Same as [`insert_before`](Self::insert_before), with no anchor to
    /// fail on.
    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<(), TreeError> {
        self.insert_before(parent_id, child_id, None)
    }

    /// Inserts `child_id` into `parent_id`'s children, before `anchor_id` if
    /// given, or at the end if `None`. `child_id` is detached from whatever
    /// it was attached to first, so this both attaches and moves. A node
    /// never has two parents.
    ///
    /// If `anchor_id` is `child_id` itself and the child is already in
    /// `parent_id`, nothing changes.
    ///
    /// If `anchor_id` names a real node that isn't (or is no longer) among
    /// `parent_id`'s children, this falls back to appending at the end, same
    /// as an absent `remove_child` target. Only a truly
    /// unknown/never-allocated `anchor_id` is an error.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `parent_id` or `child_id`
    /// names no node, or if `anchor_id` is `Some` and names no node.
    /// Returns [`TreeError::WouldCycle`] if `child_id` is `parent_id` or one
    /// of its ancestors.
    pub fn insert_before(
        &mut self,
        parent_id: NodeId,
        child_id: NodeId,
        anchor_id: Option<NodeId>,
    ) -> Result<(), TreeError> {
        if !self.nodes.contains_key(&child_id) {
            return Err(TreeError::NodeNotFound(child_id));
        }
        if let Some(anchor_id) = anchor_id
            && !self.nodes.contains_key(&anchor_id)
        {
            return Err(TreeError::NodeNotFound(anchor_id));
        }
        if !self.nodes.contains_key(&parent_id) {
            return Err(TreeError::NodeNotFound(parent_id));
        }
        if self.is_self_or_ancestor(child_id, parent_id) {
            return Err(TreeError::WouldCycle(child_id));
        }

        // A child that is its own anchor stays where it is.
        if anchor_id == Some(child_id)
            && self
                .nodes
                .get(&child_id)
                .is_some_and(|c| c.parent == Some(parent_id))
        {
            return Ok(());
        }

        self.detach(child_id);

        let parent = self.node_mut(parent_id)?;
        let index = anchor_id
            .and_then(|anchor_id| parent.children.iter().position(|&id| id == anchor_id))
            .unwrap_or(parent.children.len());
        parent.children.insert(index, child_id);
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        self.orphans.remove(&child_id);
        Ok(())
    }

    /// Whether `candidate` is `of` itself or sits above it.
    fn is_self_or_ancestor(&self, candidate: NodeId, of: NodeId) -> bool {
        let mut current = Some(of);
        while let Some(id) = current {
            if id == candidate {
                return true;
            }
            current = self.nodes.get(&id).and_then(|node| node.parent);
        }
        false
    }

    /// Unlinks `id` from its parent, if it has one. Leaves the node itself
    /// alone, children and all.
    pub(super) fn detach(&mut self, id: NodeId) {
        let Some(parent_id) = self.nodes.get(&id).and_then(|node| node.parent) else {
            return;
        };
        self.generation += 1;
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.retain(|&child| child != id);
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = None;
        }
    }

    /// Unlinks `child_id` from `parent_id`'s children, if present. `child_id`
    /// not being (or no longer being) a child of `parent_id` is a no-op, not
    /// an error. Only an unknown `parent_id` is. The child node keeps its own
    /// children and, unless held, is an orphan until it is inserted again.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `parent_id` names no node.
    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<(), TreeError> {
        let parent = self.node_mut(parent_id)?;
        let before = parent.children.len();
        parent.children.retain(|&id| id != child_id);
        if parent.children.len() == before {
            return Ok(());
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = None;
        }
        self.orphan_if_free(child_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn append_child_keeps_the_call_order() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let a = tree.create_node("span").unwrap();
        let b = tree.create_node("span").unwrap();
        let c = tree.create_node("span").unwrap();

        tree.append_child(parent, a).unwrap();
        tree.append_child(parent, b).unwrap();
        tree.append_child(parent, c).unwrap();

        assert_eq!(tree.get(parent).unwrap().children(), &[a, b, c]);
    }

    #[test]
    fn insert_before_places_a_node_at_the_start_middle_and_end() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let a = tree.create_node("span").unwrap();
        let b = tree.create_node("span").unwrap();
        let c = tree.create_node("span").unwrap();

        tree.insert_before(parent, b, None).unwrap(); // end: [b]
        tree.insert_before(parent, a, Some(b)).unwrap(); // start: [a, b]
        tree.insert_before(parent, c, None).unwrap(); // end: [a, b, c]

        assert_eq!(tree.get(parent).unwrap().children(), &[a, b, c]);

        let d = tree.create_node("span").unwrap();
        tree.insert_before(parent, d, Some(b)).unwrap(); // middle: [a, d, b, c]
        assert_eq!(tree.get(parent).unwrap().children(), &[a, d, b, c]);
    }

    #[test]
    fn insert_before_itself_is_a_no_op() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let a = tree.create_node("span").unwrap();
        let b = tree.create_node("span").unwrap();
        tree.insert_before(parent, a, None).unwrap();
        tree.insert_before(parent, b, None).unwrap();

        tree.insert_before(parent, a, Some(a)).unwrap();

        assert_eq!(tree.get(parent).unwrap().children(), &[a, b]);
    }

    #[test]
    fn insert_before_unknown_anchor_falls_back_to_append() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let a = tree.create_node("span").unwrap();
        let stray_anchor = tree.create_node("span").unwrap(); // real node, never attached here

        tree.insert_before(parent, a, Some(stray_anchor)).unwrap();

        assert_eq!(tree.get(parent).unwrap().children(), &[a]);
    }

    #[test]
    fn insert_before_unknown_ids_error() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();

        assert_eq!(
            tree.insert_before(999, child, None).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.insert_before(parent, 999, None).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.insert_before(parent, child, Some(999)).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
    }

    #[test]
    fn a_node_has_one_parent_at_a_time() {
        let mut tree = VirtualTree::new();
        let first = tree.create_node("div").unwrap();
        let second = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();

        tree.append_child(first, child).unwrap();
        tree.append_child(second, child).unwrap();

        assert!(
            tree.get(first).unwrap().children().is_empty(),
            "attaching elsewhere must detach from the previous parent"
        );
        assert_eq!(tree.get(second).unwrap().children(), &[child]);
        assert_eq!(tree.get(child).unwrap().parent(), Some(second));
    }

    #[test]
    fn attaching_a_node_under_itself_is_refused() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        assert_eq!(
            tree.append_child(node, node).unwrap_err(),
            TreeError::WouldCycle(node)
        );
    }

    #[test]
    fn attaching_an_ancestor_under_its_descendant_is_refused() {
        let mut tree = VirtualTree::new();
        let grandparent = tree.create_node("div").unwrap();
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(grandparent, parent).unwrap();
        tree.append_child(parent, child).unwrap();

        assert_eq!(
            tree.append_child(child, grandparent).unwrap_err(),
            TreeError::WouldCycle(grandparent),
            "a cycle here would leave the render walk with no end"
        );
        assert_eq!(tree.get(grandparent).unwrap().children(), &[parent]);
    }

    #[test]
    fn moving_a_child_within_one_parent_reorders_it() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let a = tree.create_node("span").unwrap();
        let b = tree.create_node("span").unwrap();
        tree.append_child(parent, a).unwrap();
        tree.append_child(parent, b).unwrap();

        tree.insert_before(parent, b, Some(a)).unwrap();

        assert_eq!(
            tree.get(parent).unwrap().children(),
            &[b, a],
            "a move must not leave the node listed twice"
        );
    }

    #[test]
    fn remove_child_clears_the_parent_link() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();
        tree.append_child(parent, child).unwrap();

        tree.remove_child(parent, child).unwrap();

        assert_eq!(tree.get(child).unwrap().parent(), None);
    }

    #[test]
    fn remove_child_leaves_a_link_to_a_different_parent_alone() {
        let mut tree = VirtualTree::new();
        let real_parent = tree.create_node("div").unwrap();
        let stranger = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();
        tree.append_child(real_parent, child).unwrap();

        tree.remove_child(stranger, child).unwrap();

        assert_eq!(tree.get(child).unwrap().parent(), Some(real_parent));
    }

    #[test]
    fn detach_keeps_node_alive() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();
        tree.append_child(parent, child).unwrap();

        tree.remove_child(parent, child).unwrap();

        assert!(tree.get(parent).unwrap().children().is_empty());
        assert!(
            tree.get(child).is_some(),
            "detaching a child must not deallocate it"
        );
    }

    #[test]
    fn remove_of_absent_child_is_a_no_op() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let never_appended = tree.create_node("span").unwrap();

        // Absent child that exists elsewhere in the tree.
        assert!(tree.remove_child(parent, never_appended).is_ok());
        assert!(tree.get(parent).unwrap().children().is_empty());

        // Absent child id that was never allocated at all.
        assert!(tree.remove_child(parent, 9999).is_ok());
    }
}
