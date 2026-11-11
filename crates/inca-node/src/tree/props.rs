// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The attributes and style props of a node.

use super::{AttributeValue, NodeId, TreeError, VirtualTree};
use crate::inline_style::sync_style_entry;
use crate::spec::{css_declaration, style_key};

impl VirtualTree {
    /// Sets (inserting or overwriting) one attribute prop. The key `style` applies the declarations
    /// of the text as style props and stores the derived text; the warning lines are dropped.
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
        let key: String = key.into();
        let value = value.into();
        if key == "style" {
            self.set_style_text(node_id, &value.text())?;
            return Ok(());
        }
        let node = self.node_mut(node_id)?;
        node.attributes.insert(key, value);
        Ok(())
    }

    /// Removes one attribute prop. Removing a key that isn't set is a no-op. Removing `style`
    /// clears the style props.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn remove_attribute(&mut self, node_id: NodeId, key: &str) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        if key == "style" {
            node.style_props.clear();
        }
        node.attributes.shift_remove(key);
        Ok(())
    }

    /// Sets (inserting or overwriting) one style prop. `key` may be
    /// `camelCase`, `kebab-case` or `snake_case`, a host name or its CSS name,
    /// and all spellings of one prop share one slot. The `style` attribute is
    /// derived again, and created when the value has a declaration text.
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
        let value = value.into();
        let declared = css_declaration(&stored, &value).is_some();
        node.style_props.insert(stored, value);
        sync_style_entry(node, declared);
        Ok(())
    }

    /// Removes one style prop, so the property renders as if never set.
    /// `key` may be `camelCase`, `kebab-case` or `snake_case`, a host name or
    /// its CSS name. Removing a key that isn't set is a no-op. An existing
    /// `style` attribute is derived again.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn remove_style(&mut self, node_id: NodeId, key: &str) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        node.style_props.shift_remove(&*style_key(key));
        sync_style_entry(node, false);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::css_text;
    use crate::spec::style_key;
    use crate::tests::text;
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

    fn style_entry(tree: &VirtualTree, node: NodeId) -> Option<String> {
        tree.get(node)
            .unwrap()
            .attributes()
            .get("style")
            .map(AttributeValue::text)
    }

    #[test]
    fn removing_the_last_declaration_leaves_the_entry_empty() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_style(node, "width", "1px").unwrap();

        tree.remove_style(node, "width").unwrap();

        assert_eq!(style_entry(&tree, node), Some(String::new()));
    }

    #[rstest]
    #[case::set_style(|t: &mut VirtualTree, n: NodeId| t.set_style(n, "gap", "3px").unwrap())]
    #[case::remove_style(|t: &mut VirtualTree, n: NodeId| t.remove_style(n, "width").unwrap())]
    #[case::set_style_property(|t: &mut VirtualTree, n: NodeId| {
        t.set_style_property(n, "gap", "3px").unwrap();
    })]
    #[case::remove_style_property(|t: &mut VirtualTree, n: NodeId| {
        t.remove_style_property(n, "width").unwrap();
    })]
    #[case::set_style_text(|t: &mut VirtualTree, n: NodeId| {
        t.set_style_text(n, "gap: 3px").unwrap();
    })]
    fn a_style_write_derives_the_entry_again(#[case] write: fn(&mut VirtualTree, NodeId)) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_style(node, "width", "1px").unwrap();
        assert_eq!(style_entry(&tree, node).as_deref(), Some("width: 1px;"));

        write(&mut tree, node);

        let derived = css_text(tree.get(node).unwrap());
        assert_eq!(style_entry(&tree, node), Some(derived));
    }

    #[test]
    fn a_declaration_write_creates_the_entry_last() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "a", "1").unwrap();
        tree.set_style(node, "width", "1px").unwrap();
        tree.set_attribute(node, "b", "1").unwrap();

        assert_eq!(attribute_keys(&tree, node), ["a", "style", "b"]);
    }

    #[rstest]
    #[case::empty_string(text(""))]
    #[case::bool(AttributeValue::Bool(true))]
    fn set_style_of_a_value_without_text_creates_no_entry(#[case] value: AttributeValue) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        tree.set_style(node, "width", value).unwrap();

        assert_eq!(style_entry(&tree, node), None);
    }

    #[test]
    fn set_style_of_an_unknown_key_creates_no_entry() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        tree.set_style(node, "not_a_key", "1px").unwrap();

        assert_eq!(style_entry(&tree, node), None);
    }

    #[test]
    fn remove_style_creates_no_entry() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        tree.remove_style(node, "width").unwrap();

        assert_eq!(style_entry(&tree, node), None);
    }

    #[rstest]
    #[case::comment("#comment")]
    #[case::text("#text")]
    fn a_comment_and_a_text_node_get_no_entry(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node(tag).unwrap();

        tree.set_style(node, "width", "1px").unwrap();
        tree.set_attribute(node, "style", "width: 2px").unwrap();

        assert_eq!(style_entry(&tree, node), None);
    }

    #[test]
    fn set_attribute_style_stores_the_derived_text() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        tree.set_attribute(node, "style", "WIDTH:1px;bogus:1")
            .unwrap();

        assert_eq!(style_entry(&tree, node).as_deref(), Some("width: 1px;"));
    }

    #[test]
    fn set_attribute_style_applies_the_declarations_as_props() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        tree.set_attribute(node, "style", "width:1px").unwrap();

        assert_eq!(
            tree.get(node).unwrap().style_props().get("width"),
            Some(&text("1px"))
        );
    }

    #[test]
    fn set_attribute_style_stores_the_entry_at_the_position_of_the_write() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "a", "1").unwrap();
        tree.set_attribute(node, "style", "width:1px").unwrap();
        tree.set_attribute(node, "b", "1").unwrap();
        tree.set_attribute(node, "style", "width:2px").unwrap();

        assert_eq!(attribute_keys(&tree, node), ["a", "style", "b"]);
    }

    #[test]
    fn remove_attribute_style_clears_the_props_and_the_entry() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "style", "width:1px").unwrap();

        tree.remove_attribute(node, "style").unwrap();

        let got = tree.get(node).unwrap();
        assert!(got.style_props().is_empty());
        assert!(got.attributes().is_empty());
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
