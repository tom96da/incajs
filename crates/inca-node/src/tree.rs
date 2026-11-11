// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Retained virtual tree.
//!
//! `VirtualTree` owns every `VirtualNode` and hands JS a [`NodeId`] for each.
//!
//! Two invariants the render path relies on: a node has at most one parent
//! (attaching it somewhere new detaches it from where it was), and an
//! attachment that would make a node its own ancestor is refused, so a walk
//! down the tree always terminates. A fragment never has a parent.
//!
//! A template owns a fragment as its content. The content has no parent and is never connected.
//!
//! A document can be declared with [`VirtualTree::set_document`]. It is the top of the tree and
//! [`VirtualTree::is_connected`] reads the parent chain up to it. A root declared with
//! [`VirtualTree::set_root`] is the node the host owns: the sweep never frees it. Without a
//! document, `is_connected` measures against the root.
//!
//! [`VirtualTree::remove_child`] detaches a node and keeps it with its
//! children.
//! [`VirtualTree::destroy_node`] frees a subtree on request.
//!
//! An orphan is a node with no parent that is neither the root, the document nor held with
//! [`VirtualTree::hold`]. [`VirtualTree::sweep`] frees every orphan with its
//! subtree. A node that is created, detached or released is an orphan until
//! it is inserted, held or destroyed.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;

use html5ever::Namespace;
use indexmap::IndexMap;

mod class_list;
mod computed;
mod element;
mod insertion;
mod lifetime;
mod links;
mod name;
mod props;
mod relations;
mod state;
mod template;
mod text;

pub use class_list::ClassListError;
pub use props::valid_attribute_name;

/// The tag of a node that stands in for a comment.
pub(crate) const COMMENT_TAG: &str = "#comment";

/// The tag of a text content node.
pub const TEXT_TAG: &str = "#text";

/// The tag of the document: the top of the tree, which holds at most one element and any number of
/// comments.
pub(crate) const DOCUMENT_TAG: &str = "#document";

/// The tag of a fragment: a node that holds children, has no parent and is never connected.
pub(crate) const FRAGMENT_TAG: &str = "#fragment";

/// Stable handle returned to JS, used in every subsequent host-bridge call.
pub type NodeId = u32;

/// An owned, primitive JS value for a style/attribute prop.
///
/// Restricted to primitives on purpose: a non-primitive value (an object or
/// array) can't be represented here, so a caller converting a JS value into
/// one of these must reject it explicitly. Nothing coerces or
/// stores it silently.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Bool(bool),
}

impl AttributeValue {
    /// The text of a `String` value, `None` for any other kind.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The text of the value: a string as is, a number through [`js_number_text`], a bool as `true`
    /// or `false`.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::String(text) => text.clone(),
            Self::Number(number) => js_number_text(*number),
            Self::Bool(flag) => flag.to_string(),
        }
    }

    /// The number of a `Number` value, `None` for any other kind.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl From<&str> for AttributeValue {
    fn from(value: &str) -> Self {
        AttributeValue::String(value.to_owned())
    }
}

impl From<String> for AttributeValue {
    fn from(value: String) -> Self {
        AttributeValue::String(value)
    }
}

impl From<f64> for AttributeValue {
    fn from(value: f64) -> Self {
        AttributeValue::Number(value)
    }
}

impl From<bool> for AttributeValue {
    fn from(value: bool) -> Self {
        AttributeValue::Bool(value)
    }
}

/// The text JavaScript makes of a number. `-0` prints as `0`, and values from `1e21` up or below
/// `1e-6` use the exponent form.
#[must_use]
pub fn js_number_text(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-Infinity" } else { "Infinity" }.to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    // `{:e}` prints the shortest digits that read back, as `d.ddde<exp>`.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    // The value is 0.<digits> * 10^point.
    let point = exponent + 1;
    let count = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    let body = if count <= point && point <= 21 {
        let zeros = usize::try_from(point - count).unwrap_or(0);
        format!("{digits}{}", "0".repeat(zeros))
    } else if 0 < point && point <= 21 {
        let at = usize::try_from(point).unwrap_or(0);
        format!("{}.{}", &digits[..at], &digits[at..])
    } else if -6 < point && point <= 0 {
        let zeros = usize::try_from(-point).unwrap_or(0);
        format!("0.{}{digits}", "0".repeat(zeros))
    } else {
        let power = point - 1;
        let exponent_sign = if power < 0 { "-" } else { "+" };
        let fraction = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        format!("{}{fraction}e{exponent_sign}{}", &digits[..1], power.abs())
    };
    format!("{sign}{body}")
}

