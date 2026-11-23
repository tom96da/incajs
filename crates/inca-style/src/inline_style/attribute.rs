// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `style` attribute of a node: the text of its declarations, written as a whole or derived
//! again after each write.

use crate::spec::css_declaration;
use inca_node::tree::{NodeId, TreeError, VirtualNode, VirtualTree, css_name, style_key};

/// unknown key and a value without a text are no declaration.
pub(super) fn style_declarations(node: &VirtualNode) -> impl Iterator<Item = (&str, String)> {
    node.style_props()
        .iter()
        .filter_map(|(key, value)| Some((key.as_str(), css_declaration(key, value)?)))
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

/// Stores `css_text` as the `style` attribute of `node`: an existing entry keeps its place, and a
/// missing one is created last when `create` is set. A text node and a comment never get one.
pub(super) fn sync_style_entry(node: &mut VirtualNode, create: bool) {
    if node.is_character_data() {
        return;
    }
    if create || node.attributes.contains_key("style") {
        let text = css_text(node);
        node.attributes.insert("style".into(), text.into());
    }
}

/// Replaces the style props of `node_id` with the declarations of `text` and stores the
/// declarations as the `style` attribute, in serialized form. A repeated property takes the later
/// value and the later position. `!important` is read and dropped. Returns one line for each
/// declaration that is dropped.
///
/// # Errors
///
/// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
pub fn set_style_text(
    tree: &mut VirtualTree,
    node_id: NodeId,
    text: &str,
) -> Result<Vec<String>, TreeError> {
    let node = tree.node_mut(node_id)?;
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{
        attribute_keys, block, block_with_no_declaration_keys, entry, node, num, text,
    };
    use crate::{
        StyleCache, build_spec, remove_style, remove_style_property, set_style, set_style_property,
        style_length,
    };
    use inca_node::tree::AttributeValue;
    use inca_node::{HtmlScope, NodeId};

    fn outer(tree: &VirtualTree, id: NodeId) -> String {
        tree.serialize_html(id, HtmlScope::Outer).unwrap()
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

    #[test]
    fn css_text_skips_an_unknown_key_and_a_bool_value() {
        let (tree, id) = block_with_no_declaration_keys();

        assert_eq!(css_text(tree.get(id).unwrap()), "width: 5px;");
    }

    #[test]
    fn set_style_text_replaces_the_old_props() {
        let (mut tree, id) = block(&[("height", text("9px"))]);

        set_style_text(&mut tree, id, "width: 1px").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[test]
    fn set_style_text_returns_a_line_for_each_dropped_declaration() {
        let (mut tree, id) = block(&[]);

        let lines = set_style_text(&mut tree, id, "bogus: 1; width: 2px").unwrap();

        assert_eq!(lines, ["ignoring unknown style key `bogus`"]);
    }

    #[test]
    fn set_style_text_reads_important_as_normal() {
        let (mut tree, id) = block(&[]);

        let lines = set_style_text(&mut tree, id, "width: 3px !important").unwrap();

        assert!(lines.is_empty());
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 3px;");
    }

    #[test]
    fn a_repeated_property_takes_the_later_value_and_position() {
        let (mut tree, id) = block(&[]);

        set_style_text(&mut tree, id, "width:1px;height:2px;width:3px").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "height: 2px; width: 3px;");
    }

    #[test]
    fn set_style_text_of_the_empty_text_creates_the_empty_entry() {
        let (mut tree, id) = block(&[]);

        set_style_text(&mut tree, id, "").unwrap();

        assert_eq!(entry(&tree, id), Some(String::new()));
    }

    #[rstest]
    #[case::infinity("opacity: Infinity")]
    #[case::nan("opacity: NaN")]
    fn set_style_text_drops_a_non_finite_opacity(#[case] written: &str) {
        let (mut tree, id) = block(&[]);

        let lines = set_style_text(&mut tree, id, written).unwrap();

        assert_eq!(
            lines,
            ["ignoring unsupported value for style key `opacity`"]
        );
        assert_eq!(css_text(tree.get(id).unwrap()), "");
        assert_eq!(style_length(tree.get(id).unwrap()), 0);
        assert_eq!(entry(&tree, id).as_deref(), Some(""));
    }

    #[test]
    fn set_style_text_of_a_percentage_opacity_stores_one_declaration() {
        let (mut tree, id) = block(&[]);

        let lines = set_style_text(&mut tree, id, "opacity: 50%").unwrap();

        assert!(lines.is_empty(), "{lines:?}");
        assert_eq!(style_length(tree.get(id).unwrap()), 1);
        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .opacity,
            Some(0.5)
        );
    }

    #[test]
    fn set_style_text_of_an_overflowing_opacity_stores_the_largest_value() {
        let (mut tree, id) = block(&[]);

        let lines = set_style_text(&mut tree, id, "opacity: 1e999").unwrap();

        assert!(lines.is_empty(), "{lines:?}");
        assert_eq!(
            css_text(tree.get(id).unwrap()),
            "opacity: 3.4028234663852886e+38;"
        );
        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .opacity,
            Some(1.0)
        );
    }

    #[rstest]
    #[case::set_style(|t: &mut VirtualTree, n: NodeId| set_style(t, n, "gap", "3px").unwrap())]
    #[case::remove_style(|t: &mut VirtualTree, n: NodeId| remove_style(t, n, "width").unwrap())]
    #[case::set_style_property(|t: &mut VirtualTree, n: NodeId| {
        set_style_property(t, n, "gap", "3px").unwrap();
    })]
    #[case::remove_style_property(|t: &mut VirtualTree, n: NodeId| {
        remove_style_property(t, n, "width").unwrap();
    })]
    #[case::set_style_text(|t: &mut VirtualTree, n: NodeId| {
        set_style_text(t, n, "gap: 3px").unwrap();
    })]
    fn a_style_write_derives_the_entry_again(#[case] write: fn(&mut VirtualTree, NodeId)) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        set_style(&mut tree, node, "width", "1px").unwrap();
        assert_eq!(entry(&tree, node).as_deref(), Some("width: 1px;"));

        write(&mut tree, node);

        let derived = css_text(tree.get(node).unwrap());
        assert_eq!(entry(&tree, node), Some(derived));
    }

    #[test]
    fn a_declaration_write_creates_the_entry_last() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "a", "1").unwrap();
        set_style(&mut tree, node, "width", "1px").unwrap();
        tree.set_attribute(node, "b", "1").unwrap();

        assert_eq!(attribute_keys(&tree, node), ["a", "style", "b"]);
    }

    #[rstest]
    #[case::comment("#comment")]
    #[case::text("#text")]
    fn a_comment_and_a_text_node_get_no_entry(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node(tag).unwrap();

        set_style(&mut tree, node, "width", "1px").unwrap();
        set_style_text(&mut tree, node, "width: 2px").unwrap();

        assert_eq!(entry(&tree, node), None);
    }

    #[test]
    fn set_attribute_style_leaves_the_props() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        set_style(&mut tree, node, "gap", "2px").unwrap();

        tree.set_attribute(node, "style", "WIDTH:1px").unwrap();

        assert_eq!(
            tree.get(node).unwrap().style_props().get("gap"),
            Some(&text("2px"))
        );
    }

    #[test]
    fn set_style_text_stores_the_derived_text() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        set_style_text(&mut tree, node, "WIDTH:1px;bogus:1").unwrap();

        assert_eq!(entry(&tree, node).as_deref(), Some("width: 1px;"));
    }

    #[test]
    fn set_style_text_applies_the_declarations_as_props() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        set_style_text(&mut tree, node, "width:1px").unwrap();

        assert_eq!(
            tree.get(node).unwrap().style_props().get("width"),
            Some(&text("1px"))
        );
    }

    #[test]
    fn set_style_text_stores_the_entry_at_the_position_of_the_write() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_attribute(node, "a", "1").unwrap();
        set_style_text(&mut tree, node, "width:1px").unwrap();
        tree.set_attribute(node, "b", "1").unwrap();
        set_style_text(&mut tree, node, "width:2px").unwrap();

        assert_eq!(attribute_keys(&tree, node), ["a", "style", "b"]);
    }

    #[test]
    fn remove_attribute_style_clears_the_props_and_the_entry() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        set_style_text(&mut tree, node, "width:1px").unwrap();

        tree.remove_attribute(node, "style").unwrap();

        let got = tree.get(node).unwrap();
        assert!(got.style_props().is_empty());
        assert!(got.attributes().is_empty());
    }
}
