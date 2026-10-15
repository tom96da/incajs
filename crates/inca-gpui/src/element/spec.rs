// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::parse::style_spec_from_props;
use crate::tree::{AttributeValue, NodeId, VirtualNode, VirtualTree};

/// What kind of element a [`VirtualNode`](crate::tree::VirtualNode) maps to.
///
/// Two kinds exist, chosen by the `"text"` tag name.
///
/// Text is deliberately never allowed as a bare string child mixed into a
/// container's children. It's always its own dedicated node with a
/// stable id. That keeps every rendered text run addressable by [`NodeId`],
/// which later work (text selection, hit-testing, per-run event handling)
/// will need. A container that could also hold ad-hoc string children
/// would make some rendered text invisible to that addressing.
#[derive(Debug, Clone, PartialEq)]
pub enum ElementTag {
    /// Any tag name other than `"text"`: a generic styled box.
    Container,
    /// A `"text"` tag: its `"value"` attribute (missing or non-string →
    /// empty) followed by its descendants' text in child order. Descendants
    /// are not rendered as separate elements.
    Text(String),
}

/// A length in pixels, or `"auto"`. `gpui`-independent mirror of the subset
/// of [`gpui::Length`] this layer supports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthSpec {
    Px(f64),
    Auto,
}

/// `gpui`-independent mirror of [`gpui::Display`]'s supported variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplaySpec {
    Flex,
    Block,
    Grid,
    None,
}

/// `gpui`-independent mirror of [`gpui::FlexDirection`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDirectionSpec {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// `gpui`-independent mirror of the common subset of `gpui`'s `AlignItems`
/// and `JustifyContent` (`= AlignContent`) enums: the variants both share
/// and that this layer supports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlignSpec {
    Start,
    End,
    Center,
    Stretch,
}

/// `gpui`-independent mirror of [`gpui::Overflow`]'s supported variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverflowSpec {
    Visible,
    Hidden,
    Scroll,
}

/// One value per side of a box. `None` leaves that side at its default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgesSpec<T> {
    pub top: Option<T>,
    pub right: Option<T>,
    pub bottom: Option<T>,
    pub left: Option<T>,
}

impl<T> Default for EdgesSpec<T> {
    fn default() -> Self {
        Self {
            top: None,
            right: None,
            bottom: None,
            left: None,
        }
    }
}

/// A plain-data, `gpui`-independent style description, built from a
/// [`VirtualNode`](crate::tree::VirtualNode)'s `style_props`. See
/// `style_spec_from_props`'s match arms for the exact recognized keys.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleSpec {
    pub display: Option<DisplaySpec>,
    pub flex_direction: Option<FlexDirectionSpec>,
    pub justify_content: Option<AlignSpec>,
    pub align_items: Option<AlignSpec>,
    /// Uniform gap (px) applied to both rows and columns.
    pub gap: Option<f64>,
    pub width: Option<LengthSpec>,
    pub height: Option<LengthSpec>,
    /// Uniform border width (px) applied to all four sides.
    pub border_width: Option<f64>,
    /// `0xRRGGBB`.
    pub background: Option<u32>,
    /// `0xRRGGBB`.
    pub border_color: Option<u32>,
    /// Uniform corner radius (px) applied to all four corners.
    pub corner_radius: Option<f64>,
    /// `0xRRGGBB`. Cascades to descendant text, like `gpui`'s own
    /// `.text_color()`. Text leaves carry no style of their own.
    pub text_color: Option<u32>,
    pub text_size: Option<f64>,
    pub overflow_x: Option<OverflowSpec>,
    pub overflow_y: Option<OverflowSpec>,
    /// Inner spacing (px), each side `>= 0`.
    pub padding: EdgesSpec<f64>,
    /// Outer spacing; each side a px number (negative allowed) or `"auto"`.
    pub margin: EdgesSpec<LengthSpec>,
    /// `>= 0`.
    pub flex_grow: Option<f64>,
    /// `>= 0`.
    pub flex_shrink: Option<f64>,
    /// `0.0..=1.0`; multiplies into descendants.
    pub opacity: Option<f64>,
    pub min_width: Option<LengthSpec>,
    pub min_height: Option<LengthSpec>,
    pub max_width: Option<LengthSpec>,
    pub max_height: Option<LengthSpec>,
}