/// Whether `node` is an HTML `button` or `input` carrying the `disabled`
/// attribute.
#[must_use]
pub fn is_disabled_control(node: &VirtualNode) -> bool {
    node.is_focusable_by_default() && node.attributes().contains_key("disabled")
}

/// The inline style props of a node, in the order they were set. Setting a key that exists keeps
/// its position, and a removed key that is set again goes last.
pub type StyleMap = IndexMap<String, AttributeValue>;

/// A single retained node. Always accessed through a [`VirtualTree`]. There
/// is no way to construct one standalone, since its [`id`](VirtualNode::id)
/// is only meaningful within the tree that allocated it.
#[derive(Debug, Clone)]
pub struct VirtualNode {
    id: NodeId,
    tag_name: String,
    /// The namespace of an element. `None` for a node that is no element and for an element made
    /// without one.
    namespace: Option<Namespace>,
    prefix: Option<String>,
    pub(crate) style_props: StyleMap,
    pub(crate) attributes: IndexMap<String, AttributeValue>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// The fragment a template owns.
    content: Option<NodeId>,
    /// The template that owns this fragment.
    owner: Option<NodeId>,
}

impl VirtualNode {
    fn new(
        id: NodeId,
        tag_name: String,
        namespace: Option<Namespace>,
        prefix: Option<String>,
    ) -> Self {
        Self {
            id,
            tag_name,
            namespace,
            prefix,
            style_props: StyleMap::new(),
            attributes: IndexMap::new(),
            parent: None,
            children: Vec::new(),
            content: None,
            owner: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> NodeId {
        self.id
    }

    #[must_use]
    pub fn tag_name(&self) -> &str {
        &self.tag_name
    }

    #[must_use]
    pub fn style_props(&self) -> &StyleMap {
        &self.style_props
    }

    #[must_use]
    pub fn attributes(&self) -> &IndexMap<String, AttributeValue> {
        &self.attributes
    }

    /// The node this one is attached to, or `None` while it is detached.
    #[must_use]
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// Ordered child handles. Order is append order, not insertion-sorted.
    #[must_use]
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

/// How a [`VirtualTree`] operation fails. Returned as a `Result`, so a caller further up the stack can turn it into whatever
/// handling it needs (at the JS boundary, a catchable exception).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeError {
    /// An id that doesn't, or no longer does, resolve to a node.
    NodeNotFound(NodeId),
    /// An attachment that would have made a node its own ancestor.
    WouldCycle(NodeId),
    /// Every id has been handed out.
    IdsExhausted,
    /// A text offset beyond the length of the data.
    IndexSize,
    /// A text node or a comment given as the parent of a child.
    LeafParent(NodeId),
    /// A reference or removed child that is not a child of the parent.
    NotAChild { parent: NodeId, child: NodeId },
    /// The document given as the node to insert.
    DocumentInserted(NodeId),
    /// A node the document cannot hold: text, a second element, or a fragment with either.
    DocumentChild(NodeId),
    /// A call that writes into the parent of a node, given a node that has no parent.
    NoParent(NodeId),
    /// A call that writes into the parent of a node whose parent is the document.
    DocumentParent(NodeId),
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::NodeNotFound(id) => write!(f, "unknown node id: {id}"),
            TreeError::WouldCycle(id) => {
                write!(
                    f,
                    "node {id} cannot be attached there. it would become its own ancestor"
                )
            }
            TreeError::IdsExhausted => write!(f, "node id space exhausted"),
            TreeError::IndexSize => write!(f, "the offset is beyond the length of the data"),
            TreeError::LeafParent(id) => write!(
                f,
                "node {id} cannot have children. only elements and fragments can"
            ),
            TreeError::NotAChild { parent, child } => write!(
                f,
                "node {child} is not a child of node {parent}. pass a child of the node you call it on"
            ),
            TreeError::DocumentInserted(_) => write!(
                f,
                "the document cannot be inserted. it is the top of the tree"
            ),
            TreeError::DocumentChild(_) => write!(
                f,
                "the document holds one element and no text. remove its element first or insert a single element"
            ),
            TreeError::NoParent(id) => write!(f, "node {id} has no parent to write into"),
            TreeError::DocumentParent(id) => write!(
                f,
                "node {id} is the element of the document. replace it through the document"
            ),
        }
    }
}

