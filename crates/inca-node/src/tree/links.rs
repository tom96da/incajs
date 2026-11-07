// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Attaching, moving, replacing and detaching nodes.

use std::collections::HashSet;

use super::{NodeId, TreeError, VirtualTree};

impl VirtualTree {
    /// Appends `node_id` to `parent_id`'s children. Thin wrapper over
    /// [`insert_before`](Self::insert_before) with no reference child.
    ///
    /// # Errors
    ///
    /// Same as [`insert_before`](Self::insert_before), with no reference child to fail on.
    pub fn append_child(&mut self, parent_id: NodeId, node_id: NodeId) -> Result<usize, TreeError> {
        self.insert_before(parent_id, node_id, None)
    }

    /// Inserts `node_id` into `parent_id`'s children, before `reference_id` if given, or at the
    /// end if `None`, and returns the position of the first inserted child among the children of
    /// `parent_id`. `node_id` is detached from its old parent first, so this both attaches and
    /// moves. A fragment moves its children in order and stays empty and parentless. An empty
    /// fragment changes nothing and returns the position it would take.
    ///
    /// A reference child equal to `node_id` stands for the next sibling of `node_id`.
    ///
    /// # Errors
    ///
    /// All checks run before the first change. The first one that fails returns:
    ///
    /// - [`TreeError::NodeNotFound`] if `parent_id`, `node_id` or `reference_id` names no node,
    ///   checked in that order.
    /// - [`TreeError::LeafParent`] if `parent_id` is a text node or a comment.
    /// - [`TreeError::WouldCycle`] if `node_id` is `parent_id` or one of its ancestors.
    /// - [`TreeError::NotAChild`] if `reference_id` is not a child of `parent_id`.
    /// - [`TreeError::RootMoved`] if `node_id` is the root.
    pub fn insert_before(
        &mut self,
        parent_id: NodeId,
        node_id: NodeId,
        reference_id: Option<NodeId>,
    ) -> Result<usize, TreeError> {
        self.check_insertion(parent_id, node_id, reference_id)?;
        let reference = match reference_id {
            Some(reference) if reference == node_id => self.next_sibling(node_id),
            other => other,
        };
        Ok(self.place(parent_id, self.nodes_to_move(node_id), reference))
    }

