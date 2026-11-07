// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;

use gpui::Point;

use super::parse::style_spec_from_props;
use crate::input::sanitized;
use crate::tree::{AttributeValue, NodeId, TEXT_TAG, VirtualNode, VirtualTree};

/// What kind of element a [`VirtualNode`](crate::tree::VirtualNode) maps to.
///
/// Three kinds exist, chosen by the `"#text"` and `"input"` tag names.
///
/// Text is always its own node with a stable id, never a bare string child
/// of a container. Every rendered text run is addressable by [`NodeId`].
#[derive(Debug, Clone, PartialEq)]
pub enum ElementTag {
    /// Any tag name other than `"#text"` and `"input"`: a generic styled box.
    Container,
    /// A `"#text"` tag: its `"value"` attribute (missing or non-string →
    /// empty). Its children are not rendered.
    Text(String),
    /// An `"input"` tag: one line of text the host holds per node, drawn
    /// over the box. `placeholder` is its `placeholder` attribute without CR
    /// and LF ([`crate::input::sanitized`]; missing or non-string → empty).
    /// `editable` is false with a `readonly` or `disabled` attribute. Its
    /// children are not rendered.
    Input { placeholder: String, editable: bool },
}

/// A length in pixels, or `"auto"`. `gpui`-independent mirror of the subset
/// of [`gpui::Length`] this layer supports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthSpec {
    Px(f64),
    Auto,
}

/// A color: `0xRRGGBB` and an alpha from `0` (transparent) to `255` (opaque).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorSpec {
    /// `0xRRGGBB`.
    pub rgb: u32,
    pub alpha: u8,
}

impl ColorSpec {
    /// An opaque color of `rgb`.
    #[must_use]
    pub const fn opaque(rgb: u32) -> Self {
        Self { rgb, alpha: 255 }
    }
}

/// Defines a keyword enum: its variants, `NAMES` and `from_name`. A variant
/// lists every name it takes, separated by `|`.
macro_rules! keywords {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$vmeta:meta])* $variant:ident = $first:literal $(| $more:literal)*,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)*
        }

        impl $name {
            /// Every name a style value of this kind takes, in declaration order.
            pub const NAMES: &[&str] = &[$($first, $($more,)*)*];

            /// The variant `name` stands for. Names compare without ASCII case.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                $(
                    if name.eq_ignore_ascii_case($first) $(|| name.eq_ignore_ascii_case($more))* {
                        return Some(Self::$variant);
                    }
                )*
                None
            }
        }
    };
}

keywords! {
    /// `gpui`-independent mirror of [`gpui::Display`]'s supported variants.
    pub enum DisplaySpec {
        Flex = "flex",
        Block = "block",
        Grid = "grid",
        None = "none",
    }
}

keywords! {
    /// `gpui`-independent mirror of [`gpui::FlexDirection`].
    pub enum FlexDirectionSpec {
        Row = "row",
        Column = "column",
        RowReverse = "row-reverse" | "row_reverse",
        ColumnReverse = "column-reverse" | "column_reverse",
    }
}

keywords! {
    /// `gpui`-independent mirror of the common subset of `gpui`'s `AlignItems`
    /// and `JustifyContent` (`= AlignContent`) enums: the variants both share
    /// and that this layer supports.
    pub enum AlignSpec {
        Start = "start" | "flex-start",
        End = "end" | "flex-end",
        Center = "center",
        Stretch = "stretch",
    }
}

keywords! {
    /// `gpui`-independent mirror of [`gpui::Overflow`]'s supported variants.
    /// `"auto"` names [`OverflowSpec::Scroll`].
    pub enum OverflowSpec {
        Visible = "visible",
        /// Clips without making a scroll container.
        Clip = "clip",
        Hidden = "hidden",
        Scroll = "scroll" | "auto",
    }
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
/// [`VirtualNode`](crate::tree::VirtualNode)'s `style_props`.
/// [`STYLE_KEYS`](crate::STYLE_KEYS) lists the keys it is read from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleSpec {
    pub display: Option<DisplaySpec>,
    pub flex_direction: Option<FlexDirectionSpec>,
    pub justify_content: Option<AlignSpec>,
    pub align_items: Option<AlignSpec>,
    /// Uniform gap (px, `>= 0`) applied to both rows and columns.
    pub gap: Option<f64>,
    /// Px `>= 0`, or auto.
    pub width: Option<LengthSpec>,
    /// Px `>= 0`, or auto.
    pub height: Option<LengthSpec>,
    /// Uniform border width (px, `>= 0`) applied to all four sides.
    pub border_width: Option<f64>,
    pub background: Option<ColorSpec>,
    pub border_color: Option<ColorSpec>,
    /// Uniform corner radius (px, `>= 0`) applied to all four corners.
    pub corner_radius: Option<f64>,
    /// Cascades to descendant text. Text leaves carry no style of their own.
    pub text_color: Option<ColorSpec>,
    /// Font size (px, `>= 0`). Cascades to descendant text.
    pub text_size: Option<f64>,
    pub overflow_x: Option<OverflowSpec>,
    pub overflow_y: Option<OverflowSpec>,
    /// Inner spacing (px), each side `>= 0`.
    pub padding: EdgesSpec<f64>,
    /// Outer spacing; each side a px value (negative allowed) or `"auto"`.
    pub margin: EdgesSpec<LengthSpec>,
    /// `>= 0`.
    pub flex_grow: Option<f64>,
    /// `>= 0`.
    pub flex_shrink: Option<f64>,
    /// `0.0..=1.0`; multiplies into descendants.
    pub opacity: Option<f64>,
    /// Px `>= 0`, or auto.
    pub min_width: Option<LengthSpec>,
    /// Px `>= 0`, or auto.
    pub min_height: Option<LengthSpec>,
    /// Px `>= 0`, or auto.
    pub max_width: Option<LengthSpec>,
    /// Px `>= 0`, or auto.
    pub max_height: Option<LengthSpec>,
}

