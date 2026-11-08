// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rules for putting a node under a parent.

use super::{
    COMMENT_TAG, DOCUMENT_TAG, FRAGMENT_TAG, NodeId, TEXT_TAG, TreeError, VirtualNode, VirtualTree,
};

impl VirtualTree {
    /// The checks of an insertion in DOM order:
    ///
    /// - the ids exist;
    /// - the parent is no leaf;
    /// - the node is no ancestor of the parent;
    /// - the reference is a child of the parent;
    /// - the node is not the document;
    /// - a document parent keeps to one element and no text.
    ///
    /// `replaced` is the child the insertion removes. It changes nothing.
    pub(super) fn check_insertion(
        &self,
        parent_id: NodeId,
        node_id: NodeId,
        reference_id: Option<NodeId>,
        replaced: Option<NodeId>,
    ) -> Result<(), TreeError> {
        let parent = self
            .nodes
            .get(&parent_id)
            .ok_or(TreeError::NodeNotFound(parent_id))?;
        if !self.nodes.contains_key(&node_id) {
            return Err(TreeError::NodeNotFound(node_id));
        }
        if let Some(reference_id) = reference_id
            && !self.nodes.contains_key(&reference_id)
        {
            return Err(TreeError::NodeNotFound(reference_id));
        }
        if matches!(parent.tag_name(), TEXT_TAG | COMMENT_TAG) {
            return Err(TreeError::LeafParent(parent_id));
        }
        if self.is_self_or_ancestor(node_id, parent_id) {
            return Err(TreeError::WouldCycle(node_id));
        }
        if let Some(reference_id) = reference_id
            && !parent.children.contains(&reference_id)
        {
            return Err(TreeError::NotAChild {
                parent: parent_id,
                child: reference_id,
            });
        }
        if self.document == Some(node_id) {
            return Err(TreeError::DocumentInserted(node_id));
        }
        if parent.tag_name() == DOCUMENT_TAG {
            self.check_document_child(parent, node_id, replaced)?;
        }
        Ok(())
    }

    /// The rules of a document parent: no text, at most one element in all.
    fn check_document_child(
        &self,
        parent: &VirtualNode,
        node_id: NodeId,
        replaced: Option<NodeId>,
    ) -> Result<(), TreeError> {
        let (mut texts, mut elements) = (0, 0);
        for id in self.nodes_to_move(node_id) {
            match self.nodes.get(&id).map(VirtualNode::tag_name) {
                Some(TEXT_TAG) => texts += 1,
                Some(COMMENT_TAG) | None => {}
                Some(_) => elements += 1,
            }
        }
        let held = parent
            .children
            .iter()
            .filter(|&&id| Some(id) != replaced)
            .any(|id| {
                self.nodes
                    .get(id)
                    .is_some_and(|n| n.tag_name() != COMMENT_TAG)
            });
        if texts > 0 || elements > 1 || (elements == 1 && held) {
            return Err(TreeError::DocumentChild(node_id));
        }
        Ok(())
    }

    /// The nodes an insertion of `node_id` moves: `node_id`, or the children of `node_id` when it
    /// is a fragment. An unknown id gives itself.
    #[must_use]
    pub fn nodes_to_move(&self, node_id: NodeId) -> Vec<NodeId> {
        match self.nodes.get(&node_id) {
            Some(node) if node.tag_name() == FRAGMENT_TAG => node.children.clone(),
            _ => vec![node_id],
        }
    }

