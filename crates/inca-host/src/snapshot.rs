// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A serializable copy of an app's node tree, for tests and the automation requests to read.

use std::collections::BTreeMap;

use gpui::{Pixels, Point, point, px};
use inca_node::{AttributeValue, ElementSpec, ElementTag, NodeId, VirtualTree, build_spec};
use serde::Serialize;
use serde_json::Value;

/// A rectangle in window coordinates, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Bounds {
    /// Distance of the left edge from the window's left edge.
    pub x: f32,
    /// Distance of the top edge from the window's top edge.
    pub y: f32,
    /// Horizontal extent.
    pub width: f32,
    /// Vertical extent.
    pub height: f32,
}

impl Bounds {
    /// The center of the box, in the same coordinates.
    #[must_use]
    pub fn center(&self) -> Point<Pixels> {
        point(
            px(self.x + self.width / 2.0),
            px(self.y + self.height / 2.0),
        )
    }
}

/// Reads gpui's box as logical pixels.
impl From<gpui::Bounds<Pixels>> for Bounds {
    fn from(b: gpui::Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(b.origin.x),
            y: f32::from(b.origin.y),
            width: f32::from(b.size.width),
            height: f32::from(b.size.height),
        }
    }
}

/// Where [`Node::nearest_bounds`] ended up.
#[derive(Debug, PartialEq)]
pub enum Lookup {
    /// `id` is not under the node.
    Absent,
    /// `id` is there, and neither it nor an ancestor has bounds.
    Unbounded,
    /// `id` is there, and it or its nearest ancestor with bounds has these.
    At(Bounds),
}

/// One node of a snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Node {
    /// The node's id in the app's tree.
    pub id: NodeId,
    /// The tag the app created the node with.
    pub tag: String,
    /// The data of a `"#text"` node. `None` for every other tag.
    pub text: Option<String>,
    /// The attributes the app set, keyed by name.
    pub attributes: BTreeMap<String, Value>,
    /// Where the node was laid out. `None` for a text node and for a node
    /// that has not been drawn.
    pub bounds: Option<Bounds>,
    /// The node's children in tree order.
    pub children: Vec<Node>,
}

impl Node {
    /// The first node in depth-first order, this one included, for which
    /// `predicate` holds.
    #[must_use]
    pub fn find(&self, predicate: &impl Fn(&Node) -> bool) -> Option<&Node> {
        if predicate(self) {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(predicate))
    }

    /// The bounds of node `id` in this tree, or of its nearest ancestor that has any.
    #[must_use]
    pub fn nearest_bounds(&self, id: NodeId) -> Lookup {
        self.nearest_bounds_from(id, None)
    }

    fn nearest_bounds_from(&self, id: NodeId, inherited: Option<Bounds>) -> Lookup {
        let here = self.bounds.or(inherited);
        if self.id == id {
            return here.map_or(Lookup::Unbounded, Lookup::At);
        }
        self.children
            .iter()
            .map(|child| child.nearest_bounds_from(id, here))
            .find(|found| *found != Lookup::Absent)
            .unwrap_or(Lookup::Absent)
    }
}

/// Copies `root` and its subtree out of `tree`. `bounds` supplies the layout
/// of each non-text node. Returns `None` when `root` is not in the tree.
#[must_use]
pub fn snapshot(
    tree: &VirtualTree,
    root: NodeId,
    bounds: &mut impl FnMut(NodeId) -> Option<Bounds>,
) -> Option<Node> {
    node(tree, &build_spec(tree, root)?, bounds)
}

fn node(
    tree: &VirtualTree,
    spec: &ElementSpec,
    bounds: &mut impl FnMut(NodeId) -> Option<Bounds>,
) -> Option<Node> {
    let source = tree.get(spec.id)?;
    let (text, layout) = match &spec.tag {
        ElementTag::Text(content) => (Some(content.clone()), None),
        ElementTag::Container | ElementTag::Input { .. } => (None, bounds(spec.id)),
    };
    Some(Node {
        id: spec.id,
        tag: source.local_name().to_owned(),
        text,
        attributes: source
            .attributes()
            .iter()
            .map(|(key, value)| (key.clone(), json(value)))
            .collect(),
        bounds: layout,
        children: spec
            .children
            .iter()
            .filter_map(|child| node(tree, child, bounds))
            .collect(),
    })
}