impl StyleSpec {
    /// Whether either axis is `hidden` or scrolls, which makes the node a scroll container.
    pub(super) fn is_scroll_container(&self) -> bool {
        let scrollable = |axis| matches!(axis, Some(OverflowSpec::Hidden | OverflowSpec::Scroll));
        scrollable(self.overflow_x) || scrollable(self.overflow_y)
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

/// Whether script can scroll `id`: either overflow axis is `hidden`,
/// `scroll` or `auto`.
#[must_use]
pub fn is_scroll_container(tree: &VirtualTree, id: NodeId) -> bool {
    style_spec_from_props(&tree.computed_style(id)).is_scroll_container()
}

/// Whether the `hidden` attribute hides `node`: it is present in the Hidden
/// state (any value except `until-found`) and `computed` sets no `display`.
fn hidden_with(node: &VirtualNode, computed: &HashMap<String, AttributeValue>) -> bool {
    let until_found = matches!(
        node.attributes().get("hidden"),
        Some(AttributeValue::String(value)) if value.eq_ignore_ascii_case("until-found")
    );
    let display_set = computed
        .get("display")
        .is_some_and(|value| value.as_str() != Some(""));
    node.attributes().contains_key("hidden") && !until_found && !display_set
}

/// Whether the `hidden` attribute hides `id`: it is present in the Hidden
/// state (any value except `until-found`) and the style of the node sets no
/// `display`. `false` for an unknown id.
#[must_use]
pub fn hidden_by_attribute(tree: &VirtualTree, id: NodeId) -> bool {
    tree.get(id)
        .is_some_and(|node| hidden_with(node, &tree.computed_style(id)))
}

/// The axes along which a wheel step can scroll `id` now. `None` when the node
/// is unknown or not connected, or it or an ancestor has `display: none`, a hiding
/// `hidden` attribute or `inert`. Otherwise an axis is `true` when its
/// overflow is `scroll` or `auto`; a container with any other overflow gives
/// `Some((false, false))`.
#[must_use]
pub fn wheel_axes(tree: &VirtualTree, id: NodeId) -> Option<Point<bool>> {
    tree.get(id)?;
    if !tree.is_connected(id) {
        return None;
    }
    let out = tree.ancestors(id).any(|id| {
        let Some(node) = tree.get(id) else {
            return false;
        };
        hidden_by_attribute(tree, id)
            || node.attributes().contains_key("inert")
            || style_spec_from_props(&tree.computed_style(id)).display == Some(DisplaySpec::None)
    });
    if out {
        return None;
    }
    let style = style_spec_from_props(&tree.computed_style(id));
    let scrolls = |axis| axis == Some(OverflowSpec::Scroll);
    Some(Point {
        x: scrolls(style.overflow_x),
        y: scrolls(style.overflow_y),
    })
}

fn build_spec_inner(tree: &VirtualTree, root: NodeId, inert_above: bool) -> Option<ElementSpec> {
    let node = tree.get(root)?;
    // A `<style>` element holds rules and takes no part in the layout.
    if node.tag_name().eq_ignore_ascii_case("style") {
        return None;
    }
    let inert = inert_above || node.attributes().contains_key("inert");

    let tag = match node.tag_name() {
        TEXT_TAG => ElementTag::Text(node.data().to_owned()),
        "input" => {
            let attributes = node.attributes();
            ElementTag::Input {
                placeholder: attributes
                    .get("placeholder")
                    .and_then(AttributeValue::as_str)
                    // `shape_line` takes one line.
                    .map(sanitized)
                    .unwrap_or_default(),
                editable: !attributes.contains_key("readonly")
                    && !attributes.contains_key("disabled"),
            }
        }
        _ => ElementTag::Container,
    };

    let computed = tree.computed_style(root);
    let mut style = style_spec_from_props(&computed);
    if hidden_with(node, &computed) {
        style.display = Some(DisplaySpec::None);
    }
    let children = if matches!(tag, ElementTag::Text(_) | ElementTag::Input { .. }) {
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

    use rstest::rstest;

    use super::*;
    use crate::tests::Fixture;
    use crate::tree::VirtualTree;

    /// A node with `name` as its class under the root of `f`.
    fn classed(f: &mut Fixture, name: &str) -> NodeId {
        let node = f.add(f.root, "div", name);
        f.attr(node, "class", name);
        node
    }

    /// A chain `[grandparent, parent, node]` rooted at the grandparent.
    fn chain() -> (VirtualTree, [NodeId; 3]) {
        let mut tree = VirtualTree::new();
        let ids = ["div"; 3].map(|tag| tree.create_node(tag).unwrap());
        tree.set_root(ids[0]);
        tree.append_child(ids[0], ids[1]).unwrap();
        tree.append_child(ids[1], ids[2]).unwrap();
        (tree, ids)
    }

    /// `wheel_axes` with the axes as a tuple.
    fn wheel(tree: &VirtualTree, id: NodeId) -> Option<(bool, bool)> {
        wheel_axes(tree, id).map(|axes| (axes.x, axes.y))
    }

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

    #[rstest]
    #[case::plain(vec![], "")]
    #[case::placeholder(vec![AttributeValue::from("hi")], "hi")]
    #[case::placeholder_with_newline(vec![AttributeValue::from("a\nb")], "ab")]
    #[case::number_placeholder(vec![AttributeValue::from(3.0)], "")]
    fn an_input_spec_reads_its_placeholder(
        #[case] placeholder: Vec<AttributeValue>,
        #[case] want: &str,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("input").unwrap();
        for value in placeholder {
            tree.set_attribute(id, "placeholder", value).unwrap();
        }
        let want = ElementTag::Input {
            placeholder: want.to_owned(),
            editable: true,
        };
        assert_eq!(build_spec(&tree, id).unwrap().tag, want);
    }

    #[rstest]
    #[case::plain(&[], true)]
    #[case::readonly(&["readonly"], false)]
    #[case::disabled(&["disabled"], false)]
    fn an_input_spec_is_editable_without_readonly_or_disabled(
        #[case] flags: &[&str],
        #[case] want: bool,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("input").unwrap();
        for flag in flags {
            tree.set_attribute(id, *flag, true).unwrap();
        }
        let want = ElementTag::Input {
            placeholder: String::new(),
            editable: want,
        };
        assert_eq!(build_spec(&tree, id).unwrap().tag, want);
    }

    #[test]
    fn an_input_spec_has_no_children() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("input").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(id, child).unwrap();
        assert!(build_spec(&tree, id).unwrap().children.is_empty());
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

    #[rstest]
    #[case::scroll_y(Some("hidden"), Some("scroll"), (false, true))]
    #[case::scroll_x(Some("scroll"), Some("hidden"), (true, false))]
    #[case::hidden(Some("hidden"), Some("hidden"), (false, false))]
    fn wheel_axes_follow_the_scroll_overflow_of_each_axis(
        #[case] overflow_x: Option<&str>,
        #[case] overflow_y: Option<&str>,
        #[case] want: (bool, bool),
    ) {
        let (mut tree, [_, _, node]) = chain();
        for (key, value) in [("overflow_x", overflow_x), ("overflow_y", overflow_y)] {
            if let Some(value) = value {
                tree.set_style(node, key, value).unwrap();
            }
        }
        assert_eq!(wheel(&tree, node), Some(want));
    }

    enum Where {
        Own,
        Parent,
        Grandparent,
    }

    enum Hide {
        DisplayNone,
        HiddenAttribute,
        Inert,
    }

    #[rstest]
    #[case::own_display_none(Where::Own, Hide::DisplayNone)]
    #[case::own_hidden_attribute(Where::Own, Hide::HiddenAttribute)]
    #[case::own_inert(Where::Own, Hide::Inert)]
    #[case::parent_display_none(Where::Parent, Hide::DisplayNone)]
    #[case::parent_hidden_attribute(Where::Parent, Hide::HiddenAttribute)]
    #[case::parent_inert(Where::Parent, Hide::Inert)]
    #[case::grandparent_display_none(Where::Grandparent, Hide::DisplayNone)]
    fn wheel_axes_is_none_for_a_hidden_or_inert_node(#[case] at: Where, #[case] hide: Hide) {
        let (mut tree, [grandparent, parent, node]) = chain();
        tree.set_style(node, "overflow", "scroll").unwrap();
        let target = match at {
            Where::Own => node,
            Where::Parent => parent,
            Where::Grandparent => grandparent,
        };
        match hide {
            Hide::DisplayNone => tree.set_style(target, "display", "none").unwrap(),
            Hide::HiddenAttribute => tree.set_attribute(target, "hidden", true).unwrap(),
            Hide::Inert => tree.set_attribute(target, "inert", true).unwrap(),
        }
        assert_eq!(wheel(&tree, node), None);
    }

    #[test]
    fn wheel_axes_is_none_for_an_unknown_node() {
        assert_eq!(wheel(&VirtualTree::new(), 42), None);
    }

    #[test]
    fn wheel_axes_is_none_for_a_detached_node() {
        let (mut tree, [_, parent, node]) = chain();
        tree.set_style(node, "overflow", "scroll").unwrap();
        tree.remove_child(parent, node).unwrap();
        assert_eq!(wheel(&tree, node), None);
    }

    #[test]
    fn a_hidden_attribute_does_not_hide_a_node_whose_display_is_set() {
        let (mut tree, [_, parent, node]) = chain();
        tree.set_style(node, "overflow_x", "hidden").unwrap();
        tree.set_style(node, "overflow_y", "scroll").unwrap();
        tree.set_attribute(parent, "hidden", true).unwrap();
        tree.set_style(parent, "display", "flex").unwrap();
        assert_eq!(wheel(&tree, node), Some((false, true)));
    }

    #[test]
    fn the_hidden_attribute_hides_a_node_with_an_empty_display_and_no_sheet() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();
        tree.set_style(node, "display", "").unwrap();
        tree.set_attribute(node, "hidden", true).unwrap();

        assert!(hidden_by_attribute(&tree, node));
    }

    #[test]
    fn the_hidden_attribute_hides_a_node_with_an_empty_display_and_no_matching_rule() {
        let mut f = Fixture::new();
        f.sheet(".b { width: 1px }");
        let node = classed(&mut f, "a");
        f.tree.set_style(node, "display", "").unwrap();
        f.attr(node, "hidden", true);

        assert!(hidden_by_attribute(&f.tree, node));
    }

    #[test]
    fn a_style_element_builds_no_element() {
        let mut f = Fixture::new();
        let (style, _) = f.sheet(".a { width: 1px }");
        let other = classed(&mut f, "a");

        let spec = build_spec(&f.tree, f.root).unwrap();

        assert!(build_spec(&f.tree, style).is_none());
        assert_eq!(
            spec.children
                .iter()
                .map(|child| child.id)
                .collect::<Vec<_>>(),
            [other]
        );
    }

    #[test]
    fn a_rule_overflow_makes_a_scroll_container() {
        let mut f = Fixture::new();
        f.sheet(".s { overflow: scroll }");
        let node = classed(&mut f, "s");
        let plain = classed(&mut f, "plain");

        assert!(is_scroll_container(&f.tree, node));
        assert!(!is_scroll_container(&f.tree, plain));
    }

    #[test]
    fn a_rule_display_none_leaves_the_wheel_no_axes() {
        let mut f = Fixture::new();
        f.sheet(".s { overflow: scroll } .h { display: none }");
        let hidden = classed(&mut f, "h");
        let node = f.add(hidden, "div", "node");
        f.attr(node, "class", "s");

        assert_eq!(wheel(&f.tree, node), None);
    }

    #[test]
    fn a_rule_display_stops_the_hidden_attribute_from_hiding() {
        let mut f = Fixture::new();
        f.sheet(".s { display: flex }");
        let node = classed(&mut f, "s");
        f.attr(node, "hidden", true);

        assert!(!hidden_by_attribute(&f.tree, node));
        assert_eq!(
            build_spec(&f.tree, node).unwrap().style.display,
            Some(DisplaySpec::Flex)
        );
    }

    #[test]
    fn a_text_named_tag_is_a_generic_box() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("text").unwrap();
        tree.set_attribute(id, "value", "hello").unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Container);
    }

    #[test]
    fn a_text_node_renders_its_value_attribute() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("#text").unwrap();
        tree.set_attribute(id, "value", "hello").unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text("hello".into()));
    }

    #[test]
    fn a_text_node_counts_a_non_string_value_as_empty() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("#text").unwrap();
        tree.set_attribute(id, "value", 1.0).unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.tag, ElementTag::Text(String::new()));
    }

    #[test]
    fn a_text_node_without_a_value_renders_empty() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("#text").unwrap();

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