    /// Whether `candidate` is `of` itself or sits above it. The parentless content of a template
    /// continues at its template.
    fn is_self_or_ancestor(&self, candidate: NodeId, of: NodeId) -> bool {
        let mut current = Some(of);
        while let Some(id) = current {
            if id == candidate {
                return true;
            }
            current = self
                .nodes
                .get(&id)
                .and_then(|node| node.parent.or(node.owner));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{Fixture, children, family, node, not_a_child};

    #[rstest]
    #[case::parent(|_: NodeId, child: NodeId| (999, child, None))]
    #[case::node(|parent: NodeId, _: NodeId| (parent, 999, None))]
    #[case::reference(|parent: NodeId, child: NodeId| (parent, child, Some(999)))]
    fn insert_before_unknown_ids_error(
        #[case] ids: fn(NodeId, NodeId) -> (NodeId, NodeId, Option<NodeId>),
    ) {
        let mut tree = VirtualTree::new();
        let parent = node(&mut tree, "div");
        let child = node(&mut tree, "span");
        let (parent, node_id, reference) = ids(parent, child);

        assert_eq!(
            tree.insert_before(parent, node_id, reference).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
    }

    #[rstest]
    #[case::child_of_another_parent("other")]
    #[case::parentless_node("loose")]
    #[case::fragment("fragment")]
    fn insert_before_throws_for_a_reference_that_is_not_a_child(#[case] kind: &str) {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 1);
        let (other, others) = family(&mut tree, 1);
        let reference = match kind {
            "other" => others[0],
            "loose" => node(&mut tree, "span"),
            _ => node(&mut tree, FRAGMENT_TAG),
        };
        let extra = node(&mut tree, "span");

        assert_eq!(
            tree.insert_before(parent, extra, Some(reference))
                .unwrap_err(),
            not_a_child(parent, reference)
        );
        assert_eq!(tree.get(extra).unwrap().parent(), None);
        assert_eq!(children(&tree, other), others);
    }

    #[test]
    fn insert_before_checks_the_leaf_parent_before_the_reference() {
        let mut tree = VirtualTree::new();
        let leaf = node(&mut tree, TEXT_TAG);
        let extra = node(&mut tree, "span");
        let stray = node(&mut tree, "span");

        assert_eq!(
            tree.insert_before(leaf, extra, Some(stray)).unwrap_err(),
            TreeError::LeafParent(leaf)
        );
    }

    #[test]
    fn insert_before_checks_the_cycle_before_the_reference() {
        let mut tree = VirtualTree::new();
        let (parent, kids) = family(&mut tree, 1);
        let stray = node(&mut tree, "span");

        assert_eq!(
            tree.insert_before(kids[0], parent, Some(stray))
                .unwrap_err(),
            TreeError::WouldCycle(parent)
        );
    }

    #[test]
    fn insert_before_checks_the_reference_before_the_root() {
        let mut f = Fixture::new();
        let parent = node(&mut f.tree, "div");
        let stray = node(&mut f.tree, "span");

        assert_eq!(
            f.tree
                .insert_before(parent, f.root, Some(stray))
                .unwrap_err(),
            not_a_child(parent, stray)
        );
    }

    #[test]
    fn insert_before_checks_the_reference_before_the_adjustment() {
        let mut tree = VirtualTree::new();
        let (parent, _) = family(&mut tree, 1);
        let loose = node(&mut tree, "span");

        assert_eq!(
            tree.insert_before(parent, loose, Some(loose)).unwrap_err(),
            not_a_child(parent, loose)
        );
    }

    #[rstest]
    #[case::text(TEXT_TAG)]
    #[case::comment(COMMENT_TAG)]
    fn a_leaf_node_cannot_be_a_parent(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let leaf = node(&mut tree, tag);
        let extra = node(&mut tree, "span");

        assert_eq!(
            tree.append_child(leaf, extra).unwrap_err(),
            TreeError::LeafParent(leaf)
        );
    }

    #[test]
    fn attaching_a_node_under_itself_is_refused() {
        let mut tree = VirtualTree::new();
        let one = node(&mut tree, "div");

        assert_eq!(
            tree.append_child(one, one).unwrap_err(),
            TreeError::WouldCycle(one)
        );
    }

    #[test]
    fn attaching_an_ancestor_under_its_descendant_is_refused() {
        let mut tree = VirtualTree::new();
        let (grandparent, kids) = family(&mut tree, 1);
        let child = node(&mut tree, "div");
        tree.append_child(kids[0], child).unwrap();

        assert_eq!(
            tree.append_child(child, grandparent).unwrap_err(),
            TreeError::WouldCycle(grandparent)
        );
        assert_eq!(children(&tree, grandparent), kids);
    }

    #[test]
    fn the_root_moved_under_a_connected_parent_is_a_cycle() {
        let mut f = Fixture::new();
        let child = f.add(f.root, "div", "child");

        assert_eq!(
            f.tree.append_child(child, f.root).unwrap_err(),
            TreeError::WouldCycle(f.root)
        );
    }

    #[test]
    fn the_root_takes_children() {
        let mut f = Fixture::new();
        let extra = node(&mut f.tree, "div");

        f.tree.append_child(f.root, extra).unwrap();

        assert_eq!(children(&f.tree, f.root), [extra]);
    }

    #[rstest]
    #[case::template(|template: NodeId, _: NodeId, _: NodeId| template)]
    #[case::ancestor_of_the_template(|_: NodeId, ancestor: NodeId, _: NodeId| ancestor)]
    fn a_node_above_the_content_cannot_go_into_the_content(
        #[case] pick: fn(NodeId, NodeId, NodeId) -> NodeId,
    ) {
        let mut tree = VirtualTree::new();
        let ancestor = node(&mut tree, "div");
        let template = node(&mut tree, "template");
        let inside = node(&mut tree, "span");
        tree.append_child(ancestor, template).unwrap();
        let content = tree.template_content(template).unwrap();
        tree.append_child(content, inside).unwrap();
        let moved = pick(template, ancestor, inside);

        assert_eq!(
            tree.append_child(content, moved).unwrap_err(),
            TreeError::WouldCycle(moved)
        );
    }

    #[test]
    fn a_template_cannot_go_into_a_node_of_its_content() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");
        let inside = node(&mut tree, "span");
        let content = tree.template_content(template).unwrap();
        tree.append_child(content, inside).unwrap();

        assert_eq!(
            tree.append_child(inside, template).unwrap_err(),
            TreeError::WouldCycle(template)
        );
    }

    #[test]
    fn the_document_cannot_be_inserted() {
        let (mut f, document) = Fixture::with_document();
        let parent = node(&mut f.tree, "div");

        assert_eq!(
            f.tree.append_child(parent, document).unwrap_err(),
            TreeError::DocumentInserted(document)
        );
    }

    type Operand = fn(&mut VirtualTree) -> NodeId;

    fn fragment_of(tree: &mut VirtualTree, tags: &[&str]) -> NodeId {
        let fragment = node(tree, FRAGMENT_TAG);
        for tag in tags {
            let child = node(tree, tag);
            tree.append_child(fragment, child).unwrap();
        }
        fragment
    }

    #[rstest]
    #[case::second_element(|tree: &mut VirtualTree| node(tree, "div"))]
    #[case::text(|tree: &mut VirtualTree| node(tree, TEXT_TAG))]
    #[case::fragment_of_two_elements(|tree: &mut VirtualTree| fragment_of(tree, &["div", "span"]))]
    #[case::fragment_with_text(|tree: &mut VirtualTree| fragment_of(tree, &[COMMENT_TAG, TEXT_TAG]))]
    fn the_document_refuses(#[case] make: Operand) {
        let (mut f, document) = Fixture::with_document();
        let operand = make(&mut f.tree);

        assert_eq!(
            f.tree.append_child(document, operand).unwrap_err(),
            TreeError::DocumentChild(operand)
        );
        assert_eq!(children(&f.tree, document), [f.root]);
    }

    #[rstest]
    #[case::comment_before_root(|tree: &mut VirtualTree| node(tree, COMMENT_TAG), true)]
    #[case::fragment_of_comments(|tree: &mut VirtualTree| fragment_of(tree, &[COMMENT_TAG, COMMENT_TAG]), false)]
    #[case::empty_fragment(|tree: &mut VirtualTree| fragment_of(tree, &[]), false)]
    fn the_document_takes(#[case] make: Operand, #[case] before_root: bool) {
        let (mut f, document) = Fixture::with_document();
        let operand = make(&mut f.tree);
        let reference = before_root.then_some(f.root);

        f.tree.insert_before(document, operand, reference).unwrap();

        assert_eq!(f.tree.document_element(), Some(f.root));
    }

    #[test]
    fn the_document_takes_an_element_after_its_element_left() {
        let (mut f, document) = Fixture::with_document();
        let comment = node(&mut f.tree, COMMENT_TAG);
        f.tree.append_child(document, comment).unwrap();
        f.tree.remove_child(document, f.root).unwrap();
        let next = node(&mut f.tree, "div");

        f.tree.append_child(document, next).unwrap();

        assert_eq!(f.tree.document_element(), Some(next));
    }

    #[rstest]
    #[case::element(|tree: &mut VirtualTree| node(tree, "div"))]
    #[case::comment(|tree: &mut VirtualTree| node(tree, COMMENT_TAG))]
    fn the_root_can_be_replaced_in_the_document(#[case] make: Operand) {
        let (mut f, document) = Fixture::with_document();
        let operand = make(&mut f.tree);

        f.tree.replace_child(document, operand, f.root).unwrap();

        assert_eq!(children(&f.tree, document), [operand]);
    }

    #[test]
    fn a_comment_cannot_be_replaced_by_an_element_beside_the_root() {
        let (mut f, document) = Fixture::with_document();
        let comment = node(&mut f.tree, COMMENT_TAG);
        f.tree.append_child(document, comment).unwrap();
        let element = node(&mut f.tree, "div");

        assert_eq!(
            f.tree
                .replace_child(document, element, comment)
                .unwrap_err(),
            TreeError::DocumentChild(element)
        );
    }

    #[test]
    fn the_root_moves_under_a_detached_element_while_a_document_is_declared() {
        let (mut f, _) = Fixture::with_document();
        let parent = node(&mut f.tree, "div");

        f.tree.append_child(parent, f.root).unwrap();

        assert_eq!(f.tree.get(f.root).unwrap().parent(), Some(parent));
    }
}