impl std::error::Error for TreeError {}

/// Owns the whole retained tree. One instance per `QuickJS` engine/document.
/// Ids from one `VirtualTree` are meaningless in another.
#[derive(Debug, Default)]
pub struct VirtualTree {
    nodes: HashMap<NodeId, VirtualNode>,
    next_id: NodeId,
    root: Option<NodeId>,
    document: Option<NodeId>,
    held: HashSet<NodeId>,
    orphans: HashSet<NodeId>,
    /// Bumped by every change to a node, a link, the root or the document, so a cache built from
    /// the tree can tell that it is stale.
    generation: u64,
    style_cache: RefCell<computed::SheetCache>,
    interaction: state::InteractionState,
}

impl VirtualTree {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares `id` the node the host owns: the sweep never frees it.
    pub fn set_root(&mut self, id: NodeId) {
        self.generation += 1;
        self.root = Some(id);
        self.orphans.remove(&id);
    }

    /// The root declared with [`set_root`](Self::set_root), if any.
    #[must_use]
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Declares `id` the document that [`is_connected`](Self::is_connected) measures against.
    pub fn set_document(&mut self, id: NodeId) {
        self.generation += 1;
        self.document = Some(id);
        self.orphans.remove(&id);
    }

    /// The document declared with [`set_document`](Self::set_document), if any.
    #[must_use]
    pub fn document(&self) -> Option<NodeId> {
        self.document
    }

    /// The first element child of the document, skipping comments. `None` while no document is
    /// declared or it has no element child.
    #[must_use]
    pub fn document_element(&self) -> Option<NodeId> {
        self.get(self.document?)?
            .children()
            .iter()
            .copied()
            .find(|&id| {
                self.get(id)
                    .is_some_and(|node| node.tag_name() != COMMENT_TAG)
            })
    }

    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&VirtualNode> {
        self.nodes.get(&id)
    }

    pub(crate) fn node_mut(&mut self, id: NodeId) -> Result<&mut VirtualNode, TreeError> {
        self.generation += 1;
        self.nodes.get_mut(&id).ok_or(TreeError::NodeNotFound(id))
    }

