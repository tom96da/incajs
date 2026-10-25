// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Retained virtual tree.
//!
//! `VirtualTree` owns every `VirtualNode` and hands JS a [`NodeId`] for each.
//!
//! Two invariants the render path relies on: a node has at most one parent
//! (attaching it somewhere new detaches it from where it was), and an
//! attachment that would make a node its own ancestor is refused, so a walk
//! down the tree always terminates.
//!
//! A root can be declared with [`VirtualTree::set_root`].
//! [`VirtualTree::is_connected`] then reads the parent chain up to it.
//!
//! [`VirtualTree::remove_child`] detaches a node and keeps it with its
//! children.
//! [`VirtualTree::destroy_node`] frees a subtree on request.
//!
//! An orphan is a node with no parent that is neither the root nor held with
//! [`VirtualTree::hold`]. [`VirtualTree::sweep`] frees every orphan with its
//! subtree. A node that is created, detached or released is an orphan until
//! it is inserted, held or destroyed.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;

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

/// Whether a node of `tag` takes focus without a `tabindex`: a `button` or an
/// `input`.
#[must_use]
pub fn focusable_by_default(tag: &str) -> bool {
    matches!(tag, "button" | "input")
}

/// Whether `node` is a `button` or an `input` carrying the `disabled`
/// attribute.
#[must_use]
pub fn is_disabled_control(node: &VirtualNode) -> bool {
    focusable_by_default(node.tag_name()) && node.attributes().contains_key("disabled")
}

/// A single retained node. Always accessed through a [`VirtualTree`]. There
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
            TreeError::IdsExhausted => write!(f, "node id space exhausted"),
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
    held: HashSet<NodeId>,
    orphans: HashSet<NodeId>,
}

impl VirtualTree {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares `id` the root that [`is_connected`](Self::is_connected)
    /// measures against.
    pub fn set_root(&mut self, id: NodeId) {
        self.root = Some(id);
        self.orphans.remove(&id);
    }