impl StyleSpec {
    /// Whether either axis clips (`hidden`) or scrolls.
    pub(super) fn clips(&self) -> bool {
        let clips = |axis| matches!(axis, Some(OverflowSpec::Hidden | OverflowSpec::Scroll));
        clips(self.overflow_x) || clips(self.overflow_y)
    }
}

/// A `gpui`-independent, recursive description of one
/// [`VirtualNode`](crate::tree::VirtualNode) and its subtree.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementSpec {
    pub id: NodeId,
    pub tag: ElementTag,
    pub style: StyleSpec,
    /// The node or an ancestor has the `inert` attribute. The node reports
    /// no mouse target and no hover.
    pub inert: bool,
    pub children: Vec<ElementSpec>,
}

/// A node's `"value"` attribute followed by its descendants' text in child
/// order, whatever their tags. An unresolved child id is skipped.
fn text_content(tree: &VirtualTree, id: NodeId) -> String {
    let Some(node) = tree.get(id) else {
        return String::new();
    };
    let mut content = node
        .attributes()
        .get("value")
        .and_then(AttributeValue::as_str)
        .unwrap_or_default()
        .to_owned();
    for &child_id in node.children() {
        content.push_str(&text_content(tree, child_id));
    }
    content
}

/// Builds an [`ElementSpec`] for `root` and its whole subtree. `None` if `root`
/// doesn't resolve, matching [`VirtualTree::get`]'s convention. The render path
/// is fed by JS-supplied data, so it skips a child id that doesn't resolve.
#[must_use]
pub fn build_spec(tree: &VirtualTree, root: NodeId) -> Option<ElementSpec> {
    let mut inert = false;
    let mut cursor = tree.get(root)?.parent();
    while let Some(ancestor) = cursor.and_then(|id| tree.get(id)) {
        inert |= ancestor.attributes().contains_key("inert");
        cursor = ancestor.parent();
    }
    build_spec_inner(tree, root, inert)
}

/// Whether script can scroll `node`: either overflow axis is `hidden`,
/// `scroll` or `auto`.
#[must_use]
pub fn is_scroll_container(node: &VirtualNode) -> bool {
    style_spec_from_props(node.style_props()).clips()
}

/// Whether the `hidden` attribute hides `node`: it is present in the Hidden
/// state (any value except `until-found`) and the node sets no `display`.
#[must_use]
pub fn hidden_by_attribute(node: &VirtualNode) -> bool {
    let until_found = matches!(
        node.attributes().get("hidden"),
        Some(AttributeValue::String(value)) if value.eq_ignore_ascii_case("until-found")
    );
    node.attributes().contains_key("hidden")
        && !until_found
        && !node.style_props().contains_key("display")
}

