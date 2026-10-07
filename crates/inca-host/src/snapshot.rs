// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A serializable copy of an app's node tree, for tests to read.

use std::collections::BTreeMap;

use inca_gpui::{AttributeValue, ElementSpec, ElementTag, NodeId, VirtualTree, build_spec};
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

/// One node of a snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Node {
    /// The node's id in the app's tree.
    pub id: NodeId,
    /// The tag the app created the node with.
    pub tag: String,
    /// The text a `"text"` node renders. `None` for every other tag.
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
        ElementTag::Container | ElementTag::Button { .. } => (None, bounds(spec.id)),
    };
    Some(Node {
        id: spec.id,
        tag: source.tag_name().to_owned(),
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
#[allow(clippy::unwrap_used, clippy::unnecessary_wraps)]
mod tests {
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
    fn a_text_node_renders_its_content_and_has_no_bounds() {
        let mut tree = VirtualTree::new();
        let text = tree.create_node("text").unwrap();
        tree.set_attribute(text, "value", "hi").unwrap();
        let inner = tree.create_node("text").unwrap();
        tree.set_attribute(inner, "value", " there").unwrap();
        tree.append_child(text, inner).unwrap();

        let snap = snapshot(&tree, text, &mut unit).unwrap();

        assert_eq!(snap.text.as_deref(), Some("hi there"));
        assert_eq!(snap.bounds, None);
        assert!(snap.children.is_empty());
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
    fn a_number_without_a_json_form_becomes_null() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_attribute(id, "nan", f64::NAN).unwrap();
        tree.set_attribute(id, "inf", f64::INFINITY).unwrap();

        let snap = snapshot(&tree, id, &mut no_bounds).unwrap();

        assert_eq!(snap.attributes["nan"], Value::Null);
        assert_eq!(snap.attributes["inf"], Value::Null);
    }

    #[test]
    fn a_snapshot_serializes_to_json() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();

        let json = serde_json::to_value(snapshot(&tree, id, &mut unit).unwrap()).unwrap();

        assert_eq!(json["tag"], "div");
        assert_eq!(json["bounds"]["width"], 3.0);
        assert!(json["text"].is_null());
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
    fn find_misses_when_nothing_matches() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let snap = snapshot(&tree, root, &mut no_bounds).unwrap();

        assert!(snap.find(&|n| n.tag == "nope").is_none());
    }
}