    /// The root declared with [`set_root`](Self::set_root), if any.
    #[must_use]
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Keeps `id` and its subtree out of [`sweep`](Self::sweep) while it has
    /// no parent. Holding a held, attached or root node changes nothing
    /// visible. [`destroy_node`](Self::destroy_node) ends the hold.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn hold(&mut self, id: NodeId) -> Result<(), TreeError> {
        if !self.nodes.contains_key(&id) {
            return Err(TreeError::NodeNotFound(id));
        }
        self.held.insert(id);
        self.orphans.remove(&id);
        Ok(())
    }

    /// Ends the hold of `id`. A parentless node that is not the root becomes
    /// an orphan. An unknown or unheld `id` changes nothing.
    pub fn release(&mut self, id: NodeId) {
        if self.held.remove(&id) {
            self.orphan_if_free(id);
        }
    }

    /// Whether a [`sweep`](Self::sweep) would free a node.
    #[must_use]
    pub fn has_orphans(&self) -> bool {
        !self.orphans.is_empty()
    }

    /// Frees every orphan with its subtree, as
    /// [`destroy_node`](Self::destroy_node) does, and returns every freed id.
    pub fn sweep(&mut self) -> Vec<NodeId> {
        let mut orphans: Vec<NodeId> = self.orphans.drain().collect();
        orphans.sort_unstable();
        orphans
            .into_iter()
            .flat_map(|id| self.destroy_node(id))
            .collect()
    }

    /// Marks `id` an orphan when it is a parentless node, not the root and
    /// not held.
    fn orphan_if_free(&mut self, id: NodeId) {
        let free = self.nodes.get(&id).is_some_and(|n| n.parent.is_none())
            && self.root != Some(id)
            && !self.held.contains(&id);
        if free {
            self.orphans.insert(id);
        }
    }

    /// Whether `id` is the root or its parent chain reaches it. Always true
    /// while no root is declared. With a root, a destroyed or unknown id is
    /// not connected.
    #[must_use]
    pub fn is_connected(&self, id: NodeId) -> bool {
        let Some(root) = self.root else {
            return true;
        };
        let mut current = Some(id);
        while let Some(id) = current {
            if id == root {
                return self.nodes.contains_key(&id);
            }
            current = self.nodes.get(&id).and_then(|node| node.parent);
        }
        false
    }

    /// Allocates a new node and returns its id. Ids are assigned
    /// sequentially starting at 0 and are never reused, even after the node
    /// they named is destroyed, so a stale id never names another node.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::IdsExhausted`] once `u32::MAX` nodes have been
    /// created over the tree's lifetime.
    pub fn create_node(&mut self, tag_name: impl Into<String>) -> Result<NodeId, TreeError> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(TreeError::IdsExhausted)?;
        self.nodes.insert(id, VirtualNode::new(id, tag_name.into()));
        self.orphans.insert(id);
        Ok(id)
    }

    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&VirtualNode> {
        self.nodes.get(&id)
    }

    fn node_mut(&mut self, id: NodeId) -> Result<&mut VirtualNode, TreeError> {
        self.nodes.get_mut(&id).ok_or(TreeError::NodeNotFound(id))
    }

    /// `id` and its ancestors up to the root, nearest first. An id that names
    /// no node yields only itself.
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(Some(id), |&id| self.get(id).and_then(VirtualNode::parent))
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
    /// it was attached to first, so this both attaches and moves. A node
    /// never has two parents.
    ///
    /// If `anchor_id` is `child_id` itself and the child is already in
    /// `parent_id`, nothing changes.
    ///
    /// If `anchor_id` names a real node that isn't (or is no longer) among
    /// `parent_id`'s children, this falls back to appending at the end, same
    /// as an absent `remove_child` target. Only a truly
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

        // A child that is its own anchor stays where it is.
        if anchor_id == Some(child_id)
            && self
                .nodes
                .get(&child_id)
                .is_some_and(|c| c.parent == Some(parent_id))
        {
            return Ok(());
        }

        self.detach(child_id);

        let parent = self.node_mut(parent_id)?;
        let index = anchor_id
            .and_then(|anchor_id| parent.children.iter().position(|&id| id == anchor_id))
            .unwrap_or(parent.children.len());
        parent.children.insert(index, child_id);
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        self.orphans.remove(&child_id);
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
    /// node's id and nothing below it. Also ends the hold of every freed id.
    /// An unknown `id` frees nothing and returns an empty `Vec`: destroying
    /// twice is a no-op, not an error.
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
                self.held.remove(&current);
                self.orphans.remove(&current);
                freed.push(current);
            }
        }
        freed
    }

    /// Unlinks `child_id` from `parent_id`'s children, if present. `child_id`
    /// not being (or no longer being) a child of `parent_id` is a no-op, not
    /// an error. Only an unknown `parent_id` is. The child node keeps its own
    /// children and, unless held, is an orphan until it is inserted again.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `parent_id` names no node.
    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) -> Result<(), TreeError> {
        let parent = self.node_mut(parent_id)?;
        let before = parent.children.len();
        parent.children.retain(|&id| id != child_id);
        if parent.children.len() == before {
            return Ok(());
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = None;
        }
        self.orphan_if_free(child_id);
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
        let node = self.node_mut(node_id)?;
        node.attributes.insert(key.into(), value.into());
        Ok(())
    }

    /// Removes one attribute prop. Removing a key that isn't set is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn remove_attribute(&mut self, node_id: NodeId, key: &str) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        node.attributes.remove(key);
        Ok(())
    }

    /// Sets (inserting or overwriting) one style prop. `key` may be
    /// `camelCase`, `kebab-case` or `snake_case`, and all three name one prop.
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
        let node = self.node_mut(node_id)?;
        let key = key.into();
        let canonical = match style_key(&key) {
            Cow::Owned(k) => Some(k),
            Cow::Borrowed(_) => None,
        };
        node.style_props
            .insert(canonical.unwrap_or(key), value.into());
        Ok(())
    }

    /// Removes one style prop, so the property renders as if never set.
    /// `key` may be `camelCase`, `kebab-case` or `snake_case`. Removing a key
    /// that isn't set is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `node_id` names no node.
    pub fn remove_style(&mut self, node_id: NodeId, key: &str) -> Result<(), TreeError> {
        let node = self.node_mut(node_id)?;
        node.style_props.remove(&*style_key(key));
        Ok(())
    }
}

/// The `snake_case` spelling of a style key: each ASCII uppercase letter
/// becomes `_` and its lowercase form, and each `-` becomes `_`. A key that
/// is already `snake_case` is returned borrowed.
pub(crate) fn style_key(key: &str) -> Cow<'_, str> {
    if !key.bytes().any(|b| b.is_ascii_uppercase() || b == b'-') {
        return Cow::Borrowed(key);
    }
    let mut out = String::with_capacity(key.len() + 2);
    for c in key.chars() {
        match c {
            '-' => out.push('_'),
            c if c.is_ascii_uppercase() => {
                out.push('_');
                out.push(c.to_ascii_lowercase());
            }
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests;
