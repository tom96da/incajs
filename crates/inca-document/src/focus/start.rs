// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where Tab starts while no node is focused.

use super::{FocusModel, NodeId, VirtualTree};
use inca_node::tree::VirtualNode;

/// Where Tab starts while no node is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Start {
    /// A mouse press on `node`. `focusing` is the node the press focuses, until the model
    /// sees that focus.
    Press {
        node: NodeId,
        focusing: Option<NodeId>,
    },
    /// The node that took focus last.
    Focus(NodeId),
}

impl Start {
    pub(super) fn node(self) -> NodeId {
        match self {
            Self::Press { node, .. } | Self::Focus(node) => node,
        }
    }
}

/// Whether `node_id` is the document or a child of it. A tree without a document has none.
fn is_document_side(tree: &VirtualTree, node_id: NodeId) -> bool {
    tree.document().is_some_and(|document| {
        node_id == document || tree.get(node_id).and_then(VirtualNode::parent) == Some(document)
    })
}

impl FocusModel {
    /// Records a mouse press on `node_id` as where Tab starts while no node is
    /// focused. `focusing` is the node the press focuses, `None` when it
    /// focuses none or that node already holds focus. A press on `root`
    /// clears the start.
    pub fn press_start(&mut self, node_id: NodeId, root: NodeId, focusing: Option<NodeId>) {
        self.start = (node_id != root).then_some(Start::Press {
            node: node_id,
            focusing,
        });
    }

    /// The node Tab starts from while no node is focused.
    #[cfg(test)]
    pub(crate) fn start_node(&self) -> Option<NodeId> {
        self.start.map(Start::node)
    }

    /// Moves where Tab starts off `node_id`'s subtree, which leaves its
    /// place in the tree, to `before`, the node right before that place
    /// ([`VirtualTree::preceding`] taken before the change). A start that
    /// lies outside the subtree stays. `before` being the root, the document or one of its
    /// children, or `None`, clears the start.
    pub fn start_leaves(&mut self, tree: &VirtualTree, node_id: NodeId, before: Option<NodeId>) {
        let Some(start) = self.start else { return };
        if tree
            .ancestors(start.node())
            .any(|ancestor| ancestor == node_id)
        {
            self.start = before
                .filter(|&before| Some(before) != tree.root() && !is_document_side(tree, before))
                .map(|node| Start::Press {
                    node,
                    focusing: None,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{ROOT, rooted_tree};

    #[rstest::rstest]
    #[case::before_is_document(false)]
    #[case::before_is_comment_sibling(true)]
    fn a_start_in_the_root_clears_when_the_root_leaves_the_document(#[case] comment: bool) {
        let (mut tree, ids) = rooted_tree(1);
        let document = tree.document().unwrap();
        if comment {
            let comment = tree.create_node("#comment").unwrap();
            tree.insert_before(document, comment, Some(ROOT)).unwrap();
        }
        let mut registry = FocusModel::default();
        registry.press_start(ids[0], ROOT, None);
        let before = tree.preceding(ROOT);
        tree.remove_child(document, ROOT).unwrap();

        registry.start_leaves(&tree, ROOT, before);

        assert_eq!(registry.start, None);
    }

    #[test]
    fn a_start_clears_when_it_leaves_as_the_first_child_of_a_root_without_a_document() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.append_child(root, child).unwrap();
        let mut registry = FocusModel::default();
        registry.press_start(child, root, None);

        registry.start_leaves(&tree, child, tree.preceding(child));

        assert_eq!(registry.start, None);
    }

    #[test]
    fn a_parentless_before_other_than_the_root_keeps_the_start_without_a_document() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        let loose = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.append_child(root, child).unwrap();
        let mut registry = FocusModel::default();
        registry.press_start(child, root, None);

        registry.start_leaves(&tree, child, Some(loose));

        assert_eq!(
            registry.start,
            Some(Start::Press {
                node: loose,
                focusing: None
            })
        );
    }
}
