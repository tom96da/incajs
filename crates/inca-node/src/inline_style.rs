// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The inline style of a node as a block of declarations.

use crate::spec::{STYLE_KEYS, css_declaration, css_name, style_key};
use crate::tree::VirtualNode;

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
}
