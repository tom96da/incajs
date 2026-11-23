// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scroll container rules and the axes a wheel step can scroll.

use super::parse::style_spec_from_props;
use super::{DisplaySpec, OverflowSpec, StyleSpec, hidden_by_attribute};
use crate::StyleCache;
use inca_node::tree::{NodeId, VirtualTree};

/// The axes a wheel step can scroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WheelAxes {
    /// Whether the horizontal axis scrolls.
    pub x: bool,
    /// Whether the vertical axis scrolls.
    pub y: bool,
}

/// The axes along which a wheel step can scroll `id` now. `None` when the node
/// is unknown or not connected, or it or an ancestor is a template or has `display: none`, a
/// hiding `hidden` attribute or `inert`. Otherwise an axis is `true` when its
/// overflow is `scroll` or `auto`; a container with any other overflow gives
/// `Some(WheelAxes { x: false, y: false })`.
#[must_use]
pub fn wheel_axes(tree: &VirtualTree, cache: &StyleCache, id: NodeId) -> Option<WheelAxes> {
    tree.get(id)?;
    if !tree.is_connected(id) {
        return None;
    }
    let out = tree.ancestors(id).any(|id| {
        let Some(node) = tree.get(id) else {
            return false;
        };
        node.is_template()
            || hidden_by_attribute(tree, cache, id)
            || node.attributes().contains_key("inert")
            || style_spec_from_props(&cache.computed_style(tree, id)).display
                == Some(DisplaySpec::None)
    });
    if out {
        return None;
    }
    Some(style_spec_from_props(&cache.computed_style(tree, id)).scroll_axes())
}

impl StyleSpec {
    /// Whether either axis is `hidden` or scrolls, which makes the node a scroll container.
    #[must_use]
    pub fn is_scroll_container(&self) -> bool {
        let scrollable = |axis| {
            matches!(
                axis,
                Some(OverflowSpec::Hidden | OverflowSpec::Scroll | OverflowSpec::Auto)
            )
        };
        scrollable(self.overflow_x) || scrollable(self.overflow_y)
    }

    /// The axes whose overflow is `scroll` or `auto`.
    #[must_use]
    pub fn scroll_axes(&self) -> WheelAxes {
        let scrolls = |axis| matches!(axis, Some(OverflowSpec::Scroll | OverflowSpec::Auto));
        WheelAxes {
            x: scrolls(self.overflow_x),
            y: scrolls(self.overflow_y),
        }
    }

    /// The axes whose overflow is `scroll`: the axes that reserve the width of a scrollbar.
    #[must_use]
    pub fn scroll_keyword_axes(&self) -> WheelAxes {
        WheelAxes {
            x: self.overflow_x == Some(OverflowSpec::Scroll),
            y: self.overflow_y == Some(OverflowSpec::Scroll),
        }
    }
}

