// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The attributes and style props of a node.

use super::{AttributeValue, NodeId, TreeError, VirtualTree};
use crate::spec::style_key;

impl VirtualTree {
    /// Sets (inserting or overwriting) one attribute prop.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn set_attribute(
        &mut self,
        node_id: NodeId,
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        node.attributes.insert(key.into(), value.into());
        Ok(())
    }

    /// Removes one attribute prop. Removing a key that isn't set is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn remove_attribute(&mut self, node_id: NodeId, key: &str) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        node.attributes.shift_remove(key);
        Ok(())
    }

    /// Sets (inserting or overwriting) one style prop. `key` may be
    /// `camelCase`, `kebab-case` or `snake_case`, a host name or its CSS name,
    /// and all spellings of one prop share one slot.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn set_style(
        &mut self,
        node_id: NodeId,
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        let key: String = key.into();
        let stored = style_key(&key).into_owned();
        node.style_props.insert(stored, value.into());
        Ok(())
    }

    /// Applies the declarations of the text of a `style` attribute as style props, in order, and
    /// returns one line for each declaration that is dropped. The text is not stored as an
    /// attribute.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node and the text holds a valid
    /// declaration.
    pub(crate) fn set_style_attribute(
        &mut self,
        node_id: NodeId,
        text: &str,
    ) -> Result<Vec<String>, TreeError> {
        let (declarations, warnings) = crate::stylesheet::declarations(text);
        for (key, value) in declarations {
            self.set_style(node_id, key, value)?;
        }
        Ok(warnings)
    }

    /// Removes one style prop, so the property renders as if never set.
    /// `key` may be `camelCase`, `kebab-case` or `snake_case`, a host name or
    /// its CSS name. Removing a key that isn't set is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn remove_style(&mut self, node_id: NodeId, key: &str) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        node.style_props.shift_remove(&*style_key(key));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::spec::style_key;
    use rstest::rstest;
    use std::borrow::Cow;

    #[test]
    fn set_attribute_replaces_an_existing_value() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("input").unwrap();

        tree.set_attribute(node, "value", "first").unwrap();
        tree.set_attribute(node, "value", "second").unwrap();

        assert_eq!(
            tree.get(node).unwrap().attributes().get("value"),
            Some(&AttributeValue::String("second".into()))
        );
    }

    fn attribute_keys(tree: &VirtualTree, node: NodeId) -> Vec<&str> {
        tree.get(node)
            .unwrap()
            .attributes()
            .keys()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn setting_an_existing_attribute_keeps_its_position() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        for key in ["a", "b", "c", "a"] {
            tree.set_attribute(node, key, key).unwrap();
        }

        assert_eq!(attribute_keys(&tree, node), ["a", "b", "c"]);
    }

    #[test]
    fn removing_an_attribute_keeps_the_others_in_order() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        for key in ["a", "b", "c"] {
            tree.set_attribute(node, key, key).unwrap();
        }
        tree.remove_attribute(node, "a").unwrap();

        assert_eq!(attribute_keys(&tree, node), ["b", "c"]);
    }

    #[test]
    fn remove_attribute_unsets_only_that_key() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "label", "a").unwrap();
        tree.set_attribute(node, "count", 3.0).unwrap();

        tree.remove_attribute(node, "label").unwrap();

        let attrs = tree.get(node).unwrap().attributes();
        assert!(attrs.get("label").is_none());
        assert_eq!(attrs.get("count"), Some(&AttributeValue::Number(3.0)));
    }

    #[test]
    fn remove_attribute_of_an_unset_key_succeeds() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "label", "a").unwrap();

        tree.remove_attribute(node, "never_set").unwrap();
        tree.remove_attribute(node, "label").unwrap();
        tree.remove_attribute(node, "label").unwrap();

        assert!(tree.get(node).unwrap().attributes().is_empty());
    }

    #[test]
    fn remove_attribute_on_an_unknown_node_errors() {
        let mut tree = VirtualTree::new();
        assert_eq!(
            tree.remove_attribute(999, "k").unwrap_err(),
            TreeError::NodeNotFound(999)
        );
    }

    #[test]
    fn remove_attribute_leaves_the_rest_of_the_node_unchanged() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let node = tree.create_node("span").unwrap();
        tree.append_child(parent, node).unwrap();
        tree.set_attribute(node, "label", "a").unwrap();
        tree.set_style(node, "label", "b").unwrap();

        tree.remove_attribute(node, "label").unwrap();

        let got = tree.get(node).unwrap();
        assert_eq!(got.tag_name(), "span");
        assert_eq!(got.parent(), Some(parent));
        assert_eq!(
            got.style_props().get("label"),
            Some(&AttributeValue::String("b".into()))
        );
    }

    #[test]
    fn remove_style_unsets_only_that_key() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_style(node, "gap", 8.0).unwrap();
        tree.set_style(node, "width", 10.0).unwrap();

        tree.remove_style(node, "gap").unwrap();
        tree.remove_style(node, "never_set").unwrap();

        let styles = tree.get(node).unwrap().style_props();
        assert!(styles.get("gap").is_none());
        assert!(styles.get("width").is_some());
    }

    #[test]
    fn a_style_key_set_again_keeps_its_position() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "width", "1px").unwrap();
        tree.set_style(id, "height", "2px").unwrap();

        tree.set_style(id, "width", "3px").unwrap();

        let keys: Vec<&str> = tree
            .get(id)
            .unwrap()
            .style_props()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["width", "height"]);
    }

    #[test]
    fn a_removed_style_key_set_again_goes_last() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "width", "1px").unwrap();
        tree.set_style(id, "height", "2px").unwrap();
        tree.set_style(id, "gap", "3px").unwrap();

        tree.remove_style(id, "width").unwrap();
        tree.set_style(id, "width", "4px").unwrap();

        let keys: Vec<&str> = tree
            .get(id)
            .unwrap()
            .style_props()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["height", "gap", "width"]);
    }

    #[test]
    fn set_style_is_independent_of_attributes() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        tree.set_style(node, "gap", "red").unwrap();

        let got = tree.get(node).unwrap();
        assert_eq!(
            got.style_props().get("gap"),
            Some(&AttributeValue::String("red".into()))
        );
        assert!(got.attributes().get("gap").is_none());
    }

    #[rstest]
    #[case::camel("flexDirection")]
    #[case::kebab("flex-direction")]
    #[case::snake("flex_direction")]
    fn a_style_key_reads_back_in_snake_case_whatever_its_spelling(#[case] key: &str) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, key, "column").unwrap();
        let props = tree.get(id).unwrap().style_props();
        assert_eq!(props.len(), 1, "{key}");
        assert_eq!(props.get("flex_direction"), Some(&"column".into()), "{key}");
    }

    #[test]
    fn a_style_key_in_another_spelling_overwrites_the_value() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "flexDirection", "row").unwrap();
        tree.set_style(id, "flex-direction", "column").unwrap();
        let props = tree.get(id).unwrap().style_props();
        assert_eq!(props.len(), 1);
        assert_eq!(props.get("flex_direction"), Some(&"column".into()));
    }

    #[test]
    fn a_style_key_in_another_spelling_removes_the_value() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "flex-direction", "column").unwrap();

        tree.remove_style(id, "flexDirection").unwrap();
        assert!(tree.get(id).unwrap().style_props().is_empty());
    }

    #[rstest]
    #[case::host_key("flex_direction")]
    #[case::css_name("color")]
    #[case::css_name_snake("font_size")]
    #[case::css_name_camel("backgroundColor")]
    fn style_key_borrows_a_key_that_needs_no_new_text(#[case] key: &str) {
        assert!(matches!(style_key(key), Cow::Borrowed(_)), "{key}");
    }

    #[rstest]
    #[case::camel("flexDirection", "flex_direction")]
    #[case::kebab("flex-direction", "flex_direction")]
    #[case::leading_capital("FlexDirection", "_flex_direction")]
    #[case::double_dash("--x", "__x")]
    #[case::dash_before_capital("flex-Direction", "flex__direction")]
    #[case::empty("", "")]
    #[case::multibyte_before_capital("é-Z", "é__z")]
    #[case::multibyte_only("幅", "幅")]
    #[case::css_color("color", "text_color")]
    #[case::css_background_color("backgroundColor", "background")]
    #[case::css_kebab("border-radius", "corner_radius")]
    #[case::css_snake("font_size", "text_size")]
    #[case::vendor_prefix("-webkit-x", "_webkit_x")]
    fn a_style_key_normalizes_to_snake_case_in_every_edge_case(
        #[case] key: &str,
        #[case] want: &str,
    ) {
        assert_eq!(style_key(key), want, "{key}");
    }
}
