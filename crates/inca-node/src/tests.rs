// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test helpers that the tests of several modules share.

#![allow(clippy::unwrap_used)]

use crate::selector::{closest_selector, matches_selector};
use crate::spec::*;
use crate::tree::{AttributeValue, NodeId, TEXT_TAG, VirtualTree};

/// A tree under construction. Every node an `add` call makes gets an `id` attribute of `name`.
pub(crate) struct Fixture {
    pub(crate) tree: VirtualTree,
    pub(crate) root: NodeId,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.set_attribute(root, "id", "root").unwrap();
        Fixture { tree, root }
    }

    /// A fixture whose root hangs under a new `#document`. Returns it with the document.
    pub(crate) fn with_document() -> (Self, NodeId) {
        let mut f = Self::new();
        let document = f.tree.create_node("#document").unwrap();
        f.tree.set_document(document);
        f.tree.append_child(document, f.root).unwrap();
        (f, document)
    }

    pub(crate) fn add(&mut self, parent: NodeId, tag: &str, name: &str) -> NodeId {
        let node = self.tree.create_node(tag).unwrap();
        self.tree.append_child(parent, node).unwrap();
        self.tree.set_attribute(node, "id", name).unwrap();
        node
    }

    pub(crate) fn attr(&mut self, node: NodeId, key: &str, value: impl Into<AttributeValue>) {
        self.tree.set_attribute(node, key, value).unwrap();
    }

    /// A `<style>` element under the root that holds `css`, and its text node.
    pub(crate) fn sheet(&mut self, css: &str) -> (NodeId, NodeId) {
        let style = self.add(self.root, "style", "sheet");
        let text_node = self.tree.create_node("#text").unwrap();
        self.attr(text_node, "value", css);
        self.tree.append_child(style, text_node).unwrap();
        (style, text_node)
    }

    /// A `div` under the root with `name` as its id and its class.
    pub(crate) fn boxed(&mut self, name: &str) -> NodeId {
        let node = self.add(self.root, "div", name);
        self.attr(node, "class", name);
        node
    }

    pub(crate) fn computed(&self, node: NodeId, key: &str) -> Option<AttributeValue> {
        self.tree.computed_style(node).get(key).cloned()
    }

    pub(crate) fn matches(&self, node: NodeId, selector: &str) -> bool {
        matches_selector(&self.tree, node, selector).unwrap()
    }

    pub(crate) fn closest(&self, node: NodeId, selector: &str) -> Option<NodeId> {
        closest_selector(&self.tree, node, selector).unwrap()
    }
}

/// `root > main > (p1, p2)` and `root > foot`, with the attributes the selector rows read.
pub(crate) struct Page {
    pub(crate) f: Fixture,
    pub(crate) main: NodeId,
    pub(crate) p1: NodeId,
    pub(crate) p2: NodeId,
    pub(crate) p3: NodeId,
    pub(crate) foot: NodeId,
}

pub(crate) fn page() -> Page {
    let mut f = Fixture::new();
    let main = f.add(f.root, "section", "main");
    f.attr(main, "class", "card");
    f.attr(main, "data-k", "ab-cd ef");
    let p1 = f.add(main, "p", "p1");
    let p2 = f.add(main, "span", "p2");
    let p3 = f.add(main, "p", "p3");
    let foot = f.add(f.root, "footer", "foot");
    Page {
        f,
        main,
        p1,
        p2,
        p3,
        foot,
    }
}

impl Page {
    pub(crate) fn node(&self, name: &str) -> NodeId {
        match name {
            "root" => self.f.root,
            "main" => self.main,
            "p1" => self.p1,
            "p2" => self.p2,
            "p3" => self.p3,
            _ => self.foot,
        }
    }
}

/// `parent > [comment, text node, a, b]`.
pub(crate) struct Mixed {
    pub(crate) f: Fixture,
    pub(crate) parent: NodeId,
    pub(crate) comment: NodeId,
    pub(crate) a: NodeId,
    pub(crate) text_node: NodeId,
    pub(crate) b: NodeId,
}

