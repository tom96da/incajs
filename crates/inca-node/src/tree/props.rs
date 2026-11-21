// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The attributes and style props of a node.

use super::{AttributeValue, NodeId, TreeError, VirtualTree};

impl VirtualTree {
    /// Sets (inserting or overwriting) one attribute prop. The key `style` is stored as given and
    /// leaves the style props alone; `set_style_text` writes both.
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
        let node = self.node_mut(node_id)?;
        node.aria_refs.remove(key.as_str());
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
        node.aria_refs.remove(key);
        node.attributes.shift_remove(key);
        Ok(())
    }
}

/// Whether `name` is a valid attribute name. A name is invalid when it is empty or holds NUL, ASCII
/// whitespace, `/`, `=` or `>`.
#[must_use]
pub fn valid_attribute_name(name: &str) -> bool {
    !name.is_empty()
        && !name
            .chars()
            .any(|c| c == '\0' || c.is_ascii_whitespace() || matches!(c, '/' | '=' | '>'))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use rstest::rstest;

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

    #[rstest]
    #[case::empty("", false)]
    #[case::nul("a\0b", false)]
    #[case::space("a b", false)]
    #[case::tab("a\tb", false)]
    #[case::newline("a\nb", false)]
    #[case::form_feed("a\x0cb", false)]
    #[case::carriage_return("a\rb", false)]
    #[case::slash("a/b", false)]
    #[case::equals("a=b", false)]
    #[case::greater_than("a>b", false)]
    #[case::vertical_tab("a\x0bb", true)]
    #[case::non_ascii_space("a\u{a0}b", true)]
    #[case::colon("a:b", true)]
    #[case::data("data-x", true)]
    fn valid_attribute_name_follows_the_name_rule(#[case] name: &str, #[case] valid: bool) {
        assert_eq!(valid_attribute_name(name), valid, "{name:?}");
    }
}
