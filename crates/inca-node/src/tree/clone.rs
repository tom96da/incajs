// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cloning a node.

use super::{NodeId, NodeKind, TreeError, VirtualTree};

impl VirtualTree {
    /// Copies `id` with its kind, name, namespace, prefix, attributes and style props, and
    /// with its descendants when `deep`. Returns the original and the copy of each node in
    /// pre-order, the copy of `id` first. Only that copy has no parent. The copy of a template
    /// comes with a fresh content that follows it in the list, with the content descendants when
    /// `deep`, before the other children.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node and [`TreeError::IdsExhausted`]
    /// when no id is left. Copies made before the failure stay as orphans.
    pub fn clone_subtree(
        &mut self,
        id: NodeId,
        deep: bool,
    ) -> Result<Vec<(NodeId, NodeId)>, TreeError> {
        self.get(id).ok_or(TreeError::NodeNotFound(id))?;
        let mut made = Vec::new();
        let mut pending = vec![Step::Node(id, None)];
        while let Some(step) = pending.pop() {
            let (original, copy) = match step {
                Step::Content(original, copy) => (original, copy),
                Step::Node(original, parent) => {
                    let node = self
                        .get(original)
                        .ok_or(TreeError::NodeNotFound(original))?;
                    let (kind, tag, namespace, prefix) = (
                        node.kind,
                        node.tag_name.clone(),
                        node.namespace.clone(),
                        node.prefix.clone(),
                    );
                    let (attributes, style_props) = (
                        node.attributes.clone(),
                        node.style_props.clone(),
                    );
                    let copy = if kind == NodeKind::Element {
                        self.create_element(namespace, prefix, tag)?
                    } else {
                        self.allocate(kind, tag, namespace, prefix)?
                    };
                    let target = self.node_mut(copy)?;
                    target.attributes = attributes;
                    target.style_props = style_props;
                    if let Some(parent) = parent {
                        self.append_child(parent, copy)?;
                    }
                    (original, copy)
                }
            };
            made.push((original, copy));
            let node = self
                .get(original)
                .ok_or(TreeError::NodeNotFound(original))?;
            let (children, content) = (node.children.clone(), node.content);
            if deep {
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|child| Step::Node(child, Some(copy))),
                );
            }
            if let Some(content) = content {
                let fresh = self
                    .template_content(copy)
                    .ok_or(TreeError::NodeNotFound(copy))?;
                pending.push(Step::Content(content, fresh));
            }
        }
        Ok(made)
    }
}

/// A node `clone_subtree` still has to copy: an ordinary node with the copy of its parent, or a
/// template content with the content its template copy made.
enum Step {
    Node(NodeId, Option<NodeId>),
    Content(NodeId, NodeId),
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::tests::{SVG, children, family, node, sample};
    use crate::tree::{AttributeValue, FRAGMENT_TAG};
    use rstest::rstest;

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