/// Whether script can scroll `id`: either overflow axis is `hidden`,
/// `scroll` or `auto`.
#[must_use]
pub fn is_scroll_container(tree: &VirtualTree, cache: &StyleCache, id: NodeId) -> bool {
    style_spec_from_props(&cache.computed_style(tree, id)).is_scroll_container()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::set_style;
    use crate::tests::{Fixture, SVG, classed};

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
        wheel_axes(tree, &StyleCache::new(), id).map(|axes| (axes.x, axes.y))
    }

    /// A style with these overflow values.
    fn overflowing(x: OverflowSpec, y: OverflowSpec) -> StyleSpec {
        StyleSpec {
            overflow_x: Some(x),
            overflow_y: Some(y),
            ..StyleSpec::default()
        }
    }

    #[rstest]
    #[case::scroll_y(Some("hidden"), Some("scroll"), (false, true))]
    #[case::scroll_x(Some("scroll"), Some("hidden"), (true, false))]
    #[case::auto_y(Some("hidden"), Some("auto"), (false, true))]
    #[case::hidden(Some("hidden"), Some("hidden"), (false, false))]
    fn wheel_axes_follow_the_scroll_overflow_of_each_axis(
        #[case] overflow_x: Option<&str>,
        #[case] overflow_y: Option<&str>,
        #[case] want: (bool, bool),
    ) {
        let (mut tree, [_, _, node]) = chain();
        for (key, value) in [("overflow_x", overflow_x), ("overflow_y", overflow_y)] {
            if let Some(value) = value {
                set_style(&mut tree, node, key, value).unwrap();
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
        set_style(&mut tree, node, "overflow", "scroll").unwrap();
        let target = match at {
            Where::Own => node,
            Where::Parent => parent,
            Where::Grandparent => grandparent,
        };
        match hide {
            Hide::DisplayNone => set_style(&mut tree, target, "display", "none").unwrap(),
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
        set_style(&mut tree, node, "overflow", "scroll").unwrap();
        tree.remove_child(parent, node).unwrap();
        assert_eq!(wheel(&tree, node), None);
    }

    #[test]
    fn wheel_axes_is_not_none_under_an_svg_template() {
        let (mut tree, [_, parent, node]) = chain();
        set_style(&mut tree, node, "overflow", "scroll").unwrap();
        let template = tree.create_element_ns(Some(SVG), None, "template").unwrap();
        tree.append_child(parent, template).unwrap();
        tree.append_child(template, node).unwrap();

        assert_eq!(wheel(&tree, node), Some((true, true)));
    }

    #[test]
    fn wheel_axes_is_none_under_a_template() {
        let (mut tree, [_, parent, node]) = chain();
        set_style(&mut tree, node, "overflow", "scroll").unwrap();
        let template = tree.create_node("template").unwrap();
        tree.append_child(parent, template).unwrap();
        tree.append_child(template, node).unwrap();

        assert_eq!(wheel(&tree, node), None);
    }

    #[rstest]
    #[case::scroll(OverflowSpec::Scroll, true)]
    #[case::auto(OverflowSpec::Auto, true)]
    #[case::hidden(OverflowSpec::Hidden, false)]
    fn scroll_axes_follow_the_overflow_of_each_axis(
        #[case] overflow: OverflowSpec,
        #[case] want: bool,
    ) {
        let axes = overflowing(overflow, OverflowSpec::Visible).scroll_axes();
        assert_eq!((axes.x, axes.y), (want, false));
    }

    #[rstest]
    #[case::scroll(OverflowSpec::Scroll, true)]
    #[case::auto(OverflowSpec::Auto, false)]
    #[case::hidden(OverflowSpec::Hidden, false)]
    fn scroll_keyword_axes_hold_the_scroll_value_only(
        #[case] overflow: OverflowSpec,
        #[case] want: bool,
    ) {
        let axes = overflowing(OverflowSpec::Visible, overflow).scroll_keyword_axes();
        assert_eq!((axes.x, axes.y), (false, want));
    }

    #[test]
    fn a_hidden_attribute_does_not_hide_a_node_whose_display_is_set() {
        let (mut tree, [_, parent, node]) = chain();
        set_style(&mut tree, node, "overflow_x", "hidden").unwrap();
        set_style(&mut tree, node, "overflow_y", "scroll").unwrap();
        tree.set_attribute(parent, "hidden", true).unwrap();
        set_style(&mut tree, parent, "display", "flex").unwrap();
        assert_eq!(wheel(&tree, node), Some((false, true)));
    }

    #[test]
    fn a_rule_overflow_makes_a_scroll_container() {
        let mut f = Fixture::new();
        f.sheet(".s { overflow: scroll }");
        let node = classed(&mut f, "s");
        let plain = classed(&mut f, "plain");

        assert!(is_scroll_container(&f.tree, &f.cache, node));
        assert!(!is_scroll_container(&f.tree, &f.cache, plain));
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
}
