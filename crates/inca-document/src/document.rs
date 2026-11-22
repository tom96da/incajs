// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tree a document starts from, and the state that joins it to its style, its listeners and
//! its focus.

use std::cell::RefCell;
use std::collections::HashMap;

use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;

use crate::{EventListeners, FocusModel, InputState, Signals, StyleWarnings};

/// A tree with a root element under a document node. The root is the node a mounting app
/// attaches under.
///
/// # Panics
///
/// Panics if a new tree has no free node id.
#[must_use]
pub fn new_document_tree() -> VirtualTree {
    let mut tree = VirtualTree::new();
    let root = tree
        .create_node("div")
        .expect("a new tree has free node ids");
    let document = tree
        .create_node("#document")
        .expect("a new tree has free node ids");
    tree.append_child(document, root)
        .expect("a new document takes its first element");
    tree.set_root(root);
    tree.set_document(document);
    tree
}

/// The retained tree a document mutates, the cascade over it, the listeners registered on it and
/// the state its focus and text controls keep.
pub struct DocumentState {
    pub tree: VirtualTree,
    /// The cascade of the tree, rebuilt when the tree changes.
    pub style: StyleCache,
    /// Receives one line for each style a `setStyle` call stores with an unknown key, an invalid
    /// color or a value of a shape the document does not support. A node prints each line once.
    /// Writes to stderr by default.
    pub warnings: StyleWarnings,
    pub listeners: EventListeners,
    /// The state of every `AbortSignal`, by id. Plain data. The objects live in JS.
    pub signals: Signals,
    pub focus: FocusModel,
    /// Last number handed out as an `eventId`. Each event takes the next one.
    pub last_event_id: u64,
    /// The node a script asked to capture the mouse pointer. Its `gotpointercapture` fires before
    /// the next pointer event.
    pub capture_pending: Option<NodeId>,
    /// The text states of the `input` nodes, created on first use. Its own cell, as the draw runs
    /// under a shared borrow.
    pub inputs: RefCell<HashMap<NodeId, InputState>>,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            tree: new_document_tree(),
            style: StyleCache::new(),
            warnings: StyleWarnings::default(),
            listeners: EventListeners::default(),
            signals: Signals::default(),
            focus: FocusModel::default(),
            last_event_id: 0,
            capture_pending: None,
            inputs: RefCell::new(HashMap::new()),
        }
    }
}

impl std::fmt::Debug for DocumentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentState")
            .field("tree", &self.tree)
            .field("listeners", &self.listeners)
            .field("focus", &self.focus)
            .finish_non_exhaustive()
    }
}

impl DocumentState {
    /// The node a mounting app attaches under. Allocated with the tree, so `rootNodeId` always
    /// resolves.
    ///
    /// # Panics
    ///
    /// Panics if the tree has no root. [`DocumentState::default`] always declares one.
    #[must_use]
    pub fn root(&self) -> NodeId {
        self.tree.root().expect("the document declares a root")
    }

    /// The document, the parent of the node a mounting app attaches under.
    ///
    /// # Panics
    ///
    /// Panics if the tree has no document. [`DocumentState::default`] always declares one.
    #[must_use]
    pub fn document(&self) -> NodeId {
        self.tree
            .document()
            .expect("the document declares a document node")
    }

    /// The node events fall back to: the document element, else the document.
    #[must_use]
    pub fn top(&self) -> NodeId {
        self.tree
            .document_element()
            .unwrap_or_else(|| self.document())
    }

    /// Moves where Tab starts off `node_id`'s subtree to `before`, the node right before its
    /// place.
    pub fn move_tab_start(&mut self, node_id: NodeId, before: Option<NodeId>) {
        self.focus.start_leaves(&self.tree, node_id, before);
    }

    /// Queues a focus re-check of the node and its subtree.
    pub fn mark_focus_dirty(&mut self, node_id: NodeId) {
        self.focus.mark_subtree_dirty(&self.tree, node_id);
    }

    /// Writes the focus that style rules read from the focus model. Returns whether the value
    /// changed.
    pub fn sync_focus_style(&mut self) -> bool {
        let node = self.focus.visual_focus(&self.tree, &self.style);
        self.tree.set_focus(node)
    }

    /// Reports a style warning of `node_id` through the sink, once for each node and line.
    pub fn warn_style(&mut self, node_id: NodeId, line: &str) {
        self.warnings.report(&self.tree, node_id, line);
    }

    /// Drops the document state of freed nodes and returns the callback ids their listeners held.
    pub fn forget(&mut self, ids: &[NodeId]) -> Vec<u32> {
        for &id in ids {
            self.warnings.forget(id);
            self.inputs.get_mut().remove(&id);
            self.focus.forget(id);
        }
        self.listeners.release_nodes(ids)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn the_root_is_the_only_child_of_the_document() {
        let tree = new_document_tree();

        let (root, document) = (tree.root().unwrap(), tree.document().unwrap());

        assert_eq!(tree.get(document).unwrap().children(), [root]);
        assert_eq!(tree.get(root).unwrap().parent(), Some(document));
    }
}