fn build_spec_inner(tree: &VirtualTree, root: NodeId, inert_above: bool) -> Option<ElementSpec> {
    let node = tree.get(root)?;
    let inert = inert_above || node.attributes().contains_key("inert");

    let tag = match node.tag_name() {
        "text" => ElementTag::Text(text_content(tree, root)),
        _ => ElementTag::Container,
    };

    let mut style = style_spec_from_props(node.style_props());
    if hidden_by_attribute(node) {
        style.display = Some(DisplaySpec::None);
    }
    let children = if matches!(tag, ElementTag::Text(_)) {
        Vec::new()
    } else {
        node.children()
            .iter()
            .filter_map(|&child_id| build_spec_inner(tree, child_id, inert))
            .collect()
    };

    Some(ElementSpec {
        id: root,
        tag,
        style,
        inert,
        children,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tree::VirtualTree;

    #[test]
    fn unknown_root_returns_none() {
        let tree = VirtualTree::new();
        assert!(build_spec(&tree, 42).is_none());
    }

    #[test]
    fn non_text_tag_is_a_container() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Container);
    }

    #[test]
    fn the_hidden_attribute_makes_the_spec_display_none_unless_display_is_set() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        let display = |tree: &VirtualTree| build_spec(tree, id).unwrap().style.display;
        assert_eq!(display(&tree), None);
        for (value, want) in [
            ("", Some(DisplaySpec::None)),
            ("hidden", Some(DisplaySpec::None)),
            ("until-found", None),
            ("UNTIL-FOUND", None),
        ] {
            tree.set_attribute(id, "hidden", value).unwrap();
            assert_eq!(display(&tree), want, "{value:?}");
        }
        tree.set_attribute(id, "hidden", true).unwrap();
        assert_eq!(display(&tree), Some(DisplaySpec::None));
        tree.set_style(id, "display", "flex").unwrap();
        assert_eq!(display(&tree), Some(DisplaySpec::Flex));
        tree.remove_style(id, "display").unwrap();
        tree.remove_attribute(id, "hidden").unwrap();
        assert_eq!(display(&tree), None);
    }

    #[test]
    fn an_inert_node_and_its_subtree_are_inert() {
        let mut tree = VirtualTree::new();
        let [outer, inert, child, sibling] = ["div"; 4].map(|tag| tree.create_node(tag).unwrap());
        tree.append_child(outer, inert).unwrap();
        tree.append_child(inert, child).unwrap();
        tree.append_child(outer, sibling).unwrap();
        tree.set_attribute(inert, "inert", true).unwrap();
        let inerts = |root| {
            let spec = build_spec(&tree, root).unwrap();
            let mut seen = vec![(spec.id, spec.inert)];
            let mut stack = spec.children;
            while let Some(spec) = stack.pop() {
                seen.push((spec.id, spec.inert));
                stack.extend(spec.children);
            }
            seen.sort_unstable();
            seen
        };
        assert_eq!(
            inerts(outer),
            [
                (outer, false),
                (inert, true),
                (child, true),
                (sibling, false)
            ]
        );
        // A subtree built from the child alone still sees the inert ancestor.
        assert_eq!(inerts(child), [(child, true)]);
    }

    #[test]
    fn text_tag_uses_its_value_attribute() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("text").unwrap();
        tree.set_attribute(id, "value", "hello").unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text("hello".into()));
    }

    #[test]
    fn text_tag_puts_its_value_before_its_children_in_order() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("text").unwrap();
        tree.set_attribute(id, "value", "a").unwrap();
        for part in ["b", "c"] {
            let child = tree.create_node("text").unwrap();
            tree.set_attribute(child, "value", part).unwrap();
            tree.append_child(id, child).unwrap();
        }

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text("abc".into()));
    }

    #[test]
    fn text_tag_includes_text_nested_in_any_tag() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("text").unwrap();
        let wrapper = tree.create_node("div").unwrap();
        let leaf = tree.create_node("text").unwrap();
        tree.set_attribute(leaf, "value", "nested").unwrap();
        tree.append_child(wrapper, leaf).unwrap();
        tree.append_child(id, wrapper).unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text("nested".into()));
    }

    #[test]
    fn text_tag_counts_a_non_string_value_as_empty() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("text").unwrap();
        tree.set_attribute(id, "value", 1.0).unwrap();
        let child = tree.create_node("text").unwrap();
        tree.set_attribute(child, "value", "kept").unwrap();
        tree.append_child(id, child).unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text("kept".into()));
    }

    #[test]
    fn text_tag_missing_value_is_empty_content() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("text").unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text(String::new()));
    }

    #[test]
    fn children_are_built_in_append_order() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let a = tree.create_node("div").unwrap();
        let b = tree.create_node("div").unwrap();
        tree.append_child(parent, a).unwrap();
        tree.append_child(parent, b).unwrap();

        let spec = build_spec(&tree, parent).unwrap();
        let child_ids: Vec<NodeId> = spec.children.iter().map(|c| c.id).collect();
        assert_eq!(child_ids, &[a, b]);
    }
}
