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

    let is_text = node.tag_name() == "text";
    let tag = match node.tag_name() {
        "text" => ElementTag::Text(text_content(tree, root)),
        _ => ElementTag::Container,
    };

    let mut style = style_spec_from_props(node.style_props());
    if hidden_by_attribute(node) {
        style.display = Some(DisplaySpec::None);
    }
    let children = if is_text {
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
