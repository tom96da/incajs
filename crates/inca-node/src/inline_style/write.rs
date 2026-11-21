// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writing the declarations of an inline style.

use super::read::{css_text, property_key, style_property_value};
use crate::spec::{css_declaration, style_warning, unknown_key_warning};
use crate::tree::{AttributeValue, NodeId, TreeError, VirtualNode, VirtualTree, style_key};

/// Stores `css_text` as the `style` attribute of `node`: an existing entry keeps its place, and a
/// missing one is created last when `create` is set. A text node and a comment never get one.
fn sync_style_entry(node: &mut VirtualNode, create: bool) {
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

/// Sets (inserting or overwriting) one style prop. `key` may be `camelCase`, `kebab-case` or
/// `snake_case`, a host name or its CSS name, and all spellings of one prop share one slot. The
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
/// `kebab-case` or `snake_case`, a host name or its CSS name. Removing a key that isn't set is a
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

/// Sets the CSS property `name` to `text`. An empty text removes the property. Returns the
/// warning line when `name` is unknown or the value is invalid, and then changes nothing.
///
/// # Errors
///
/// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
pub fn set_style_property(
    tree: &mut VirtualTree,
    node_id: NodeId,
    name: &str,
    text: &str,
) -> Result<Vec<String>, TreeError> {
    let node = tree.node_mut(node_id)?;
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

/// Removes the CSS property `name` and returns its former canonical text, empty when it had none.
///
/// # Errors
///
/// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
pub fn remove_style_property(
    tree: &mut VirtualTree,
    node_id: NodeId,
    name: &str,
) -> Result<String, TreeError> {
    let node = tree.node_mut(node_id)?;
    let former = style_property_value(node, name);
    if let Some(key) = property_key(name) {
        node.style_props.shift_remove(key);
        sync_style_entry(node, false);
    }
    Ok(former)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::build_spec;
    use crate::inline_style::tests::{attribute_keys, block, entry};
    use crate::style_length;
    use crate::tests::{num, text};
    use crate::tree::{AttributeValue, StyleCache, VirtualTree};
    use rstest::rstest;

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

    #[test]
    fn set_style_property_stores_a_valid_value() {
        let (mut tree, id) = block(&[]);

        let lines = set_style_property(&mut tree, id, "WIDTH", " 4px ").unwrap();

        assert!(lines.is_empty());
        assert_eq!(entry(&tree, id).as_deref(), Some("width: 4px;"));
    }

    #[test]
    fn set_style_property_of_an_existing_key_keeps_its_position() {
        let (mut tree, id) = block(&[("width", text("1px")), ("height", text("2px"))]);

        set_style_property(&mut tree, id, "width", "3px").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "width: 3px; height: 2px;");
    }

    #[test]
    fn set_style_property_of_an_invalid_value_warns_and_keeps_the_block() {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        let lines = set_style_property(&mut tree, id, "width", "bogus").unwrap();

        assert_eq!(lines, ["ignoring unsupported value for style key `width`"]);
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[rstest]
    #[case::unknown("foo")]
    #[case::underscore("flex_direction")]
    fn set_style_property_of_an_unknown_name_warns_and_keeps_the_block(#[case] name: &str) {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        let lines = set_style_property(&mut tree, id, name, "row").unwrap();

        assert_eq!(lines, [format!("ignoring unknown style key `{name}`")]);
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[rstest]
    #[case::infinity("Infinity")]
    #[case::nan("NaN")]
    fn set_style_property_of_a_non_finite_opacity_keeps_the_former_value(#[case] written: &str) {
        let (mut tree, id) = block(&[("opacity", num(0.5))]);

        let lines = set_style_property(&mut tree, id, "opacity", written).unwrap();

        assert_eq!(
            lines,
            ["ignoring unsupported value for style key `opacity`"]
        );
        assert_eq!(css_text(tree.get(id).unwrap()), "opacity: 0.5;");
        assert_eq!(entry(&tree, id).as_deref(), Some("opacity: 0.5;"));
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
    fn set_style_property_of_a_percentage_opacity_stores_its_fraction() {
        let (mut tree, id) = block(&[]);

        let lines = set_style_property(&mut tree, id, "opacity", "50%").unwrap();

        assert!(lines.is_empty(), "{lines:?}");
        assert_eq!(css_text(tree.get(id).unwrap()), "opacity: 0.5;");
        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .opacity,
            Some(0.5)
        );
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
    fn set_style_property_of_the_empty_text_removes_the_property() {
        let (mut tree, id) = block(&[("width", text("1px")), ("height", text("2px"))]);

        set_style_property(&mut tree, id, "width", "").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "height: 2px;");
    }

    #[test]
    fn set_style_property_of_whitespace_removes_the_property() {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        set_style_property(&mut tree, id, "width", "  ").unwrap();

        assert_eq!(css_text(tree.get(id).unwrap()), "");
    }

    #[test]
    fn set_style_property_of_an_unknown_name_and_the_empty_text_is_silent() {
        let (mut tree, id) = block(&[("width", text("1px"))]);

        let lines = set_style_property(&mut tree, id, "foo", "").unwrap();

        assert!(lines.is_empty());
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 1px;");
    }

    #[test]
    fn set_style_property_of_the_empty_text_creates_no_entry() {
        let (mut tree, id) = block(&[]);

        set_style_property(&mut tree, id, "width", "").unwrap();

        assert_eq!(entry(&tree, id), None);
    }

    #[test]
    fn set_style_property_of_an_invalid_value_creates_no_entry() {
        let (mut tree, id) = block(&[]);

        set_style_property(&mut tree, id, "width", "bogus").unwrap();

        assert_eq!(entry(&tree, id), None);
    }

    #[test]
    fn remove_style_property_returns_the_former_canonical_text() {
        let (mut tree, id) = block(&[("text_color", text("#f00")), ("width", num(2.0))]);

        let former = remove_style_property(&mut tree, id, "color").unwrap();

        assert_eq!(former, "rgb(255, 0, 0)");
        assert_eq!(css_text(tree.get(id).unwrap()), "width: 2px;");
    }

    #[test]
    fn remove_style_property_of_an_absent_property_returns_empty_and_creates_no_entry() {
        let (mut tree, id) = block(&[]);

        let former = remove_style_property(&mut tree, id, "width").unwrap();

        assert_eq!(former, "");
        assert_eq!(entry(&tree, id), None);
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
    fn set_attribute_style_stores_the_text_as_given() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        tree.set_attribute(node, "style", "WIDTH:1px").unwrap();

        assert_eq!(entry(&tree, node).as_deref(), Some("WIDTH:1px"));
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
