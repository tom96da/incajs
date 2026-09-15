// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Retained virtual tree.
//!
//! `VirtualTree` owns every `VirtualNode` and hands JS a [`NodeId`] for each.
//!
//! Two invariants the render path relies on: a node has at most one parent —
//! attaching it somewhere new detaches it from where it was — and an
//! attachment that would make a node its own ancestor is refused, so a walk
//! down the tree always terminates.
//!
//! [`VirtualTree::remove_child`] detaches and keeps the node alive.
//! [`VirtualTree::destroy_node`] is the only call that frees.

use std::collections::HashMap;
use std::fmt;

/// Stable handle returned to JS, used in every subsequent host-bridge call.
pub type NodeId = u32;

/// An owned, primitive JS value for a style/attribute prop.
///
/// Restricted to primitives on purpose: a non-primitive value (an object or
/// array) can't be represented here, so a caller converting a JS value into
/// one of these is forced to reject it explicitly instead of coercing or
/// storing it silently.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Bool(bool),
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

/// A single retained node. Always accessed through a [`VirtualTree`] — there
/// is no way to construct one standalone, since its [`id`](VirtualNode::id)
/// is only meaningful within the tree that allocated it.
#[derive(Debug, Clone)]
pub struct VirtualNode {
    id: NodeId,
    tag_name: String,
    style_props: HashMap<String, AttributeValue>,
    attributes: HashMap<String, AttributeValue>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

impl VirtualNode {
    fn new(id: NodeId, tag_name: String) -> Self {
        Self {
            id,
            tag_name,
            style_props: HashMap::new(),
            attributes: HashMap::new(),
            parent: None,
            children: Vec::new(),
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
    pub fn style_props(&self) -> &HashMap<String, AttributeValue> {
        &self.style_props
    }

    #[must_use]
    pub fn attributes(&self) -> &HashMap<String, AttributeValue> {
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

/// How a [`VirtualTree`] operation fails. Returned as a `Result` rather than
/// panicking so a caller further up the stack can turn it into whatever
/// handling it needs — at the JS boundary, a catchable exception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeError {
    /// An id that doesn't, or no longer does, resolve to a node.
    NodeNotFound(NodeId),
    /// An attachment that would have made a node its own ancestor.
    WouldCycle(NodeId),
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::NodeNotFound(id) => write!(f, "unknown node id: {id}"),
            TreeError::WouldCycle(id) => {
                write!(
                    f,
                    "attaching node {id} there would make it its own ancestor"
                )
            }
        }
    }
}

impl std::error::Error for TreeError {}

/// Owns the whole retained tree. One instance per `QuickJS` engine/document —
/// ids from one `VirtualTree` are meaningless in another.
#[derive(Debug, Default)]
pub struct VirtualTree {
    nodes: HashMap<NodeId, VirtualNode>,
    next_id: NodeId,
}

impl VirtualTree {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a new node and returns its id. Ids are assigned
    /// sequentially starting at 0 and are never reused, even after the node
    /// they named is destroyed — a stale id names nothing rather than
    /// something else.
    ///
    /// # Panics
    ///
    /// Panics if the id space is exhausted (`u32::MAX` nodes ever created).
    pub fn create_node(&mut self, tag_name: impl Into<String>) -> NodeId {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("VirtualTree node id space exhausted");
        self.nodes.insert(id, VirtualNode::new(id, tag_name.into()));
        id
    }

    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&VirtualNode> {
        self.nodes.get(&id)
    }

    /// Appends `child_id` to `parent_id`'s children. Thin wrapper over
    /// [`insert_before`](Self::insert_before) with no anchor.
    ///
    /// # Errors
    ///
    /// Same as [`insert_before`](Self::insert_before), with no anchor to
    /// fail on.
    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<(), TreeError> {
        self.insert_before(parent_id, child_id, None)
    }