        let ids: Vec<NodeId> = made.iter().map(|&(_, copy)| copy).collect();
        let tags: Vec<&str> = ids
            .iter()
            .map(|&id| tree.get(id).unwrap().tag_name())
            .collect();
        assert_eq!(tags, ["div", "span", "#text", "#text"]);
        assert_eq!(children(&tree, ids[0]), [ids[1], ids[3]]);
        assert_eq!(children(&tree, ids[1]), [ids[2]]);
        assert_eq!(tree.is_equal(root, ids[0]), Ok(true));
    }

    #[rstest]
    #[case::fragment(FRAGMENT_TAG, NodeKind::Fragment)]
    #[case::comment("#comment", NodeKind::Comment)]
    #[case::text("#text", NodeKind::Text)]
    #[case::element("div", NodeKind::Element)]
    fn a_clone_keeps_the_kind_of_the_node(#[case] tag: &str, #[case] kind: NodeKind) {
        let mut tree = VirtualTree::new();
        let original = node(&mut tree, tag);

        let made = tree.clone_subtree(original, true).unwrap();

        let copy = tree.get(made[0].1).unwrap();
        assert_eq!(copy.tag_name(), tag);
        assert_eq!(copy.kind(), kind);
    }

    #[test]
    fn a_clone_of_a_template_keeps_a_fragment_content() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");

        let made = tree.clone_subtree(template, true).unwrap();

        let content = tree.template_content(made[0].1).unwrap();
        assert_eq!(tree.get(content).unwrap().kind(), NodeKind::Fragment);
    }

    #[test]
    fn a_shallow_clone_keeps_the_namespace_and_the_prefix() {
        let mut tree = VirtualTree::new();
        let svg = tree.create_element_ns(Some(SVG), Some("s"), "svg").unwrap();

        let made = tree.clone_subtree(svg, false).unwrap();

        let copy = tree.get(made[0].1).unwrap();
        assert_eq!(copy.namespace_uri(), Some(SVG));
        assert_eq!(copy.prefix(), Some("s"));
    }

    #[test]
    fn a_deep_clone_keeps_the_namespace_and_the_prefix_of_a_descendant() {
        let mut tree = VirtualTree::new();
        let svg = tree.create_element_ns(Some(SVG), None, "svg").unwrap();
        let inner = tree
            .create_element_ns(Some(SVG), Some("s"), "foreignObject")
            .unwrap();
        tree.append_child(svg, inner).unwrap();

        let made = tree.clone_subtree(svg, true).unwrap();

        let copy = tree.get(made[1].1).unwrap();
        assert_eq!(copy.namespace_uri(), Some(SVG));
        assert_eq!(copy.prefix(), Some("s"));
    }

    #[test]
    fn a_clone_of_an_svg_template_has_no_content() {
        let mut tree = VirtualTree::new();
        let template = tree.create_element_ns(Some(SVG), None, "template").unwrap();

        let made = tree.clone_subtree(template, true).unwrap();

        assert_eq!(made.len(), 1);
        assert_eq!(tree.template_content(made[0].1), None);
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
    fn a_clone_keeps_the_declaration_order_and_the_style_entry() {
        let mut tree = VirtualTree::new();
        let original = node(&mut tree, "div");
        tree.set_attribute(original, "style", "height: 2px; width: 1px")
            .unwrap();

        let made = tree.clone_subtree(original, false).unwrap();

        let copy = tree.get(made[0].1).unwrap();
        assert_eq!(
            copy.attributes().get("style"),
            Some(&AttributeValue::String("height: 2px; width: 1px;".into()))
        );
        assert_eq!(crate::css_text(copy), "height: 2px; width: 1px;");
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

    #[test]
    fn a_shallow_clone_of_a_template_has_a_fresh_empty_content() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");
        let content = tree.template_content(template).unwrap();
        let inner = node(&mut tree, "span");
        tree.append_child(content, inner).unwrap();

        let made = tree.clone_subtree(template, false).unwrap();

        let tags: Vec<&str> = made
            .iter()
            .map(|&(_, copy)| tree.get(copy).unwrap().tag_name())
            .collect();
        assert_eq!(tags, ["template", FRAGMENT_TAG]);
        let copy = made[0].1;
        assert_eq!(tree.template_content(copy), Some(made[1].1));
        assert_ne!(made[1].1, content);
        assert!(children(&tree, made[1].1).is_empty());
    }

    #[test]
    fn a_deep_clone_of_a_template_lists_its_content_before_its_children() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");
        let content = tree.template_content(template).unwrap();
        let (inner, own) = (node(&mut tree, "span"), node(&mut tree, "b"));
        tree.append_child(content, inner).unwrap();
        tree.append_child(template, own).unwrap();

        let made = tree.clone_subtree(template, true).unwrap();

        let tags: Vec<&str> = made
            .iter()
            .map(|&(_, copy)| tree.get(copy).unwrap().tag_name())
            .collect();
        assert_eq!(tags, ["template", FRAGMENT_TAG, "span", "b"]);
        let copies: Vec<NodeId> = made.iter().map(|&(_, copy)| copy).collect();
        assert_eq!(children(&tree, copies[1]), [copies[2]]);
        assert_eq!(children(&tree, copies[0]), [copies[3]]);
    }

    #[test]
    fn a_deep_clone_copies_the_content_of_a_template_in_a_content() {
        let mut tree = VirtualTree::new();
        let outer = node(&mut tree, "template");
        let outer_content = tree.template_content(outer).unwrap();
        let nested = node(&mut tree, "template");
        let nested_content = tree.template_content(nested).unwrap();
        let inner = node(&mut tree, "span");
        tree.append_child(outer_content, nested).unwrap();
        tree.append_child(nested_content, inner).unwrap();

        let made = tree.clone_subtree(outer, true).unwrap();

        let tags: Vec<&str> = made
            .iter()
            .map(|&(_, copy)| tree.get(copy).unwrap().tag_name())
            .collect();
        assert_eq!(
            tags,
            ["template", FRAGMENT_TAG, "template", FRAGMENT_TAG, "span"]
        );
        assert_eq!(children(&tree, made[3].1), [made[4].1]);
    }
}
