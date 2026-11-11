// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The inline style of a node as a block of declarations.

use crate::spec::{
    STYLE_KEYS, css_declaration, css_name, style_key, style_warning, unknown_key_warning,
};
use crate::tree::{
    AttributeValue, COMMENT_TAG, NodeId, TEXT_TAG, TreeError, VirtualNode, VirtualTree,
};

/// The style props of `node` that have a canonical text, in prop order, with that text. A stored
/// unknown key and a value without a text are no declaration.
fn style_declarations(node: &VirtualNode) -> impl Iterator<Item = (&str, String)> {
    node.style_props()
        .iter()
        .filter_map(|(key, value)| Some((key.as_str(), css_declaration(key, value)?)))
}

/// The host key that the CSS property `name` stands for. The name is read without ASCII case; a
/// name with `_` and a camelCase name stand for no key.
fn property_key(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    if name.contains('_') {
        return None;
    }
    let key = style_key(&name);
    STYLE_KEYS
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.key)
}

/// The declarations of `node` as `name: value;` joined by one space, in prop order. Empty when the
/// node has none.
#[must_use]
pub fn css_text(node: &VirtualNode) -> String {
    style_declarations(node)
        .map(|(key, text)| format!("{}: {text};", css_name(key)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The canonical text of the declaration of the CSS property `name`. Empty when `name` is unknown
/// or the node has no such declaration.
#[must_use]
pub fn style_property_value(node: &VirtualNode, name: &str) -> String {
    let Some(key) = property_key(name) else {
        return String::new();
    };
    style_declarations(node)
        .find_map(|(declared, text)| (declared == key).then_some(text))
        .unwrap_or_default()
}

/// The CSS name of the declaration at `index`, `None` past the last one.
#[must_use]
pub fn style_item(node: &VirtualNode, index: usize) -> Option<String> {
    style_declarations(node)
        .nth(index)
        .map(|(key, _)| css_name(key))
}

/// The number of declarations of `node`.
#[must_use]
pub fn style_length(node: &VirtualNode) -> usize {
    style_declarations(node).count()
}

/// Stores `css_text` as the `style` attribute of `node`: an existing entry keeps its place, and a
/// missing one is created last when `create` is set. A text node and a comment never get one.
pub(crate) fn sync_style_entry(node: &mut VirtualNode, create: bool) {
    if matches!(node.tag_name(), TEXT_TAG | COMMENT_TAG) {
        return;
    }
    if create || node.attributes.contains_key("style") {
        let text = css_text(node);
        node.attributes.insert("style".into(), text.into());
    }
}

impl VirtualTree {
    /// Replaces the style props with the declarations of `text` and stores the declarations as the
    /// `style` attribute, in serialized form. A repeated property takes the later value and the
    /// later position. `!important` is read and dropped. Returns one line for each declaration
    /// that is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn set_style_text(
        &mut self,
        node_id: NodeId,
        text: &str,
    ) -> Result<Vec<String>, TreeError> {
        let node = self.node_mut(node_id)?;
        let (declarations, warnings) = crate::stylesheet::declarations(text);
        node.style_props.clear();
        for (key, value) in declarations {
            let key = style_key(&key).into_owned();
            node.style_props.shift_remove(&key);
            node.style_props.insert(key, value);
        }
        sync_style_entry(node, true);
        Ok(warnings)
    }

    /// Sets the CSS property `name` to `text`. An empty text removes the property. Returns the
    /// warning line when `name` is unknown or the value is invalid, and then changes nothing.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn set_style_property(
        &mut self,
        node_id: NodeId,
        name: &str,
        text: &str,
    ) -> Result<Vec<String>, TreeError> {
        let node = self.node_mut(node_id)?;
        let text = text.trim_ascii();
        let key = property_key(name);
        if text.is_empty() {
            if let Some(key) = key {
                node.style_props.shift_remove(key);
                sync_style_entry(node, false);
            }
            return Ok(Vec::new());
        }
        let Some(key) = key else {
            return Ok(vec![unknown_key_warning(name)]);
        };
        let value = AttributeValue::String(text.to_owned());
        if let Some(line) = style_warning(&name.to_ascii_lowercase(), &value) {
            return Ok(vec![line]);
        }
        let declared = css_declaration(key, &value).is_some();
        node.style_props.insert(key.to_owned(), value);
        sync_style_entry(node, declared);
        Ok(Vec::new())
    }

    /// Removes the CSS property `name` and returns its former canonical text, empty when it had
    /// none.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn remove_style_property(
        &mut self,
        node_id: NodeId,
        name: &str,
    ) -> Result<String, TreeError> {
        let node = self.node_mut(node_id)?;
        let former = style_property_value(node, name);
        if let Some(key) = property_key(name) {
            node.style_props.shift_remove(key);
            sync_style_entry(node, false);
        }
        Ok(former)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{num, text};
    use crate::tree::{AttributeValue, VirtualTree};
    use rstest::rstest;

    fn block(props: &[(&str, AttributeValue)]) -> (VirtualTree, crate::NodeId) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        for (key, value) in props {
            tree.set_style(id, *key, value.clone()).unwrap();
        }
        (tree, id)
    }

    #[test]
    fn css_text_keeps_prop_order_and_writes_name_value_pairs() {
        let (tree, id) = block(&[
            ("width", num(5.0)),
            ("text_color", text("#f00")),
            ("corner_radius", num(2.0)),
        ]);

        assert_eq!(
            css_text(tree.get(id).unwrap()),
            "width: 5px; color: rgb(255, 0, 0); border-radius: 2px;"
        );
    }

    #[test]
    fn css_text_of_a_node_without_style_is_empty() {
        let (tree, id) = block(&[]);

        assert_eq!(css_text(tree.get(id).unwrap()), "");
    }

    fn block_with_no_declaration_keys() -> (VirtualTree, crate::NodeId) {
        block(&[
            ("not_a_key", num(1.0)),
            ("height", AttributeValue::Bool(true)),
            ("width", num(5.0)),
        ])
    }

    #[test]
    fn css_text_skips_an_unknown_key_and_a_bool_value() {
        let (tree, id) = block_with_no_declaration_keys();

        assert_eq!(css_text(tree.get(id).unwrap()), "width: 5px;");
    }

    #[test]
    fn style_length_skips_an_unknown_key_and_a_bool_value() {
        let (tree, id) = block_with_no_declaration_keys();

        assert_eq!(style_length(tree.get(id).unwrap()), 1);
    }

    #[test]
    fn style_item_skips_an_unknown_key_and_a_bool_value() {
        let (tree, id) = block_with_no_declaration_keys();
        let node = tree.get(id).unwrap();

        assert_eq!(style_item(node, 0).as_deref(), Some("width"));
        assert_eq!(style_item(node, 1), None);
    }

    #[test]
    fn style_item_writes_the_css_name_in_prop_order() {
        let (tree, id) = block(&[
            ("text_color", text("#fff")),
            ("flex_direction", text("row")),
        ]);
        let node = tree.get(id).unwrap();

        assert_eq!(style_item(node, 0).as_deref(), Some("color"));
        assert_eq!(style_item(node, 1).as_deref(), Some("flex-direction"));
    }

    #[rstest]
    #[case::css_name("border-radius", "2px")]
    #[case::alias_of_a_key("color", "rgb(255, 0, 0)")]
    #[case::upper_case("BORDER-RADIUS", "2px")]
    #[case::camel_case("borderRadius", "")]
    #[case::snake_case("corner_radius", "")]
    #[case::unset("height", "")]
    #[case::unknown("foo", "")]
    fn style_property_value_reads_the_canonical_text(#[case] name: &str, #[case] expected: &str) {
        let (tree, id) = block(&[("corner_radius", num(2.0)), ("text_color", text("#f00"))]);

        assert_eq!(style_property_value(tree.get(id).unwrap(), name), expected);
    }

    fn entry(tree: &VirtualTree, id: crate::NodeId) -> Option<String> {
        tree.get(id)
            .unwrap()
            .attributes()
            .get("style")
            .map(AttributeValue::text)
    }

    #[test]
    fn set_style_text_replaces_the_old_props() {
        let (mut tree, id) = block(&[("height", text("9px"))]);

        tree.set_style_text(id, "width: 1px").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[test]
    fn set_style_text_returns_a_line_for_each_dropped_declaration() {
        let (mut tree, id) = block(&[]);

        let lines = tree.set_style_text(id, "bogus: 1; width: 2px").unwrap();

        assert_eq!(lines, ["ignoring unknown style key `bogus`"]);
    }

    #[test]
    fn set_style_text_reads_important_as_normal() {
        let (mut tree, id) = block(&[]);

        let lines = tree.set_style_text(id, "width: 3px !important").unwrap();

        assert!(lines.is_empty());
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 3px;");
    }

    #[test]
    fn a_repeated_property_takes_the_later_value_and_position() {
        let (mut tree, id) = block(&[]);

        tree.set_style_text(id, "width:1px;height:2px;width:3px")
            .unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "height: 2px; width: 3px;");
    }

    #[test]
    fn set_style_text_of_the_empty_text_creates_the_empty_entry() {
        let (mut tree, id) = block(&[]);

        tree.set_style_text(id, "").unwrap();

        assert_eq!(entry(&tree, id), Some(String::new()));
    }

    #[test]
    fn set_style_property_stores_a_valid_value() {
        let (mut tree, id) = block(&[]);

        let lines = tree.set_style_property(id, "WIDTH", " 4px ").unwrap();

        assert!(lines.is_empty());
        assert_eq!(entry(&tree, id).as_deref(), Some("width: 4px;"));
    }

    #[test]
    fn set_style_property_of_an_existing_key_keeps_its_position() {
        let (mut tree, id) = block(&[("width", text("1px")), ("height", text("2px"))]);

        tree.set_style_property(id, "width", "3px").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "width: 3px; height: 2px;");
    }

    #[test]
    fn set_style_property_of_an_invalid_value_warns_and_keeps_the_block() {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        let lines = tree.set_style_property(id, "width", "bogus").unwrap();

        assert_eq!(lines, ["ignoring unsupported value for style key `width`"]);
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[rstest]
    #[case::unknown("foo")]
    #[case::underscore("flex_direction")]
    fn set_style_property_of_an_unknown_name_warns_and_keeps_the_block(#[case] name: &str) {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        let lines = tree.set_style_property(id, name, "row").unwrap();

        assert_eq!(lines, [format!("ignoring unknown style key `{name}`")]);
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[test]
    fn set_style_property_of_the_empty_text_removes_the_property() {
        let (mut tree, id) = block(&[("width", text("1px")), ("height", text("2px"))]);

        tree.set_style_property(id, "width", "").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "height: 2px;");
    }

    #[test]
    fn set_style_property_of_whitespace_removes_the_property() {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        tree.set_style_property(id, "width", "  ").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "");
    }

    #[test]
    fn set_style_property_of_an_unknown_name_and_the_empty_text_is_silent() {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        let lines = tree.set_style_property(id, "foo", "").unwrap();

        assert!(lines.is_empty());
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[test]
    fn set_style_property_of_the_empty_text_creates_no_entry() {
        let (mut tree, id) = block(&[]);

        tree.set_style_property(id, "width", "").unwrap();

        assert_eq!(entry(&tree, id), None);
    }

    #[test]
    fn set_style_property_of_an_invalid_value_creates_no_entry() {
        let (mut tree, id) = block(&[]);

        tree.set_style_property(id, "width", "bogus").unwrap();

        assert_eq!(entry(&tree, id), None);
    }

    #[test]
    fn remove_style_property_returns_the_former_canonical_text() {
        let (mut tree, id) = block(&[("text_color", text("#f00")), ("width", num(2.0))]);

        let former = tree.remove_style_property(id, "color").unwrap();

        assert_eq!(former, "rgb(255, 0, 0)");
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 2px;");
    }

    #[test]
    fn remove_style_property_of_an_absent_property_returns_empty_and_creates_no_entry() {
        let (mut tree, id) = block(&[]);

        let former = tree.remove_style_property(id, "width").unwrap();

        assert_eq!(former, "");
        assert_eq!(entry(&tree, id), None);
    }
}
