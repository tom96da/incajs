// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Holding, releasing, sweeping and destroying nodes.

use super::{NodeId, TreeError, VirtualTree};

impl VirtualTree {
    /// Keeps `id` and its subtree out of [`sweep`](Self::sweep) while it has
    /// no parent. Holding a held, attached or root node changes nothing
    /// visible. [`destroy_node`](Self::destroy_node) ends the hold.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn hold(&mut self, id: NodeId) -> Result<(), TreeError> {
        if !self.nodes.contains_key(&id) {
            return Err(TreeError::NodeNotFound(id));
        }
        self.held.insert(id);
        self.orphans.remove(&id);
        Ok(())
    }

    /// Ends the hold of `id`. A parentless node that is not the root becomes
    /// an orphan. An unknown or unheld `id` changes nothing.
    pub fn release(&mut self, id: NodeId) {
        if self.held.remove(&id) {
            self.orphan_if_free(id);
        }
    }

    /// Whether a [`sweep`](Self::sweep) would free a node.
    #[must_use]
    pub fn has_orphans(&self) -> bool {
        !self.orphans.is_empty()
    }

    /// Frees every orphan with its subtree, as
    /// [`destroy_node`](Self::destroy_node) does, and returns every freed id.
    pub fn sweep(&mut self) -> Vec<NodeId> {
        let mut orphans: Vec<NodeId> = self.orphans.drain().collect();
        orphans.sort_unstable();
        orphans
            .into_iter()
            .flat_map(|id| self.destroy_node(id))
            .collect()
    }

    /// Marks `id` an orphan when it is a parentless node, not the root and
    /// not held.
    pub(super) fn orphan_if_free(&mut self, id: NodeId) {
        let free = self.nodes.get(&id).is_some_and(|n| n.parent.is_none())
            && self.root != Some(id)
            && !self.held.contains(&id);
        if free {
            self.orphans.insert(id);
        }
    }

    /// Whether `id` is the root or its parent chain reaches it. Always true
    /// while no root is declared. With a root, a destroyed or unknown id is
    /// not connected.
    #[must_use]
    pub fn is_connected(&self, id: NodeId) -> bool {
        let Some(root) = self.root else {
            return true;
        };
        let mut current = Some(id);
        while let Some(id) = current {
            if id == root {
                return self.nodes.contains_key(&id);
            }
            current = self.nodes.get(&id).and_then(|node| node.parent);
        }
        false
    }

    /// Frees `id` and its whole subtree, detaching it from its parent first,
    /// and returns every freed id, `id` among them.
    ///
    /// The subtree goes too because a caller removing a node holds that
    /// node's id and nothing below it. Also ends the hold of every freed id.
    /// An unknown `id` frees nothing and returns an empty `Vec`: destroying
    /// twice is a no-op, not an error.
    pub fn destroy_node(&mut self, id: NodeId) -> Vec<NodeId> {
        if !self.nodes.contains_key(&id) {
            return Vec::new();
        }
        self.generation += 1;
        self.detach(id);

        let mut freed = Vec::new();
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            if let Some(node) = self.nodes.remove(&current) {
                pending.extend(node.children.iter().copied());
                self.held.remove(&current);
                self.orphans.remove(&current);
                freed.push(current);
            }
        }
        freed
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn destroy_frees_the_whole_subtree() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let branch = tree.create_node("div").unwrap();
        let leaf = tree.create_node("#text").unwrap();
        tree.append_child(root, branch).unwrap();
        tree.append_child(branch, leaf).unwrap();

        let mut freed = tree.destroy_node(branch);
        freed.sort_unstable();

        assert_eq!(
            freed,
            vec![branch, leaf],
            "a descendant is unreachable once its root is gone, so it must go too"
        );
        assert!(tree.get(branch).is_none());
        assert!(tree.get(leaf).is_none());
        assert!(tree.get(root).unwrap().children().is_empty());
    }

    #[test]
    fn destroy_is_a_no_op_the_second_time_and_for_an_unknown_id() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        assert_eq!(tree.destroy_node(node), vec![node]);
        assert!(tree.destroy_node(node).is_empty());
        assert!(tree.destroy_node(9999).is_empty());
    }

    #[test]
    fn a_tree_without_a_root_connects_every_id() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        assert!(tree.is_connected(id));
        assert!(tree.is_connected(999));
    }

    #[test]
    fn a_chain_attached_to_the_root_is_connected() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let mid = tree.create_node("div").unwrap();
        let leaf = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.append_child(root, mid).unwrap();
        tree.append_child(mid, leaf).unwrap();
        assert!(tree.is_connected(root));
        assert!(tree.is_connected(mid));
        assert!(tree.is_connected(leaf));
    }

    #[test]
    fn a_detached_subtree_is_not_connected_until_it_is_attached_again() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let mid = tree.create_node("div").unwrap();
        let leaf = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.append_child(root, mid).unwrap();
        tree.append_child(mid, leaf).unwrap();

        tree.remove_child(root, mid).unwrap();
        assert!(!tree.is_connected(mid));
        assert!(!tree.is_connected(leaf));

        tree.append_child(root, mid).unwrap();
        assert!(tree.is_connected(leaf));
    }

    #[test]
    fn a_destroyed_node_is_not_connected_under_a_root() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.append_child(root, child).unwrap();
        tree.destroy_node(child);
        assert!(!tree.is_connected(child));
    }

    #[test]
    fn an_unknown_id_is_not_connected_under_a_root() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        assert!(!tree.is_connected(999));
    }

    fn orphan_pair(tree: &mut VirtualTree) -> (NodeId, NodeId) {
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();
        tree.append_child(parent, child).unwrap();
        (parent, child)
    }

    #[test]
    fn sweep_frees_an_orphan_and_its_subtree() {
        let mut tree = VirtualTree::new();
        let (parent, child) = orphan_pair(&mut tree);

        assert_eq!(tree.sweep(), [parent, child]);
        assert!(tree.get(parent).is_none());
        assert!(tree.get(child).is_none());
        assert!(!tree.has_orphans());
    }

    #[test]
    fn an_insert_before_the_sweep_saves_the_node() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        let (parent, child) = orphan_pair(&mut tree);
        tree.append_child(root, parent).unwrap();

        assert!(tree.sweep().is_empty());
        assert!(tree.get(parent).is_some());
        assert!(tree.get(child).is_some());
    }

    #[test]
    fn a_held_parentless_node_survives_until_it_is_released() {
        let mut tree = VirtualTree::new();
        let (parent, child) = orphan_pair(&mut tree);
        tree.hold(parent).unwrap();

        assert!(tree.sweep().is_empty());
        assert!(tree.get(child).is_some());

        tree.release(parent);
        assert!(tree.has_orphans());
        assert_eq!(tree.sweep(), [parent, child]);
    }

    #[test]
    fn a_held_attached_node_stays_after_it_is_detached() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        let node = tree.create_node("div").unwrap();
        tree.append_child(root, node).unwrap();
        tree.hold(node).unwrap();
        tree.remove_child(root, node).unwrap();

        assert!(tree.sweep().is_empty());
        assert!(tree.get(node).is_some());
    }

    #[test]
    fn destroy_node_clears_the_hold_and_the_orphan_entry() {
        let mut tree = VirtualTree::new();
        let held = tree.create_node("div").unwrap();
        let loose = tree.create_node("div").unwrap();
        tree.hold(held).unwrap();

        assert_eq!(tree.destroy_node(held), [held]);
        assert_eq!(tree.destroy_node(loose), [loose]);

        assert!(!tree.has_orphans());
        assert!(tree.sweep().is_empty());
        assert!(!tree.held.contains(&held));
    }

    #[test]
    fn an_empty_orphan_set_sweeps_nothing() {
        let mut tree = VirtualTree::new();
        assert!(!tree.has_orphans());
        assert!(tree.sweep().is_empty());
    }

    #[test]
    fn a_node_created_and_never_inserted_is_an_orphan() {
        let mut tree = VirtualTree::new();
        tree.create_node("div").unwrap();
        assert!(tree.has_orphans());
    }

    #[test]
    fn release_of_an_attached_node_makes_no_orphan() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        let node = tree.create_node("div").unwrap();
        tree.append_child(root, node).unwrap();
        tree.hold(node).unwrap();

        tree.release(node);

        assert!(!tree.has_orphans());
    }

    #[test]
    fn the_root_is_never_an_orphan() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.hold(root).unwrap();
        tree.release(root);

        assert!(!tree.has_orphans());
        assert!(tree.sweep().is_empty());
        assert_eq!(tree.root(), Some(root));
    }

    #[test]
    fn hold_of_an_unknown_id_errors() {
        let mut tree = VirtualTree::new();
        assert_eq!(tree.hold(7), Err(TreeError::NodeNotFound(7)));
    }

    #[test]
    fn a_detached_node_is_an_orphan_until_it_is_inserted() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        let node = tree.create_node("div").unwrap();
        tree.append_child(root, node).unwrap();
        assert!(!tree.has_orphans());

        tree.remove_child(root, node).unwrap();

        assert!(tree.has_orphans());
        assert_eq!(tree.sweep(), [node]);
    }

    #[test]
    fn a_held_node_under_a_swept_orphan_is_freed_with_it() {
        let mut tree = VirtualTree::new();
        let (parent, child) = orphan_pair(&mut tree);
        tree.hold(child).unwrap();

        assert_eq!(tree.sweep(), [parent, child]);
        assert!(tree.held.is_empty());

        tree.release(child);
        assert!(!tree.has_orphans());
    }
}
