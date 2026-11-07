// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cloning, holding, releasing, sweeping and destroying nodes.

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

    /// Copies `id` with its tag, attributes and style props, and with its descendants when `deep`.
    /// Returns the original, the copy and the tag of each copy in pre-order, the copy of `id`
    /// first. Only that copy has no parent.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node and [`TreeError::IdsExhausted`]
    /// when no id is left. Copies made before the failure stay as orphans.
    pub fn clone_subtree(
        &mut self,
        id: NodeId,
        deep: bool,
    ) -> Result<Vec<(NodeId, NodeId, String)>, TreeError> {
        self.get(id).ok_or(TreeError::NodeNotFound(id))?;
        let mut made = Vec::new();
        let mut pending: Vec<(NodeId, Option<NodeId>)> = vec![(id, None)];
        while let Some((original, parent)) = pending.pop() {
            let node = self
                .get(original)
                .ok_or(TreeError::NodeNotFound(original))?;
            let (tag, attributes, style_props, children) = (
                node.tag_name.clone(),
                node.attributes.clone(),
                node.style_props.clone(),
                node.children.clone(),
            );
            let copy = self.create_node(tag.clone())?;
            let target = self.node_mut(copy)?;
            target.attributes = attributes;
            target.style_props = style_props;
            made.push((original, copy, tag));
            if let Some(parent) = parent {
                self.append_child(parent, copy)?;
            }
            if deep {
                pending.extend(children.into_iter().rev().map(|child| (child, Some(copy))));
            }
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{children, family, node, sample};
    use crate::tree::{AttributeValue, FRAGMENT_TAG};
    use rstest::rstest;

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

    #[test]
    fn a_shallow_clone_has_no_children() {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 3);

        let made = tree.clone_subtree(parent, false).unwrap();

        assert_eq!(made.len(), 1);
        assert!(children(&tree, made[0].1).is_empty());
    }

    #[test]
    fn a_deep_clone_lists_the_copies_in_pre_order() {
        let mut tree = VirtualTree::new();
        let root = sample(&mut tree, "div");

        let made = tree.clone_subtree(root, true).unwrap();

        let tags: Vec<&str> = made.iter().map(|(_, _, tag)| tag.as_str()).collect();
        assert_eq!(tags, ["div", "span", "#text", "#text"]);
        let ids: Vec<NodeId> = made.iter().map(|(_, copy, _)| *copy).collect();
        assert_eq!(children(&tree, ids[0]), [ids[1], ids[3]]);
        assert_eq!(children(&tree, ids[1]), [ids[2]]);
        assert_eq!(tree.is_equal(root, ids[0]), Ok(true));
    }

    #[rstest]
    #[case::fragment(FRAGMENT_TAG)]
    #[case::comment("#comment")]
    fn a_clone_keeps_the_kind_of_the_node(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let original = node(&mut tree, tag);

        let made = tree.clone_subtree(original, true).unwrap();

        assert_eq!(made[0].2, tag);
        assert_eq!(tree.get(made[0].1).unwrap().tag_name(), tag);
    }

    #[test]
    fn a_clone_keeps_the_attribute_order() {
        let mut tree = VirtualTree::new();
        let original = node(&mut tree, "div");
        for key in ["c", "a", "b"] {
            tree.set_attribute(original, key, key).unwrap();
        }

        let made = tree.clone_subtree(original, false).unwrap();

        let keys: Vec<_> = tree.get(made[0].1).unwrap().attributes().keys().collect();
        assert_eq!(keys, ["c", "a", "b"]);
    }

    #[test]
    fn a_clone_copies_the_attribute_and_style_maps_as_they_are() {
        let mut tree = VirtualTree::new();
        let original = sample(&mut tree, "div");

        let made = tree.clone_subtree(original, false).unwrap();

        let (from, to) = (tree.get(original).unwrap(), tree.get(made[0].1).unwrap());
        assert_eq!(to.attributes(), from.attributes());
        assert_eq!(to.style_props(), from.style_props());
        assert_eq!(
            to.attributes().get("tabindex"),
            Some(&AttributeValue::Number(1.0))
        );
    }

    #[test]
    fn a_clone_is_a_parentless_orphan() {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 2);

        let made = tree.clone_subtree(parent, true).unwrap();

        let copy = made[0].1;
        assert_eq!(tree.get(copy).unwrap().parent(), None);
        assert!(tree.orphans.contains(&copy));
        assert!(!tree.orphans.contains(&made[1].1));
    }

    #[test]
    fn a_clone_leaves_the_original_alone() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 2);

        tree.clone_subtree(parent, true).unwrap();

        assert_eq!(children(&tree, parent), kids);
    }

    #[test]
    fn a_clone_that_runs_out_of_ids_fails() {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 2);
        tree.next_id = NodeId::MAX - 2;

        assert_eq!(
            tree.clone_subtree(parent, true),
            Err(TreeError::IdsExhausted)
        );
    }

    #[test]
    fn the_copies_made_before_the_ids_ran_out_stay_as_orphans() {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 2);
        tree.next_id = NodeId::MAX - 2;

        let _ = tree.clone_subtree(parent, true);

        let copy = NodeId::MAX - 2;
        assert!(tree.orphans.contains(&copy));
        assert_eq!(children(&tree, copy), [NodeId::MAX - 1]);
    }

    #[test]
    fn a_clone_of_an_unknown_id_fails() {
        assert_eq!(
            VirtualTree::new().clone_subtree(4, true),
            Err(TreeError::NodeNotFound(4))
        );
    }
}
