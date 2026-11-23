// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The declaration block by CSS property name: reading a property, the item at an index, the
//! length, setting and removing a property, the priority and the property names.

use std::iter::once;

use super::attribute::{style_declarations, sync_style_entry};
use crate::spec::{css_declaration, style_warning, unknown_key_warning};
use inca_node::tree::{
    AttributeValue, NodeId, STYLE_KEYS, TreeError, VirtualNode, VirtualTree, css_name, style_key,
};

/// The style key that the CSS property `name` stands for. The name is read without ASCII case; a
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

/// Whether `priority` is neither empty nor `important`. A `setProperty` with such a priority
/// ignores a non-empty value.
#[must_use]
pub fn priority_refuses_value(priority: &str) -> bool {
    !priority.is_empty() && !priority.eq_ignore_ascii_case("important")
}

/// The distinct camelCase and kebab-case spellings of every key and CSS alias, each with the CSS
/// name that reads and writes it.
#[must_use]
pub fn property_names() -> Vec<(String, String)> {
    let mut names: Vec<(String, String)> = Vec::new();
    for entry in STYLE_KEYS {
        for raw in once(entry.key).chain(entry.aliases.iter().copied()) {
            let mut words = raw.split('_');
            let mut camel = words.next().unwrap_or_default().to_owned();
            for word in words {
                camel.extend(word.chars().take(1).flat_map(char::to_uppercase));
                camel.push_str(word.get(1..).unwrap_or_default());
            }
            let css = raw.replace('_', "-");
            for name in [css.clone(), camel] {
                if names.iter().all(|(known, _)| *known != name) {
                    names.push((name, css.clone()));
                }
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{block, block_with_no_declaration_keys, entry, num, text};
    use crate::{StyleCache, build_spec, css_text};

    #[rstest::rstest]
    #[case::kebab("corner-radius")]
    #[case::camel("cornerRadius")]
    fn property_names_lists_a_spelling_of_a_key(#[case] spelling: &str) {
        let names = property_names();

        assert!(names.contains(&(spelling.into(), "corner-radius".into())));
    }

    #[test]
    fn property_names_lists_each_spelling_once() {
        let names = property_names();

        for (index, (name, _)) in names.iter().enumerate() {
            assert!(names[..index].iter().all(|(known, _)| known != name));
        }
    }

    #[rstest::rstest]
    #[case::empty("", false)]
    #[case::important("important", false)]
    #[case::uppercase_important("IMPORTANT", false)]
    #[case::other("urgent", true)]
    fn a_priority_other_than_empty_or_important_refuses_a_value(
        #[case] priority: &str,
        #[case] refused: bool,
    ) {
        assert_eq!(priority_refuses_value(priority), refused);
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
}
