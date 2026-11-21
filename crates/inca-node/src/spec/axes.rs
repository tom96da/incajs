// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The axes a wheel step can scroll.

use super::{DisplaySpec, hidden_by_attribute, style_spec_from_props};
use crate::tree::{NodeId, StyleCache, VirtualTree};

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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::spec::tests::{chain, wheel};
    use crate::tests::SVG;

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
    fn wheel_axes_is_not_none_under_an_svg_template() {
        let (mut tree, [_, parent, node]) = chain();
        tree.set_style(node, "overflow", "scroll").unwrap();
        let template = tree.create_element_ns(Some(SVG), None, "template").unwrap();
        tree.append_child(parent, template).unwrap();
        tree.append_child(template, node).unwrap();

        assert_eq!(wheel(&tree, node), Some((true, true)));
    }

    #[test]
    fn wheel_axes_is_none_under_a_template() {
        let (mut tree, [_, parent, node]) = chain();
        tree.set_style(node, "overflow", "scroll").unwrap();
        let template = tree.create_node("template").unwrap();
        tree.append_child(parent, template).unwrap();
        tree.append_child(template, node).unwrap();

        assert_eq!(wheel(&tree, node), None);
    }
}