fn json(value: &AttributeValue) -> Value {
    match value {
        AttributeValue::String(s) => Value::from(s.as_str()),
        AttributeValue::Number(n) => Value::from(*n),
        AttributeValue::Bool(b) => Value::from(*b),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unnecessary_wraps, clippy::unwrap_used)]
    use super::*;

    fn no_bounds(_: NodeId) -> Option<Bounds> {
        None
    }

    fn unit(_: NodeId) -> Option<Bounds> {
        Some(Bounds {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        })
    }

    #[test]
    fn a_missing_root_has_no_snapshot() {
        let tree = VirtualTree::new();
        assert!(snapshot(&tree, 42, &mut no_bounds).is_none());
    }

    #[test]
    fn a_container_carries_its_tag_children_and_bounds() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("span").unwrap();
        tree.append_child(parent, child).unwrap();

        let snap = snapshot(&tree, parent, &mut unit).unwrap();

        assert_eq!(
            (snap.id, snap.tag.as_str(), snap.text.clone()),
            (parent, "div", None)
        );
        assert_eq!(snap.bounds, unit(parent));
        assert_eq!(snap.children.len(), 1);
        assert_eq!(snap.children[0].id, child);
        assert_eq!(snap.children[0].tag, "span");
        assert_eq!(snap.children[0].bounds, unit(child));
    }

    #[test]
    fn a_text_node_snapshot_carries_its_data() {
        let mut tree = VirtualTree::new();
        let text = tree.create_node("#text").unwrap();
        tree.set_data(text, "hi").unwrap();

        let snap = snapshot(&tree, text, &mut unit).unwrap();

        assert_eq!(snap.text.as_deref(), Some("hi"));
    }

    #[test]
    fn a_text_node_snapshot_has_no_bounds() {
        let mut tree = VirtualTree::new();
        let text = tree.create_node("#text").unwrap();
        tree.set_data(text, "hi").unwrap();

        let snap = snapshot(&tree, text, &mut unit).unwrap();

        assert_eq!(snap.bounds, None);
    }

    #[test]
    fn an_input_snapshots_as_a_box() {
        let mut tree = VirtualTree::new();
        let input = tree.create_node("input").unwrap();

        let snap = snapshot(&tree, input, &mut unit).unwrap();

        assert_eq!(snap.bounds, unit(input));
        assert_eq!(snap.text, None);
    }

    #[test]
    fn every_attribute_kind_becomes_a_json_value() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_attribute(id, "label", "x").unwrap();
        tree.set_attribute(id, "count", 3.0).unwrap();
        tree.set_attribute(id, "on", true).unwrap();

        let snap = snapshot(&tree, id, &mut no_bounds).unwrap();

        assert_eq!(
            serde_json::to_string(&snap.attributes).unwrap(),
            r#"{"count":3.0,"label":"x","on":true}"#
        );
    }

    #[test]
    fn find_returns_the_first_match_depth_first() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let a = tree.create_node("p").unwrap();
        let a_child = tree.create_node("b").unwrap();
        let b = tree.create_node("b").unwrap();
        tree.append_child(root, a).unwrap();
        tree.append_child(a, a_child).unwrap();
        tree.append_child(root, b).unwrap();
        let snap = snapshot(&tree, root, &mut no_bounds).unwrap();

        assert_eq!(snap.find(&|n| n.tag == "b").unwrap().id, a_child);
        assert_eq!(snap.find(&|n| n.id == root).unwrap().id, root);
    }

    #[test]
    fn nearest_bounds_inherits_from_the_closest_ancestor() {
        let b = |x| Bounds {
            x,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        };
        let node = |id, bounds, children| Node {
            id,
            tag: "div".into(),
            text: None,
            attributes: BTreeMap::new(),
            bounds,
            children,
        };
        let tree = node(
            1,
            Some(b(1.0)),
            vec![node(2, Some(b(2.0)), vec![node(3, None, vec![])])],
        );

        assert_eq!(tree.nearest_bounds(3), Lookup::At(b(2.0)));
        assert_eq!(tree.nearest_bounds(1), Lookup::At(b(1.0)));
        assert_eq!(tree.nearest_bounds(9), Lookup::Absent);
        let bare = node(1, None, vec![node(2, None, vec![])]);
        assert_eq!(bare.nearest_bounds(2), Lookup::Unbounded);
    }

    #[test]
    fn find_misses_when_nothing_matches() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let snap = snapshot(&tree, root, &mut no_bounds).unwrap();

        assert!(snap.find(&|n| n.tag == "nope").is_none());
    }
}