    /// Inserts `child_id` into `parent_id`'s children, before `anchor_id` if
    /// given, or at the end if `None`. `child_id` is detached from whatever
    /// it was attached to first, so this both attaches and moves — a node
    /// never has two parents.
    ///
    /// If `anchor_id` names a real node that isn't (or is no longer) among
    /// `parent_id`'s children, this falls back to appending at the end, same
    /// as an absent `remove_child` target — only a truly
    /// unknown/never-allocated `anchor_id` is an error.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `parent_id` or `child_id`
    /// names no node, or if `anchor_id` is `Some` and names no node.
    /// Returns [`TreeError::WouldCycle`] if `child_id` is `parent_id` or one
    /// of its ancestors.
    pub fn insert_before(
        &mut self,
        parent_id: NodeId,
        child_id: NodeId,
        anchor_id: Option<NodeId>,
    ) -> Result<(), TreeError> {
        if !self.nodes.contains_key(&child_id) {
            return Err(TreeError::NodeNotFound(child_id));
        }
        if let Some(anchor_id) = anchor_id
            && !self.nodes.contains_key(&anchor_id)
        {
            return Err(TreeError::NodeNotFound(anchor_id));
        }
        if !self.nodes.contains_key(&parent_id) {
            return Err(TreeError::NodeNotFound(parent_id));
        }
        if self.is_self_or_ancestor(child_id, parent_id) {
            return Err(TreeError::WouldCycle(child_id));
        }

        self.detach(child_id);

        let Some(parent) = self.nodes.get_mut(&parent_id) else {
            return Err(TreeError::NodeNotFound(parent_id));
        };
        let index = anchor_id
            .and_then(|anchor_id| parent.children.iter().position(|&id| id == anchor_id))
            .unwrap_or(parent.children.len());
        parent.children.insert(index, child_id);
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        Ok(())
    }

    /// Whether `candidate` is `of` itself or sits above it.
    fn is_self_or_ancestor(&self, candidate: NodeId, of: NodeId) -> bool {
        let mut current = Some(of);
        while let Some(id) = current {
            if id == candidate {
                return true;
            }
            current = self.nodes.get(&id).and_then(|node| node.parent);
        }
        false
    }

    /// Unlinks `id` from its parent, if it has one. Leaves the node itself
    /// alone, children and all.
    fn detach(&mut self, id: NodeId) {
        let Some(parent_id) = self.nodes.get(&id).and_then(|node| node.parent) else {
            return;
        };
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.retain(|&child| child != id);
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = None;
        }
    }

    /// Frees `id` and its whole subtree, detaching it from its parent first,
    /// and returns every freed id, `id` among them.
    ///
    /// The subtree goes too because a caller removing a node holds that
    /// node's id and nothing below it. An unknown `id` frees nothing and
    /// returns an empty `Vec`: destroying twice is a no-op, not an error.
    pub fn destroy_node(&mut self, id: NodeId) -> Vec<NodeId> {
        if !self.nodes.contains_key(&id) {
            return Vec::new();
        }
        self.detach(id);

        let mut freed = Vec::new();
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            if let Some(node) = self.nodes.remove(&current) {
                pending.extend(node.children.iter().copied());
                freed.push(current);
            }
        }
        freed
    }

    /// Unlinks `child_id` from `parent_id`'s children, if present. `child_id`
    /// not being (or no longer being) a child of `parent_id` is a no-op, not
    /// an error — only an unknown `parent_id` is. The child node stays alive
    /// and keeps its own children; [`destroy_node`](Self::destroy_node) is
    /// what frees.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `parent_id` names no node.
    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<(), TreeError> {
        let parent = self
            .nodes
            .get_mut(&parent_id)
            .ok_or(TreeError::NodeNotFound(parent_id))?;
        let before = parent.children.len();
        parent.children.retain(|&id| id != child_id);
        if parent.children.len() == before {
            return Ok(());
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = None;
        }
        Ok(())
    }