pub(crate) fn mixed() -> Mixed {
    let mut f = Fixture::new();
    let parent = f.add(f.root, "div", "parent");
    let comment = f.add(parent, "#comment", "comment");
    let text_node = f.add(parent, "#text", "te");
    f.attr(text_node, "value", "t");
    let a = f.add(parent, "span", "a");
    let b = f.add(parent, "span", "b");
    Mixed {
        f,
        parent,
        comment,
        a,
        text_node,
        b,
    }
}

pub(crate) fn num(n: f64) -> AttributeValue {
    AttributeValue::Number(n)
}

pub(crate) fn text(s: &str) -> AttributeValue {
    AttributeValue::String(s.to_owned())
}

pub(crate) fn style_of(props: &[(&str, AttributeValue)]) -> StyleSpec {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        tree.set_style(id, *key, value.clone()).unwrap();
    }
    build_spec(&tree, id).unwrap().style
}

/// The style a node gets from `props`, and what [`style_warning`] says
/// about each prop, in order.
pub(crate) fn style_and_warnings(props: &[(&str, AttributeValue)]) -> (StyleSpec, Vec<String>) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        tree.set_style(id, *key, value.clone()).unwrap();
    }
    let warnings = props
        .iter()
        .filter_map(|(key, value)| style_warning(key, value))
        .collect();
    (build_spec(&tree, id).unwrap().style, warnings)
}

pub(crate) fn sides<T: Copy>(edges: EdgesSpec<T>) -> [Option<T>; 4] {
    [edges.top, edges.right, edges.bottom, edges.left]
}

pub(crate) fn overflow_of(props: &[(&str, &str)]) -> (Option<OverflowSpec>, Option<OverflowSpec>) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        tree.set_style(id, *key, *value).unwrap();
    }
    let style = build_spec(&tree, id).unwrap().style;
    (style.overflow_x, style.overflow_y)
}

/// The HTML namespace.
pub(crate) const HTML: &str = "http://www.w3.org/1999/xhtml";

/// The SVG namespace.
pub(crate) const SVG: &str = "http://www.w3.org/2000/svg";

/// The `MathML` namespace.
pub(crate) const MATHML: &str = "http://www.w3.org/1998/Math/MathML";

/// A tree node of `tag`.
pub(crate) fn node(tree: &mut VirtualTree, tag: &str) -> NodeId {
    tree.create_node(tag).unwrap()
}

/// A `div` parent with `count` `span` children.
pub(crate) fn family(tree: &mut VirtualTree, count: usize) -> (NodeId, Vec<NodeId>) {
    let parent = node(tree, "div");
    let kids = (0..count)
        .map(|_| {
            let kid = node(tree, "span");
            tree.append_child(parent, kid).unwrap();
            kid
        })
        .collect();
    (parent, kids)
}

pub(crate) fn children(tree: &VirtualTree, id: NodeId) -> Vec<NodeId> {
    tree.get(id).unwrap().children().to_vec()
}

pub(crate) fn not_a_child(parent: NodeId, child: NodeId) -> crate::tree::TreeError {
    crate::tree::TreeError::NotAChild { parent, child }
}

/// `tag[class=a, tabindex=1, width=4] > (span > #text "hi", #text "t")`.
pub(crate) fn sample(tree: &mut VirtualTree, tag: &str) -> NodeId {
    let root = node(tree, tag);
    tree.set_attribute(root, "class", "a").unwrap();
    tree.set_attribute(root, "tabindex", 1.0).unwrap();
    tree.set_style(root, "width", 4.0).unwrap();
    let span = node(tree, "span");
    let inner = node(tree, TEXT_TAG);
    tree.set_attribute(inner, "value", "hi").unwrap();
    let tail = node(tree, TEXT_TAG);
    tree.set_attribute(tail, "value", "t").unwrap();
    tree.append_child(root, span).unwrap();
    tree.append_child(span, inner).unwrap();
    tree.append_child(root, tail).unwrap();
    root
}
