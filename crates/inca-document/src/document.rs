// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tree a document starts from, and the state that joins it to its style, its listeners and
//! its focus.

use std::collections::HashMap;

use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;

use crate::warnings::StyleWarnings;
use crate::{EventListeners, FocusModel, InputState, RevealQueue, Signals};

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
    /// Read through [`Self::tree`] and changed only through the operations of the document, each of
    /// which runs the steps that follow its change.
    pub(crate) tree: VirtualTree,
    /// The cascade of the tree, rebuilt when the tree changes. Read through [`Self::style`].
    pub(crate) style: StyleCache,
    /// The sink of the style warnings with the lines each node has printed. Changed only through
    /// the operations of the document.
    pub(crate) warnings: StyleWarnings,
    /// Read through [`Self::listeners`] and changed only through the operations of the document,
    /// which keep the links of the signals in step.
    pub(crate) listeners: EventListeners,
    /// The state of every `AbortSignal`, by id. Read through [`Self::signals`] and changed only
    /// through the operations of the document.
    pub(crate) signals: Signals,
    /// Read through [`Self::focus`] and changed only through the methods of the document. The
    /// tree holds the drawn focus, which [`Self::plan_focus_frame`] and
    /// [`Self::set_window_blurred`] publish. A script query reads [`Self::query_focus`].
    pub(crate) focus: FocusModel,
    /// The reveals that wait for a box: from a focus and from `scrollIntoView`.
    pub(crate) reveals: RevealQueue,
    /// The last number handed out as an `eventId`. Changed only through [`Self::next_event_id`].
    pub(crate) last_event_id: u64,
    /// The node a script asked to capture the mouse pointer. Its `gotpointercapture` fires before
    /// the next pointer event. Changed only through the capture requests of the document.
    pub(crate) capture_request: Option<NodeId>,
    /// The text states of the `input` nodes, created on first use. Read through [`Self::input`]
    /// and changed only through the operations of the document.
    pub(crate) inputs: HashMap<NodeId, InputState>,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::from_tree(new_document_tree())
    }
}

impl DocumentState {
    /// A document over `tree`, with no listener, signal, focus, reveal or text state yet. `tree`
    /// declares its root and its document, as [`new_document_tree`] builds them. The hover path,
    /// the pressed path and the drawn focus that `tree` holds are taken as they are.
    #[must_use]
    pub fn from_tree(tree: VirtualTree) -> Self {
        Self {
            tree,
            style: StyleCache::new(),
            warnings: StyleWarnings::default(),
            listeners: EventListeners::default(),
            signals: Signals::default(),
            focus: FocusModel::default(),
            reveals: RevealQueue::default(),
            last_event_id: 0,
            capture_request: None,
            inputs: HashMap::new(),
        }
    }

    /// The tree, for reads.
    #[must_use]
    pub fn tree(&self) -> &VirtualTree {
        &self.tree
    }

    /// The cascade of the tree, for reads.
    #[must_use]
    pub fn style(&self) -> &StyleCache {
        &self.style
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