    /// Sets (inserting or overwriting) one attribute prop.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn set_attribute(
        &mut self,
        node_id: NodeId,
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Result<(), TreeError> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(TreeError::NodeNotFound(node_id))?;
        node.attributes.insert(key.into(), value.into());
        Ok(())
    }

    /// Sets (inserting or overwriting) one style prop.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn set_style(
        &mut self,
        node_id: NodeId,
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Result<(), TreeError> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(TreeError::NodeNotFound(node_id))?;
        node.style_props.insert(key.into(), value.into());
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn id_uniqueness() {
        let mut tree = VirtualTree::new();
        let ids: Vec<NodeId> = (0..100).map(|_| tree.create_node("div")).collect();

        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "create_node must never reuse an id"
        );
    }

    #[test]
    fn append_order() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let a = tree.create_node("span");
        let b = tree.create_node("span");
        let c = tree.create_node("span");

        tree.append_child(parent, a).unwrap();
        tree.append_child(parent, b).unwrap();
        tree.append_child(parent, c).unwrap();

        assert_eq!(tree.get(parent).unwrap().children(), &[a, b, c]);
    }

    #[test]
    fn insert_before_at_start_middle_end() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let a = tree.create_node("span");
        let b = tree.create_node("span");
        let c = tree.create_node("span");

        tree.insert_before(parent, b, None).unwrap(); // end: [b]
        tree.insert_before(parent, a, Some(b)).unwrap(); // start: [a, b]
        tree.insert_before(parent, c, None).unwrap(); // end: [a, b, c]

        assert_eq!(tree.get(parent).unwrap().children(), &[a, b, c]);

        let d = tree.create_node("span");
        tree.insert_before(parent, d, Some(b)).unwrap(); // middle: [a, d, b, c]
        assert_eq!(tree.get(parent).unwrap().children(), &[a, d, b, c]);
    }

    #[test]
    fn insert_before_unknown_anchor_falls_back_to_append() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let a = tree.create_node("span");
        let stray_anchor = tree.create_node("span"); // real node, never attached here

        tree.insert_before(parent, a, Some(stray_anchor)).unwrap();

        assert_eq!(tree.get(parent).unwrap().children(), &[a]);
    }

    #[test]
    fn insert_before_unknown_ids_error() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let child = tree.create_node("span");

        assert_eq!(
            tree.insert_before(999, child, None).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.insert_before(parent, 999, None).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
        assert_eq!(
            tree.insert_before(parent, child, Some(999)).unwrap_err(),
            TreeError::NodeNotFound(999)
        );
    }

    #[test]
    fn a_node_has_one_parent_at_a_time() {
        let mut tree = VirtualTree::new();
        let first = tree.create_node("div");
        let second = tree.create_node("div");
        let child = tree.create_node("span");

        tree.append_child(first, child).unwrap();
        tree.append_child(second, child).unwrap();

        assert!(
            tree.get(first).unwrap().children().is_empty(),
            "attaching elsewhere must detach from the previous parent"
        );
        assert_eq!(tree.get(second).unwrap().children(), &[child]);
        assert_eq!(tree.get(child).unwrap().parent(), Some(second));
    }

    #[test]
    fn a_fresh_node_has_no_parent() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div");

        assert_eq!(tree.get(node).unwrap().parent(), None);
    }

    #[test]
    fn attaching_a_node_under_itself_is_refused() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div");

        assert_eq!(
            tree.append_child(node, node).unwrap_err(),
            TreeError::WouldCycle(node)
        );
    }

    #[test]
    fn attaching_an_ancestor_under_its_descendant_is_refused() {
        let mut tree = VirtualTree::new();
        let grandparent = tree.create_node("div");
        let parent = tree.create_node("div");
        let child = tree.create_node("div");
        tree.append_child(grandparent, parent).unwrap();
        tree.append_child(parent, child).unwrap();

        assert_eq!(
            tree.append_child(child, grandparent).unwrap_err(),
            TreeError::WouldCycle(grandparent),
            "a cycle here would leave the render walk with no end"
        );
        assert_eq!(tree.get(grandparent).unwrap().children(), &[parent]);
    }

    #[test]
    fn moving_a_child_within_one_parent_reorders_it() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let a = tree.create_node("span");
        let b = tree.create_node("span");
        tree.append_child(parent, a).unwrap();
        tree.append_child(parent, b).unwrap();

        tree.insert_before(parent, b, Some(a)).unwrap();

        assert_eq!(
            tree.get(parent).unwrap().children(),
            &[b, a],
            "a move must not leave the node listed twice"
        );
    }

    #[test]
    fn destroy_frees_the_whole_subtree() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div");
        let branch = tree.create_node("div");
        let leaf = tree.create_node("text");
        tree.append_child(root, branch).unwrap();
        tree.append_child(branch, leaf).unwrap();

        let mut freed = tree.destroy_node(branch);
        freed.sort_unstable();

        assert_eq!(
            freed,
            vec![branch, leaf],
            "a descendant is unreachable once its root is gone, so it must go too"
        );
        assert!(tree.get(branch).is_none());
        assert!(tree.get(leaf).is_none());
        assert!(tree.get(root).unwrap().children().is_empty());
    }

    #[test]
    fn destroy_is_a_no_op_the_second_time_and_for_an_unknown_id() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div");

        assert_eq!(tree.destroy_node(node), vec![node]);
        assert!(tree.destroy_node(node).is_empty());
        assert!(tree.destroy_node(9999).is_empty());
    }

    #[test]
    fn destroyed_ids_are_not_handed_out_again() {
        let mut tree = VirtualTree::new();
        let first = tree.create_node("div");
        tree.destroy_node(first);

        assert_ne!(
            tree.create_node("div"),
            first,
            "a stale id must name nothing rather than something else"
        );
    }

    #[test]
    fn remove_child_clears_the_parent_link() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let child = tree.create_node("span");
        tree.append_child(parent, child).unwrap();

        tree.remove_child(parent, child).unwrap();

        assert_eq!(tree.get(child).unwrap().parent(), None);
    }

    #[test]
    fn remove_child_leaves_a_link_to_a_different_parent_alone() {
        let mut tree = VirtualTree::new();
        let real_parent = tree.create_node("div");
        let stranger = tree.create_node("div");
        let child = tree.create_node("span");
        tree.append_child(real_parent, child).unwrap();

        tree.remove_child(stranger, child).unwrap();

        assert_eq!(tree.get(child).unwrap().parent(), Some(real_parent));
    }

    #[test]
    fn detach_keeps_node_alive() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let child = tree.create_node("span");
        tree.append_child(parent, child).unwrap();

        tree.remove_child(parent, child).unwrap();

        assert!(tree.get(parent).unwrap().children().is_empty());
        assert!(
            tree.get(child).is_some(),
            "detaching a child must not deallocate it"
        );
    }

    #[test]
    fn remove_of_absent_child_is_a_no_op() {
        let mut tree = VirtualTree::new();
        let parent = tree.create_node("div");
        let never_appended = tree.create_node("span");

        // Absent child that exists elsewhere in the tree.
        assert!(tree.remove_child(parent, never_appended).is_ok());
        assert!(tree.get(parent).unwrap().children().is_empty());

        // Absent child id that was never allocated at all.
        assert!(tree.remove_child(parent, 9999).is_ok());
    }

    #[test]
    fn set_attribute_overwrites() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("input");

        tree.set_attribute(node, "value", "first").unwrap();
        tree.set_attribute(node, "value", "second").unwrap();

        assert_eq!(
            tree.get(node).unwrap().attributes().get("value"),
            Some(&AttributeValue::String("second".into()))
        );
    }

    #[test]
    fn unknown_id_lookup_returns_none() {
        let tree = VirtualTree::new();
        assert!(tree.get(42).is_none());
    }

    #[test]
    fn operations_on_unknown_ids_error_instead_of_panicking() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div");

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
    }

    #[test]
    fn set_style_is_independent_of_attributes() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div");

        tree.set_style(node, "color", "red").unwrap();

        let got = tree.get(node).unwrap();
        assert_eq!(
            got.style_props().get("color"),
            Some(&AttributeValue::String("red".into()))
        );
        assert!(got.attributes().get("color").is_none());
    }
}
