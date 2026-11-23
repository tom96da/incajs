// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test helpers that the tests of several modules share.

#![allow(clippy::unwrap_used)]

use crate::ParsedHtml;
use crate::tree::{NodeId, NodeKind, TEXT_TAG, VirtualTree};

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
    let styled = tree.node_mut(root).unwrap();
    styled.style_props.insert("width".into(), 4.0.into());
    styled
        .attributes
        .insert("style".into(), "width: 4px;".into());
    let span = node(tree, "span");
    let inner = node(tree, TEXT_TAG);
    tree.set_data(inner, "hi").unwrap();
    let tail = node(tree, TEXT_TAG);
    tree.set_data(tail, "t").unwrap();
    tree.append_child(root, span).unwrap();
    tree.append_child(span, inner).unwrap();
    tree.append_child(root, tail).unwrap();
    root
}

pub(crate) fn parsed(context: Option<&str>, markup: &str) -> (VirtualTree, ParsedHtml) {
    let mut tree = VirtualTree::new();
    let context = context.map(|tag| node(&mut tree, tag));
    let html = tree.parse_html(context, markup).unwrap();
    (tree, html)
}

/// The nodes below `id` as text: `tag[key=value](children)`, `"text"`, `<!--comment-->` and
/// `template{content}`.
#[allow(clippy::format_push_string)]
pub(crate) fn shape(tree: &VirtualTree, id: NodeId) -> String {
    let node = tree.get(id).unwrap();
    match node.kind() {
        NodeKind::Text => return format!("{:?}", node.data()),
        NodeKind::Comment => return format!("<!--{}-->", node.data()),
        _ => {}
    }
    let mut text = if node.kind() == NodeKind::Fragment {
        String::new()
    } else {
        node.local_name().to_owned()
    };
    if let Some(content) = tree.template_content(id) {
        text.push_str(&format!("{{{}}}", children_shape(tree, content)));
    }
    if !node.children().is_empty() {
        text.push_str(&format!("({})", children_shape(tree, id)));
    }
    text
}

/// The nodes under `id`, separated by spaces.
pub(crate) fn children_shape(tree: &VirtualTree, id: NodeId) -> String {
    let children = tree.get(id).unwrap().children();
    let shapes: Vec<String> = children.iter().map(|&c| shape(tree, c)).collect();
    shapes.join(" ")
}
