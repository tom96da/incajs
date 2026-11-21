// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test helpers that the tests of several modules share.

#![allow(clippy::unwrap_used)]

use crate::{FocusSurface, new_document_tree};
use inca_node::tree::{AttributeValue, NodeId, VirtualTree};
use std::collections::HashSet;

/// The root of a tree `rooted_tree` builds: the first node created. The document is node 1.
pub(crate) const ROOT: NodeId = 0;

/// A tree whose document holds the root, with `count` nodes attached under the root, each
/// `tabindex="0"`.
pub(crate) fn rooted_tree(count: usize) -> (VirtualTree, Vec<NodeId>) {
    let mut tree = new_document_tree();
    let root = tree.root().unwrap();
    let ids = (0..count)
        .map(|_| {
            let id = tree.create_node("div").unwrap();
            tree.set_attribute(id, "tabindex", "0").unwrap();
            tree.append_child(root, id).unwrap();
            id
        })
        .collect();
    (tree, ids)
}

pub(crate) fn tree_with(tag: &str, tabindex: Option<AttributeValue>) -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node(tag).unwrap();
    if let Some(value) = tabindex {
        tree.set_attribute(id, "tabindex", value).unwrap();
    }
    (tree, id)
}

/// A window side that records what the focus model asks of it.
#[derive(Default)]
pub(crate) struct FakeSurface {
    pub(crate) handles: HashSet<NodeId>,
    pub(crate) focus: Option<NodeId>,
    pub(crate) parks: usize,
}

impl FocusSurface for FakeSurface {
    fn focused(&self) -> Option<NodeId> {
        self.focus
    }

    fn track(&mut self, node: NodeId) {
        self.handles.insert(node);
    }

    fn untrack(&mut self, node: NodeId) {
        self.handles.remove(&node);
    }

    fn focus(&mut self, node: NodeId) {
        if self.handles.contains(&node) {
            self.focus = Some(node);
        }
    }

    fn blur(&mut self) {
        self.focus = None;
    }

    fn park(&mut self) {
        self.parks += 1;
    }
}