    /// `id` and its ancestors up to the root, nearest first. An id that names
    /// no node yields only itself.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(Some(id), |&id| self.get(id).and_then(VirtualNode::parent))
    }

    /// The first child of `id`. `None` for a childless or unknown node.
    #[must_use]
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id)?.children().first().copied()
    }

    /// The last child of `id`. `None` for a childless or unknown node.
    #[must_use]
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.get(id)?.children().last().copied()
    }

    /// The child of `id`'s parent right before `id`. `None` for the first child, a parentless
    /// node and an unknown node.
    #[must_use]
    pub fn previous_sibling(&self, id: NodeId) -> Option<NodeId> {
        let siblings = self.get(self.get(id)?.parent()?)?.children();
        let index = siblings.iter().position(|&child| child == id)?;
        siblings.get(index.checked_sub(1)?).copied()
    }

    /// The child of `id`'s parent right after `id`. `None` for the last child, a parentless
    /// node and an unknown node.
    #[must_use]
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let siblings = self.get(self.get(id)?.parent()?)?.children();
        let index = siblings.iter().position(|&child| child == id)?;
        siblings.get(index + 1).copied()
    }

    /// The node right before `id`'s subtree in tree order: the last node of
    /// its previous sibling's subtree, or its parent when it is the first
    /// child. `None` for a node without a parent.
    #[must_use]
    pub fn preceding(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.get(id)?.parent()?;
        let siblings = self.get(parent)?.children();
        let index = siblings.iter().position(|&child| child == id)?;
        let Some(&previous) = index.checked_sub(1).and_then(|i| siblings.get(i)) else {
            return Some(parent);
        };
        let mut last = previous;
        while let Some(&child) = self.get(last).and_then(|node| node.children().last()) {
            last = child;
        }
        Some(last)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{Fixture, node};
    use rstest::rstest;

    #[test]
    fn ids_exhausted_has_a_message() {
        assert_eq!(
            TreeError::IdsExhausted.to_string(),
            "node id space exhausted"
        );
    }

    #[test]
    fn operations_on_unknown_ids_error_instead_of_panicking() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        assert_eq!(
            tree.append_child(node, 999).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.append_child(999, node).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.remove_child(999, node).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.set_attribute(999, "k", "v").unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.set_style(999, "k", "v").unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.remove_style(999, "k").unwrap_err(),
            TreeError::NodeNotFound(999)
        );
    }

    #[test]
    fn ancestors_lists_the_node_then_its_parents_up_to_the_root() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let mid = tree.create_node("div").unwrap();
        let leaf = tree.create_node("div").unwrap();
        tree.append_child(root, mid).unwrap();
        tree.append_child(mid, leaf).unwrap();

        assert_eq!(tree.ancestors(leaf).collect::<Vec<_>>(), [leaf, mid, root]);
        assert_eq!(tree.ancestors(root).collect::<Vec<_>>(), [root]);
    }

    #[test]
    fn ancestors_of_an_unknown_id_yields_only_that_id() {
        let tree = VirtualTree::new();
        assert_eq!(tree.ancestors(7).collect::<Vec<_>>(), [7]);
    }

    #[rstest]
    #[case::button("button", &["disabled"], true)]
    #[case::input("input", &["disabled"], true)]
    #[case::div("div", &["disabled"], false)]
    #[case::enabled_button("button", &[], false)]
    fn is_disabled_control_needs_a_control_tag_and_the_disabled_attribute(
        #[case] tag: &str,
        #[case] attributes: &[&str],
        #[case] want: bool,
    ) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node(tag).unwrap();
        for attribute in attributes {
            tree.set_attribute(node, *attribute, true).unwrap();
        }
        assert_eq!(is_disabled_control(tree.get(node).unwrap()), want, "{tag}");
    }

    #[rstest]
    #[case::negative_zero(-0.0, "0")]
    #[case::infinity(f64::INFINITY, "Infinity")]
    #[case::negative_infinity(f64::NEG_INFINITY, "-Infinity")]
    #[case::nan(f64::NAN, "NaN")]
    #[case::large(1e21, "1e+21")]
    #[case::small(1e-7, "1e-7")]
    #[case::integer(42.0, "42")]
    #[case::negative_fraction(-1.5, "-1.5")]
    #[case::below_the_exponent_form(1e20, "100000000000000000000")]
    #[case::decimal_zeros(0.000_001, "0.000001")]
    #[case::exponent_with_digits(1.5e-7, "1.5e-7")]
    #[case::fraction(0.5, "0.5")]
    fn js_number_text_prints_a_number_as_javascript_does(
        #[case] value: f64,
        #[case] expected: &str,
    ) {
        assert_eq!(js_number_text(value), expected);
    }

    /// `parent` holds `a`, `b`, `c`. `lone_parent` holds `lone`. `loose` has no parent. The last
    /// entry is an id no node has.
    fn linked_tree() -> (VirtualTree, [NodeId; 8]) {
        let mut tree = VirtualTree::new();
        let [parent, a, b, c, lone_parent, lone, loose] =
            [(); 7].map(|()| tree.create_node("div").unwrap());
        for child in [a, b, c] {
            tree.append_child(parent, child).unwrap();
        }
        tree.append_child(lone_parent, lone).unwrap();
        (tree, [parent, a, b, c, lone_parent, lone, loose, 999])
    }

    const PARENT: usize = 0;
    const A: usize = 1;
    const B: usize = 2;
    const C: usize = 3;
    const LONE_PARENT: usize = 4;
    const LONE: usize = 5;
    const LOOSE: usize = 6;
    const UNKNOWN: usize = 7;

    #[rstest]
    #[case::first(A, Some(B))]
    #[case::middle(B, Some(C))]
    #[case::last(C, None)]
    #[case::only(LONE, None)]
    #[case::detached(LOOSE, None)]
    #[case::unknown(UNKNOWN, None)]
    fn next_sibling_is_the_child_after(#[case] node: usize, #[case] want: Option<usize>) {
        let (tree, ids) = linked_tree();

        assert_eq!(tree.next_sibling(ids[node]), want.map(|at| ids[at]));
    }

    #[rstest]
    #[case::first(A, None)]
    #[case::middle(B, Some(A))]
    #[case::last(C, Some(B))]
    #[case::only(LONE, None)]
    #[case::detached(LOOSE, None)]
    #[case::unknown(UNKNOWN, None)]
    fn previous_sibling_is_the_child_before(#[case] node: usize, #[case] want: Option<usize>) {
        let (tree, ids) = linked_tree();

        assert_eq!(tree.previous_sibling(ids[node]), want.map(|at| ids[at]));
    }

    #[rstest]
    #[case::several(PARENT, Some(A))]
    #[case::only(LONE_PARENT, Some(LONE))]
    #[case::childless(LOOSE, None)]
    #[case::unknown(UNKNOWN, None)]
    fn first_child_is_the_first_of_the_children(#[case] node: usize, #[case] want: Option<usize>) {
        let (tree, ids) = linked_tree();

        assert_eq!(tree.first_child(ids[node]), want.map(|at| ids[at]));
    }

    #[rstest]
    #[case::several(PARENT, Some(C))]
    #[case::only(LONE_PARENT, Some(LONE))]
    #[case::childless(LOOSE, None)]
    #[case::unknown(UNKNOWN, None)]
    fn last_child_is_the_last_of_the_children(#[case] node: usize, #[case] want: Option<usize>) {
        let (tree, ids) = linked_tree();

        assert_eq!(tree.last_child(ids[node]), want.map(|at| ids[at]));
    }

    #[test]
    fn preceding_is_none_without_a_parent() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();

        assert_eq!(tree.preceding(id), None);
    }

    #[test]
    fn preceding_of_a_first_child_is_its_parent() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.append_child(parent, child).unwrap();

        assert_eq!(tree.preceding(child), Some(parent));
    }

    #[test]
    fn preceding_is_the_last_node_before_the_subtree() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div").unwrap();
        let first = tree.create_node("div").unwrap();
        let inner = tree.create_node("div").unwrap();
        let deepest = tree.create_node("div").unwrap();
        let second = tree.create_node("div").unwrap();
        tree.append_child(parent, first).unwrap();
        tree.append_child(first, inner).unwrap();
        tree.append_child(inner, deepest).unwrap();
        tree.append_child(parent, second).unwrap();

        assert_eq!(tree.preceding(second), Some(deepest));
    }

    #[test]
    fn a_declared_document_is_no_orphan() {
        let mut tree = VirtualTree::new();
        let document = tree.create_node(DOCUMENT_TAG).unwrap();
        tree.set_document(document);

        tree.sweep();

        assert!(tree.get(document).is_some());
    }

    #[test]
    fn the_document_element_skips_a_leading_comment() {
        let (mut f, document) = Fixture::with_document();
        let comment = node(&mut f.tree, COMMENT_TAG);
        f.tree
            .insert_before(document, comment, Some(f.root))
            .unwrap();

        assert_eq!(f.tree.document_element(), Some(f.root));
    }

    #[test]
    fn there_is_no_document_element_among_only_comments() {
        let mut tree = VirtualTree::new();
        let document = tree.create_node(DOCUMENT_TAG).unwrap();
        let comment = tree.create_node(COMMENT_TAG).unwrap();
        tree.append_child(document, comment).unwrap();
        tree.set_document(document);

        assert_eq!(tree.document_element(), None);
    }
}
