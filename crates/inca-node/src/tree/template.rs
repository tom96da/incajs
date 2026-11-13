// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The content of a template: a fragment the template owns.

use html5ever::ns;

use super::{FRAGMENT_TAG, NodeId, TreeError, VirtualTree};

impl VirtualTree {
    /// Creates a template and its empty content, and returns the id of the template. The content
    /// is no orphan: it lives and dies with the template.
    pub(super) fn create_template(
        &mut self,
        tag_name: String,
        prefix: Option<String>,
    ) -> Result<NodeId, TreeError> {
        self.next_id.checked_add(2).ok_or(TreeError::IdsExhausted)?;
        let template = self.allocate(tag_name, Some(ns!(html)), prefix)?;
        let content = self.allocate(FRAGMENT_TAG.to_owned(), None, None)?;
        self.orphans.remove(&content);
        if let Some(node) = self.nodes.get_mut(&template) {
            node.content = Some(content);
        }
        if let Some(node) = self.nodes.get_mut(&content) {
            node.owner = Some(template);
        }
        Ok(template)
    }

    /// The content fragment of the template `id`. `None` for an unknown id and for a node that is
    /// no template.
    #[must_use]
    pub fn template_content(&self, id: NodeId) -> Option<NodeId> {
        self.get(id)?.content
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::selector::query_selector;
    use crate::tests::{Fixture, HTML, SVG, children, node, text};

    #[test]
    fn a_template_has_an_empty_parentless_content_it_owns() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");

        let content = tree.template_content(template).unwrap();

        let fragment = tree.get(content).unwrap();
        assert_eq!(fragment.tag_name(), FRAGMENT_TAG);
        assert_eq!(fragment.parent(), None);
        assert_eq!(fragment.owner, Some(template));
        assert!(fragment.children().is_empty());
    }

    #[rstest]
    #[case::div("div")]
    #[case::style("style")]
    #[case::uppercase("TEMPLATE")]
    fn another_element_has_no_content(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let element = node(&mut tree, tag);

        assert_eq!(tree.template_content(element), None);
    }

    #[test]
    fn an_svg_template_has_no_content() {
        let mut tree = VirtualTree::new();
        let element = tree.create_element_ns(Some(SVG), None, "template").unwrap();

        assert_eq!(tree.template_content(element), None);
    }

    #[test]
    fn an_html_template_made_with_a_namespace_has_content() {
        let mut tree = VirtualTree::new();
        let element = tree
            .create_element_ns(Some(HTML), None, "template")
            .unwrap();

        assert!(tree.template_content(element).is_some());
    }

    #[test]
    fn the_content_of_an_unknown_id_is_none() {
        assert_eq!(VirtualTree::new().template_content(9), None);
    }

    #[test]
    fn a_template_without_two_free_ids_creates_nothing() {
        let mut tree = VirtualTree::new();
        tree.next_id = NodeId::MAX - 1;

        assert_eq!(tree.create_node("template"), Err(TreeError::IdsExhausted));

        assert!(tree.nodes.is_empty());
        assert_eq!(tree.next_id, NodeId::MAX - 1);
    }

    #[test]
    fn a_template_takes_two_ids() {
        let mut tree = VirtualTree::new();

        let template = node(&mut tree, "template");

        assert_eq!(tree.template_content(template), Some(template + 1));
        assert_eq!(node(&mut tree, "div"), template + 2);
    }

    #[test]
    fn an_append_to_a_template_fills_its_children_and_not_its_content() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");
        let child = node(&mut tree, "span");

        tree.append_child(template, child).unwrap();

        assert_eq!(children(&tree, template), [child]);
        let content = tree.template_content(template).unwrap();
        assert!(children(&tree, content).is_empty());
    }

    #[test]
    fn inserting_the_content_moves_its_children_and_leaves_it_owned() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");
        let content = tree.template_content(template).unwrap();
        let child = node(&mut tree, "span");
        let parent = node(&mut tree, "div");
        tree.append_child(content, child).unwrap();

        tree.append_child(parent, content).unwrap();
        tree.append_child(parent, content).unwrap();

        assert_eq!(children(&tree, parent), [child]);
        assert!(children(&tree, content).is_empty());
        assert_eq!(tree.get(content).unwrap().parent(), None);
        assert_eq!(tree.template_content(template), Some(content));
    }

    #[test]
    fn a_held_template_keeps_its_filled_content_through_a_sweep() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");
        let content = tree.template_content(template).unwrap();
        let child = node(&mut tree, "span");
        tree.append_child(content, child).unwrap();
        tree.hold(template).unwrap();

        assert!(!tree.orphans.contains(&content));
        assert!(tree.sweep().is_empty());
        assert!(tree.get(child).is_some());
    }

    #[test]
    fn is_equal_ignores_the_content() {
        let mut tree = VirtualTree::new();
        let (a, b) = (node(&mut tree, "template"), node(&mut tree, "template"));
        let filled = tree.template_content(a).unwrap();
        let child = node(&mut tree, "span");
        tree.append_child(filled, child).unwrap();

        assert_eq!(tree.is_equal(a, b), Ok(true));
    }

    #[test]
    fn a_query_never_finds_a_node_of_a_content() {
        let mut f = Fixture::new();
        let template = f.add(f.root, "template", "template");
        let content = f.tree.template_content(template).unwrap();
        let inner = node(&mut f.tree, "span");
        f.tree.append_child(content, inner).unwrap();

        assert_eq!(query_selector(&f.tree, f.root, "span").unwrap(), None);
        assert_eq!(query_selector(&f.tree, template, "span").unwrap(), None);
    }

    #[test]
    fn a_style_in_a_content_applies_no_rule() {
        let mut f = Fixture::new();
        let template = f.add(f.root, "template", "template");
        let content = f.tree.template_content(template).unwrap();
        let style = node(&mut f.tree, "style");
        let rules = node(&mut f.tree, "#text");
        f.attr(rules, "value", "div { width: 9px }");
        f.tree.append_child(style, rules).unwrap();
        f.tree.append_child(content, style).unwrap();
        let target = f.boxed("target");

        assert_eq!(f.computed(target, "width"), None);
    }

    #[test]
    fn a_style_in_the_children_of_a_connected_template_applies() {
        let mut f = Fixture::new();
        let template = f.add(f.root, "template", "template");
        let style = f.add(template, "style", "sheet");
        let rules = node(&mut f.tree, "#text");
        f.attr(rules, "value", "div { width: 9px }");
        f.tree.append_child(style, rules).unwrap();
        let target = f.boxed("target");

        assert_eq!(f.computed(target, "width"), Some(text("9px")));
    }
}
