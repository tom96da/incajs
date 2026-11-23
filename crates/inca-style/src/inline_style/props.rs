// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The style props of a node set and removed by style key, in any spelling of the key.

use super::attribute::sync_style_entry;
use crate::spec::css_declaration;
use inca_node::tree::{AttributeValue, NodeId, TreeError, VirtualTree, style_key};

/// Sets (inserting or overwriting) one style prop. `key` may be `camelCase`, `kebab-case` or
/// `snake_case`, a style key or its CSS name, and all spellings of one prop share one slot. The
/// `style` attribute is derived again, and created when the value has a declaration text.
///
/// # Errors
///
/// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
pub fn set_style(
    tree: &mut VirtualTree,
    node_id: NodeId,
    key: impl Into<String>,
    value: impl Into<AttributeValue>,
) -> Result<(), TreeError> {
    let node = tree.node_mut(node_id)?;
    let key: String = key.into();
    let stored = style_key(&key).into_owned();
    let value = value.into();
    let declared = css_declaration(&stored, &value).is_some();
    node.style_props.insert(stored, value);
    sync_style_entry(node, declared);
    Ok(())
}

/// Removes one style prop, so the property renders as if never set. `key` may be `camelCase`,
/// `kebab-case` or `snake_case`, a style key or its CSS name. Removing a key that isn't set is a
/// no-op. An existing `style` attribute is derived again.
///
/// # Errors
///
/// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
pub fn remove_style(tree: &mut VirtualTree, node_id: NodeId, key: &str) -> Result<(), TreeError> {
    let node = tree.node_mut(node_id)?;
    node.style_props.shift_remove(&*style_key(key));
    sync_style_entry(node, false);
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{block, entry, num, text};
    use crate::{
        StyleCache, build_spec, css_text, remove_style, set_style, set_style_text, style_length,
    };
    use inca_node::tree::{AttributeValue, TreeError, VirtualTree};

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

    #[rstest]
    #[case::infinity(f64::INFINITY)]
    #[case::negative_infinity(f64::NEG_INFINITY)]
    fn set_style_of_a_non_finite_opacity_has_no_declaration_and_renders_unset(#[case] value: f64) {
        let (tree, id) = block(&[("opacity", num(value))]);

        assert_eq!(css_text(tree.get(id).unwrap()), "");
        assert_eq!(style_length(tree.get(id).unwrap()), 0);
        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .opacity,
            None
        );
    }

    #[rstest]
    #[case::beyond_one(num(2.0), "opacity: 2;", 1.0)]
    #[case::negative(num(-1.0), "opacity: -1;", 0.0)]
    #[case::percent_beyond_one(text("150%"), "opacity: 1.5;", 1.0)]
    #[case::negative_percent(text("-50%"), "opacity: -0.5;", 0.0)]
    #[case::overflowing(text("1e999"), "opacity: 3.4028234663852886e+38;", 1.0)]
    #[case::negative_overflowing(text("-1e999"), "opacity: -3.4028234663852886e+38;", 0.0)]
    fn a_finite_opacity_outside_the_range_is_kept_and_rendered_clamped(
        #[case] value: AttributeValue,
        #[case] css: &str,
        #[case] rendered: f64,
    ) {
        let (mut tree, id) = block(&[("opacity", value)]);
        assert_eq!(css_text(tree.get(id).unwrap()), css);
        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .opacity,
            Some(rendered)
        );

        set_style_text(&mut tree, id, css).unwrap();
        assert_eq!(css_text(tree.get(id).unwrap()), css);
        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .opacity,
            Some(rendered)
        );
    }

    #[test]
    fn remove_style_unsets_only_that_key() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        set_style(&mut tree, node, "gap", 8.0).unwrap();
        set_style(&mut tree, node, "width", 10.0).unwrap();

        remove_style(&mut tree, node, "gap").unwrap();
        remove_style(&mut tree, node, "never_set").unwrap();

        let styles = tree.get(node).unwrap().style_props();
        assert!(styles.get("gap").is_none());
        assert!(styles.get("width").is_some());
    }

    #[test]
    fn a_style_key_set_again_keeps_its_position() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        set_style(&mut tree, id, "width", "1px").unwrap();
        set_style(&mut tree, id, "height", "2px").unwrap();

        set_style(&mut tree, id, "width", "3px").unwrap();

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
        set_style(&mut tree, id, "width", "1px").unwrap();
        set_style(&mut tree, id, "height", "2px").unwrap();
        set_style(&mut tree, id, "gap", "3px").unwrap();

        remove_style(&mut tree, id, "width").unwrap();
        set_style(&mut tree, id, "width", "4px").unwrap();

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
    fn removing_the_last_declaration_leaves_the_entry_empty() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        set_style(&mut tree, node, "width", "1px").unwrap();

        remove_style(&mut tree, node, "width").unwrap();

        assert_eq!(entry(&tree, node), Some(String::new()));
    }

    #[rstest]
    #[case::empty_string(text(""))]
    #[case::bool(AttributeValue::Bool(true))]
    fn set_style_of_a_value_without_text_creates_no_entry(#[case] value: AttributeValue) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        set_style(&mut tree, node, "width", value).unwrap();

        assert_eq!(entry(&tree, node), None);
    }

    #[test]
    fn set_style_of_an_unknown_key_creates_no_entry() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        set_style(&mut tree, node, "not_a_key", "1px").unwrap();

        assert_eq!(entry(&tree, node), None);
    }

    #[test]
    fn remove_style_creates_no_entry() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        remove_style(&mut tree, node, "width").unwrap();

        assert_eq!(entry(&tree, node), None);
    }

    #[test]
    fn set_style_is_independent_of_attributes() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        set_style(&mut tree, node, "gap", "red").unwrap();

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
        set_style(&mut tree, id, key, "column").unwrap();
        let props = tree.get(id).unwrap().style_props();
        assert_eq!(props.len(), 1, "{key}");
        assert_eq!(props.get("flex_direction"), Some(&"column".into()), "{key}");
    }

    #[test]
    fn a_style_key_in_another_spelling_overwrites_the_value() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        set_style(&mut tree, id, "flexDirection", "row").unwrap();
        set_style(&mut tree, id, "flex-direction", "column").unwrap();
        let props = tree.get(id).unwrap().style_props();
        assert_eq!(props.len(), 1);
        assert_eq!(props.get("flex_direction"), Some(&"column".into()));
    }

    #[test]
    fn a_style_key_in_another_spelling_removes_the_value() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        set_style(&mut tree, id, "flex-direction", "column").unwrap();

        remove_style(&mut tree, id, "flexDirection").unwrap();
        assert!(tree.get(id).unwrap().style_props().is_empty());
    }
}