    /// Detaches `nodes` and inserts them in order before `reference`, or last without one.
    /// Returns the position of the first of them among the children of `parent_id`.
    pub(super) fn place(
        &mut self,
        parent_id: NodeId,
        nodes: Vec<NodeId>,
        reference: Option<NodeId>,
    ) -> usize {
        self.detach_all(&nodes);
        let Some(parent) = self.nodes.get(&parent_id) else {
            return 0;
        };
        let index = reference
            .and_then(|reference| parent.children.iter().position(|&id| id == reference))
            .unwrap_or(parent.children.len());
        if nodes.is_empty() {
            return index;
        }
        self.generation += 1;
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.splice(index..index, nodes.iter().copied());
        }
        for id in nodes {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.parent = Some(parent_id);
            }
            self.orphans.remove(&id);
        }
        index
    }

    /// Moves every child of `from` to the end of `to`, in order, in one pass over the children.
    ///
    /// # Errors
    ///
    /// The checks of [`insert_before`](Self::insert_before) for inserting `from` into `to`.
    // The sources are nodes the current parse created, so none is the tree root.
    pub(crate) fn move_children(&mut self, from: NodeId, to: NodeId) -> Result<(), TreeError> {
        self.check_insertion(to, from, None)?;
        let nodes = self.nodes.get(&from).map(|node| node.children.clone());
        self.place(to, nodes.unwrap_or_default(), None);
        Ok(())
    }

    /// Unlinks every node of `ids` from its parent with one pass over the children.
    fn detach_all(&mut self, ids: &[NodeId]) {
        // Every caller passes one node or the children of one node: one shared parent.
        let first = ids.first().and_then(|id| self.nodes.get(id));
        let Some(parent_id) = first.and_then(|node| node.parent) else {
            return;
        };
        let moving: HashSet<NodeId> = ids.iter().copied().collect();
        self.generation += 1;
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.retain(|child| !moving.contains(child));
        }
        for id in ids {
            if let Some(node) = self.nodes.get_mut(id) {
                node.parent = None;
            }
        }
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

    /// Unlinks `child_id` from `parent_id`'s children. The child node keeps its own children and,
    /// unless held, is an orphan until it is inserted again. An unknown `child_id` changes
    /// nothing.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `parent_id` names no node, and
    /// [`TreeError::NotAChild`] if `child_id` names a node that is not a child of `parent_id`.
    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<(), TreeError> {
        let parent = self
            .nodes
            .get(&parent_id)
            .ok_or(TreeError::NodeNotFound(parent_id))?;
        if !parent.children.contains(&child_id) {
            if !self.nodes.contains_key(&child_id) {
                return Ok(());
            }
            return Err(TreeError::NotAChild {
                parent: parent_id,
                child: child_id,
            });
        }
        self.detach(child_id);
        self.orphan_if_free(child_id);
        Ok(())
    }

    /// Replaces `child_id` among the children of `parent_id` with `node_id` and returns the
    /// position of the first inserted child. `child_id` leaves the tree with its subtree and is an
    /// orphan unless held. `node_id` may be a descendant of `child_id`. A fragment moves its
    /// children.
    ///
    /// # Errors
    ///
    /// The checks and their order are those of [`insert_before`](Self::insert_before), with
    /// `child_id` as the node that has to be a child of `parent_id`.
    pub fn replace_child(
        &mut self,
        parent_id: NodeId,
        node_id: NodeId,
        child_id: NodeId,
    ) -> Result<usize, TreeError> {
        self.check_insertion(parent_id, node_id, Some(child_id))?;
        let mut reference = self.next_sibling(child_id);
        if reference == Some(node_id) {
            reference = self.next_sibling(node_id);
        }
        self.detach(child_id);
        self.orphan_if_free(child_id);
        Ok(self.place(parent_id, self.nodes_to_move(node_id), reference))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{Fixture, children, family, node, not_a_child, text};
    use crate::tree::{FRAGMENT_TAG, TEXT_TAG};

    #[test]
    fn append_child_keeps_the_call_order() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 3);

        assert_eq!(children(&tree, parent), kids);
    }

    #[rstest]
    #[case::start(0, 0)]
    #[case::middle(2, 2)]
    #[case::end_by_none(3, 3)]
    fn insert_before_returns_the_position(#[case] before: usize, #[case] expected: usize) {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 3);
        let extra = node(&mut tree, "span");
        let reference = kids.get(before).copied();

        let position = tree.insert_before(parent, extra, reference).unwrap();

        assert_eq!(position, expected);
        assert_eq!(children(&tree, parent)[expected], extra);
    }

    #[test]
    fn insert_before_counts_the_position_without_the_moved_sibling() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 4);

        let position = tree.insert_before(parent, kids[0], Some(kids[3])).unwrap();

        assert_eq!(position, 2);
        assert_eq!(
            children(&tree, parent),
            [kids[1], kids[2], kids[0], kids[3]]
        );
    }

    #[test]
    fn a_node_has_one_parent_at_a_time() {
        let mut tree = VirtualTree::new();
        let (first, kids) = family(&mut tree, 1);
        let second = node(&mut tree, "div");

        tree.append_child(second, kids[0]).unwrap();

        assert!(children(&tree, first).is_empty());
        assert_eq!(children(&tree, second), kids);
        assert_eq!(tree.get(kids[0]).unwrap().parent(), Some(second));
    }

    #[test]
    fn moving_a_child_within_one_parent_reorders_it() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 2);

        tree.insert_before(parent, kids[1], Some(kids[0])).unwrap();

        assert_eq!(children(&tree, parent), [kids[1], kids[0]]);
    }

    #[rstest]
    #[case::node_last(1)]
    #[case::node_first(0)]
    fn insert_before_itself_leaves_the_tree_unchanged(#[case] index: usize) {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 2);

        let position = tree
            .insert_before(parent, kids[index], Some(kids[index]))
            .unwrap();

        assert_eq!(position, index);
        assert_eq!(children(&tree, parent), kids);
    }

    #[test]
    fn inserting_a_fragment_moves_its_children_in_order() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 1);
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let (a, b) = (node(&mut tree, "i"), node(&mut tree, "b"));
        tree.append_child(fragment, a).unwrap();
        tree.append_child(fragment, b).unwrap();

        let position = tree.insert_before(parent, fragment, Some(kids[0])).unwrap();

        assert_eq!(position, 0);
        assert_eq!(children(&tree, parent), [a, b, kids[0]]);
        assert!(children(&tree, fragment).is_empty());
        assert_eq!(tree.get(a).unwrap().parent(), Some(parent));
        assert_eq!(tree.get(fragment).unwrap().parent(), None);
    }

    #[test]
    fn a_fragment_never_gets_a_parent() {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 1);
        let fragment = node(&mut tree, FRAGMENT_TAG);

        tree.append_child(parent, fragment).unwrap();

        assert_eq!(tree.get(fragment).unwrap().parent(), None);
        assert_eq!(children(&tree, parent).len(), 1);
    }

    #[test]
    fn a_refused_fragment_insert_keeps_the_fragment_children() {
        let mut tree = VirtualTree::new();
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let inner = node(&mut tree, "div");
        tree.append_child(fragment, inner).unwrap();

        assert_eq!(
            tree.append_child(inner, fragment).unwrap_err(),
            TreeError::WouldCycle(fragment)
        );
        assert_eq!(children(&tree, fragment), [inner]);
        assert_eq!(tree.get(inner).unwrap().parent(), Some(fragment));
    }

    #[test]
    fn inserting_an_empty_fragment_changes_nothing() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 2);
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let generation = tree.generation;

        let position = tree.insert_before(parent, fragment, Some(kids[1])).unwrap();

        assert_eq!(position, 1);
        assert_eq!(children(&tree, parent), kids);
        assert_eq!(tree.generation, generation);
    }

    #[test]
    fn move_children_keeps_their_order() {
        let mut tree = VirtualTree::new();
        let (from, kids) = family(&mut tree, 3);
        let to = node(&mut tree, "section");

        tree.move_children(from, to).unwrap();

        assert_eq!(children(&tree, to), kids);
    }

    #[test]
    fn move_children_empties_the_source() {
        let mut tree = VirtualTree::new();
        let (from, kids) = family(&mut tree, 3);
        let to = node(&mut tree, "section");

        tree.move_children(from, to).unwrap();

        assert_eq!(children(&tree, from), []);
        assert_eq!(tree.get(kids[2]).unwrap().parent(), Some(to));
    }

    #[test]
    fn move_children_advances_the_generation_by_the_same_step_for_any_count() {
        let step = |count: usize| {
            let mut tree = VirtualTree::new();
            let (from, _) = family(&mut tree, count);
            let to = node(&mut tree, "section");
            let generation = tree.generation;
            tree.move_children(from, to).unwrap();
            tree.generation - generation
        };

        assert_eq!(step(5), step(1));
    }

    #[test]
    fn move_children_refuses_a_target_inside_the_source() {
        let mut tree = VirtualTree::new();
        let (from, kids) = family(&mut tree, 1);

        assert_eq!(
            tree.move_children(from, kids[0]).unwrap_err(),
            TreeError::WouldCycle(from)
        );
        assert_eq!(children(&tree, from), kids);
    }

    #[test]
    fn a_moved_node_is_no_orphan() {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 0);
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let (loose, inner) = (node(&mut tree, "i"), node(&mut tree, "b"));
        tree.append_child(fragment, inner).unwrap();

        tree.append_child(parent, loose).unwrap();
        tree.append_child(parent, fragment).unwrap();

        assert!(!tree.orphans.contains(&loose));
        assert!(!tree.orphans.contains(&inner));
    }

    #[test]
    fn remove_child_clears_the_parent_link() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 1);

        tree.remove_child(parent, kids[0]).unwrap();

        assert_eq!(tree.get(kids[0]).unwrap().parent(), None);
        assert!(children(&tree, parent).is_empty());
    }

    #[test]
    fn remove_child_keeps_the_node_alive_as_an_orphan() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 1);

        tree.remove_child(parent, kids[0]).unwrap();

        assert!(tree.get(kids[0]).is_some());
        assert!(tree.orphans.contains(&kids[0]));
    }

    #[rstest]
    #[case::child_of_another_parent(true)]
    #[case::parentless_node(false)]
    fn remove_child_throws_for_a_known_node_that_is_not_a_child(#[case] has_parent: bool) {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 1);
        let (other, others) = family(&mut tree, 1);
        let target = if has_parent {
            others[0]
        } else {
            node(&mut tree, "span")
        };

        assert_eq!(
            tree.remove_child(parent, target).unwrap_err(),
            not_a_child(parent, target)
        );
        assert_eq!(children(&tree, other), others);
    }

    #[rstest]
    #[case::not_a_child(true)]
    #[case::unknown_parent(false)]
    fn a_refused_remove_child_leaves_the_generation_alone(#[case] known_parent: bool) {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 1);
        let loose = node(&mut tree, "span");
        let generation = tree.generation;

        let (parent, child) = if known_parent {
            (parent, loose)
        } else {
            (9999, 0)
        };
        assert!(tree.remove_child(parent, child).is_err());

        assert_eq!(tree.generation, generation);
    }

    #[test]
    fn remove_child_ignores_an_unknown_child() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 1);

        assert!(tree.remove_child(parent, 9999).is_ok());
        assert_eq!(children(&tree, parent), kids);
    }

    #[test]
    fn remove_child_throws_for_an_unknown_parent() {
        let mut tree = VirtualTree::new();

        assert_eq!(
            tree.remove_child(9999, 0).unwrap_err(),
            TreeError::NodeNotFound(9999)
        );
    }

    #[test]
    fn a_fragment_with_a_style_applies_its_rule_once_moved_into_the_tree() {
        let mut f = Fixture::new();
        let a = f.add(f.root, "div", "a");
        f.attr(a, "class", "a");
        let fragment = node(&mut f.tree, FRAGMENT_TAG);
        let style = node(&mut f.tree, "style");
        let css = node(&mut f.tree, TEXT_TAG);
        f.attr(css, "value", ".a { width: 1px }");
        f.tree.append_child(style, css).unwrap();
        f.tree.append_child(fragment, style).unwrap();
        let inside = f.tree.computed_style(a).get("width").cloned();

        f.tree.append_child(f.root, fragment).unwrap();

        assert_eq!(inside, None);
        assert_eq!(f.tree.computed_style(a).get("width"), Some(&text("1px")));
    }

    #[test]
    fn a_loose_style_applies_its_rule_once_appended_under_the_root() {
        let mut f = Fixture::new();
        let a = f.add(f.root, "div", "a");
        f.attr(a, "class", "a");
        let style = node(&mut f.tree, "style");
        let css = node(&mut f.tree, TEXT_TAG);
        f.attr(css, "value", ".a { width: 1px }");
        f.tree.append_child(style, css).unwrap();
        let loose = f.tree.computed_style(a).get("width").cloned();

        f.tree.append_child(f.root, style).unwrap();

        assert_eq!(loose, None);
        assert_eq!(f.tree.computed_style(a).get("width"), Some(&text("1px")));
    }

    #[rstest]
    #[case::style(false)]
    #[case::its_text(true)]
    fn removing_a_connected_style_stops_its_rules(#[case] remove_text: bool) {
        let mut f = Fixture::new();
        let (style, css) = f.sheet(".a { width: 1px }");
        let a = f.add(f.root, "div", "a");
        f.attr(a, "class", "a");
        let applied = f.tree.computed_style(a).get("width").cloned();

        if remove_text {
            f.tree.remove_child(style, css).unwrap();
        } else {
            f.tree.remove_child(f.root, style).unwrap();
        }

        assert_eq!(applied, Some(text("1px")));
        assert_eq!(f.tree.computed_style(a).get("width"), None);
    }

    #[test]
    fn a_replaced_child_is_an_orphan() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 1);
        let extra = node(&mut tree, "i");

        tree.replace_child(parent, extra, kids[0]).unwrap();

        assert!(tree.orphans.contains(&kids[0]));
    }

    #[test]
    fn replace_child_puts_the_node_at_the_position_of_the_child() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 3);
        let extra = node(&mut tree, "i");

        let position = tree.replace_child(parent, extra, kids[1]).unwrap();

        assert_eq!(position, 1);
        assert_eq!(children(&tree, parent), [kids[0], extra, kids[2]]);
        assert_eq!(tree.get(kids[1]).unwrap().parent(), None);
    }

    #[test]
    fn replace_child_with_the_child_itself_leaves_the_tree_unchanged() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 3);

        let position = tree.replace_child(parent, kids[1], kids[1]).unwrap();

        assert_eq!(position, 1);
        assert_eq!(children(&tree, parent), kids);
        assert!(!tree.orphans.contains(&kids[1]));
    }

    #[test]
    fn replace_child_puts_the_next_sibling_where_the_child_was() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 3);

        let position = tree.replace_child(parent, kids[2], kids[1]).unwrap();

        assert_eq!(position, 1);
        assert_eq!(children(&tree, parent), [kids[0], kids[2]]);
    }

    #[test]
    fn replace_child_counts_the_position_without_an_earlier_node() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 3);

        let position = tree.replace_child(parent, kids[0], kids[2]).unwrap();

        assert_eq!(position, 1);
        assert_eq!(children(&tree, parent), [kids[1], kids[0]]);
    }

    #[test]
    fn replace_child_with_a_fragment_moves_its_children() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 2);
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let (a, b) = (node(&mut tree, "i"), node(&mut tree, "b"));
        tree.append_child(fragment, a).unwrap();
        tree.append_child(fragment, b).unwrap();

        let position = tree.replace_child(parent, fragment, kids[0]).unwrap();

        assert_eq!(position, 0);
        assert_eq!(children(&tree, parent), [a, b, kids[1]]);
        assert!(children(&tree, fragment).is_empty());
    }

    #[test]
    fn replace_child_puts_a_descendant_where_the_child_was() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 2);
        let inner = node(&mut tree, "i");
        tree.append_child(kids[0], inner).unwrap();

        let position = tree.replace_child(parent, inner, kids[0]).unwrap();

        assert_eq!(position, 0);
        assert_eq!(children(&tree, parent), [inner, kids[1]]);
        assert!(children(&tree, kids[0]).is_empty());
        assert!(tree.orphans.contains(&kids[0]));
    }

    #[rstest]
    #[case::child_of_another_parent("other")]
    #[case::parentless_node("loose")]
    #[case::fragment("fragment")]
    fn replace_child_throws_for_a_child_that_is_not_a_child(#[case] kind: &str) {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 1);
        let (_, others) = family(&mut tree, 1);
        let target = match kind {
            "other" => others[0],
            "loose" => node(&mut tree, "span"),
            _ => node(&mut tree, FRAGMENT_TAG),
        };
        let extra = node(&mut tree, "i");

        assert_eq!(
            tree.replace_child(parent, extra, target).unwrap_err(),
            not_a_child(parent, target)
        );
        assert_eq!(children(&tree, parent), kids);
    }

    #[test]
    fn replace_child_throws_for_a_leaf_parent() {
        let mut tree = VirtualTree::new();
        let leaf = node(&mut tree, TEXT_TAG);
        let (extra, other) = (node(&mut tree, "i"), node(&mut tree, "b"));

        assert_eq!(
            tree.replace_child(leaf, extra, other).unwrap_err(),
            TreeError::LeafParent(leaf)
        );
    }
}
