// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The inline style of a node as a block of declarations.

mod read;
mod write;

pub use read::{css_text, style_item, style_length, style_property_value};
pub use write::{
    remove_style, remove_style_property, set_style, set_style_property, set_style_text,
};

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::{css_text, remove_style, set_style, set_style_text};
    use crate::tests::node;
    use inca_node::tree::{AttributeValue, TreeError, VirtualTree};
    use inca_node::{HtmlScope, NodeId};

    fn outer(tree: &VirtualTree, id: NodeId) -> String {
        tree.serialize_html(id, HtmlScope::Outer).unwrap()
    }

    pub(super) fn block(props: &[(&str, AttributeValue)]) -> (VirtualTree, inca_node::NodeId) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        for (key, value) in props {
            super::set_style(&mut tree, id, *key, value.clone()).unwrap();
        }
        (tree, id)
    }

    pub(super) fn entry(tree: &VirtualTree, id: inca_node::NodeId) -> Option<String> {
        tree.get(id)
            .unwrap()
            .attributes()
            .get("style")
            .map(AttributeValue::text)
    }

    pub(super) fn attribute_keys(tree: &VirtualTree, node: inca_node::NodeId) -> Vec<&str> {
        tree.get(node)
            .unwrap()
            .attributes()
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn a_clone_keeps_the_declaration_order_and_the_style_entry() {
        let mut tree = VirtualTree::new();
        let original = node(&mut tree, "div");
        set_style_text(&mut tree, original, "height: 2px; width: 1px").unwrap();

        let made = tree.clone_subtree(original, false).unwrap();

        let copy = tree.get(made[0].1).unwrap();
        assert_eq!(
            copy.attributes().get("style"),
            Some(&AttributeValue::String("height: 2px; width: 1px;".into()))
        );
        assert_eq!(css_text(copy), "height: 2px; width: 1px;");
    }

    #[test]
    fn remove_attribute_leaves_the_rest_of_the_node_unchanged() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let node = tree.create_node("span").unwrap();
        tree.append_child(parent, node).unwrap();
        tree.set_attribute(node, "label", "a").unwrap();
        set_style(&mut tree, node, "label", "b").unwrap();

        tree.remove_attribute(node, "label").unwrap();

        let got = tree.get(node).unwrap();
        assert_eq!(got.local_name(), "span");
        assert_eq!(got.parent(), Some(parent));
        assert_eq!(
            got.style_props().get("label"),
            Some(&AttributeValue::String("b".into()))
        );
    }

    #[test]
    fn is_equal_compares_the_derived_style_text() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        set_style(&mut tree, first, "text_color", "#f00").unwrap();
        set_style(&mut tree, second, "text_color", "rgb(255, 0, 0)").unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(true));
    }

    #[test]
    fn is_equal_tells_another_style_prop_apart() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        set_style(&mut tree, second, "width", 5.0).unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(false));
    }

    #[test]
    fn is_equal_tells_another_declaration_order_apart() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        set_style_text(&mut tree, first, "width: 1px; height: 2px").unwrap();
        set_style_text(&mut tree, second, "height: 2px; width: 1px").unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(false));
    }

    #[test]
    fn is_equal_tells_an_empty_style_attribute_from_none() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        set_style_text(&mut tree, first, "").unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(false));
    }

    #[test]
    fn a_node_without_style_writes_no_style_attribute() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        set_style(&mut tree, div, "width", "").unwrap();

        assert_eq!(outer(&tree, div), "<div></div>");
    }

    #[test]
    fn the_style_attribute_keeps_the_position_of_the_entry() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        for key in ["a", "style", "b"] {
            tree.set_attribute(div, key, "1").unwrap();
        }
        set_style(&mut tree, div, "width", "5px").unwrap();

        assert_eq!(
            outer(&tree, div),
            "<div a=\"1\" style=\"width: 5px;\" b=\"1\"></div>"
        );
    }

    #[test]
    fn declarations_are_written_in_the_order_they_were_set() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        set_style(&mut tree, div, "width", 5.0).unwrap();
        set_style(&mut tree, div, "text_color", "#f00").unwrap();
        set_style(&mut tree, div, "corner_radius", 2.0).unwrap();

        assert_eq!(
            outer(&tree, div),
            "<div style=\"width: 5px; color: rgb(255, 0, 0); border-radius: 2px;\"></div>"
        );
    }

    #[test]
    fn set_style_of_an_unknown_id_errors() {
        let mut tree = VirtualTree::new();

        assert_eq!(
            set_style(&mut tree, 999, "k", "v").unwrap_err(),
            TreeError::NodeNotFound(999)
        );
    }

    #[test]
    fn remove_style_of_an_unknown_id_errors() {
        let mut tree = VirtualTree::new();

        assert_eq!(
            remove_style(&mut tree, 999, "k").unwrap_err(),
            TreeError::NodeNotFound(999)
        );
    }
}
