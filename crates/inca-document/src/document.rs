// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tree a document starts from, and the state that joins it to its style, its listeners and
//! its focus.

use std::cell::RefCell;
use std::collections::HashMap;

use inca_node::{NodeId, TreeError, VirtualTree};
use inca_style::{MatchFocus, StyleCache};

use crate::geometry::Align;
use crate::{
    EventListeners, FocusModel, FocusPlan, InputState, Reveal, RevealOrigin, RevealQueue, Reveals,
    Signals, StyleWarnings,
};

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
    /// The cascade of the tree, rebuilt when the tree changes.
    pub style: StyleCache,
    /// Receives one line for each style a `setStyle` call stores with an unknown key, an invalid
    /// color or a value of a shape the document does not support. A node prints each line once.
    /// Writes to stderr by default.
    pub warnings: StyleWarnings,
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
        Self::from_tree(new_document_tree())
    }
}

impl DocumentState {
    /// A document over `tree`, with no listener, signal, focus, reveal or text state yet. `tree`
    /// declares its root and its document, as [`new_document_tree`] builds them.
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
            capture_pending: None,
            inputs: RefCell::new(HashMap::new()),
        }
    }

    /// The tree, for reads.
    #[must_use]
    pub fn tree(&self) -> &VirtualTree {
        &self.tree
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
    /// Sets the typed text of the `input` node `node` to `value`, creating its text state on first
    /// use.
    pub fn set_input_value(&self, node: NodeId, value: &str) {
        self.inputs
            .borrow_mut()
            .entry(node)
            .or_default()
            .set_value(value);
    }

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

    /// Creates a node named `local_name`. See [`VirtualTree::create_node`].
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::create_node`].
    pub fn create_node(&mut self, local_name: impl Into<String>) -> Result<NodeId, TreeError> {
        self.tree.create_node(local_name)
    }

    /// Creates the element `local_name` in `namespace` with `prefix`. See
    /// [`VirtualTree::create_element_ns`].
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::create_element_ns`].
    pub fn create_element_ns(
        &mut self,
        namespace: Option<&str>,
        prefix: Option<&str>,
        local_name: impl Into<String>,
    ) -> Result<NodeId, TreeError> {
        self.tree.create_element_ns(namespace, prefix, local_name)
    }

    /// Copies `node_id`, with its descendants when `deep`, and returns the copy. An HTML `input`
    /// copy takes the typed text of its original.
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::clone_subtree`].
    pub fn clone_node(&mut self, node_id: NodeId, deep: bool) -> Result<NodeId, TreeError> {
        let made = self.tree.clone_subtree(node_id, deep)?;
        for &(original, copy) in &made {
            let is_input = self
                .tree
                .get(copy)
                .is_some_and(|node| node.is_html("input"));
            let typed = self
                .inputs
                .get_mut()
                .get(&original)
                .map(|input| input.value().to_owned());
            if let (true, Some(typed)) = (is_input, typed) {
                self.set_input_value(copy, &typed);
            }
        }
        made.first()
            .map(|&(_, copy)| copy)
            .ok_or(TreeError::NodeNotFound(node_id))
    }

    /// Keeps `node_id` from the sweep while it has no parent. See [`VirtualTree::hold`].
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::hold`].
    pub fn hold(&mut self, node_id: NodeId) -> Result<(), TreeError> {
        self.tree.hold(node_id)
    }

    /// Ends the hold of `node_id`. See [`VirtualTree::release`].
    pub fn release(&mut self, node_id: NodeId) {
        self.tree.release(node_id);
    }

    /// Replaces the data of the text node or comment `node_id`. See [`VirtualTree::set_data`].
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::set_data`].
    pub fn set_data(&mut self, node_id: NodeId, data: impl Into<String>) -> Result<(), TreeError> {
        self.tree.set_data(node_id, data)
    }

    /// Replaces `count` units of the data of `node_id` from `offset` with `data`. See
    /// [`VirtualTree::replace_data`].
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::replace_data`].
    pub fn replace_data(
        &mut self,
        node_id: NodeId,
        offset: u32,
        count: u32,
        data: &str,
    ) -> Result<(), TreeError> {
        self.tree.replace_data(node_id, offset, count, data)
    }

    /// Splits the text node `node_id` at `offset` and returns the new node with the rest. See
    /// [`VirtualTree::split_text`].
    ///
    /// # Errors
    ///
    /// The error of [`VirtualTree::split_text`].
    pub fn split_text(&mut self, node_id: NodeId, offset: u32) -> Result<NodeId, TreeError> {
        self.tree.split_text(node_id, offset)
    }

    /// Stores the hover path: the deepest hovered container and its ancestors. An empty path
    /// clears it. Returns whether the set changed.
    pub fn set_hover_path(&mut self, path: &[NodeId]) -> bool {
        self.tree.set_hover_path(path)
    }

    /// Stores the pressed path, like [`Self::set_hover_path`].
    pub fn set_active_path(&mut self, path: &[NodeId]) -> bool {
        self.tree.set_active_path(path)
    }

    /// Moves where Tab starts off `node_id`'s subtree to `before`, the node right before its
    /// place.
    pub(crate) fn move_tab_start(&mut self, node_id: NodeId, before: Option<NodeId>) {
        self.focus.start_leaves(&self.tree, node_id, before);
    }

    /// The focus model, for reads.
    #[must_use]
    pub fn focus(&self) -> &FocusModel {
        &self.focus
    }

    /// Queues that `node_id` should be focused next frame, scrolled into view with `scroll`. See
    /// [`FocusModel::request_focus`].
    pub fn request_focus(&mut self, node_id: NodeId, scroll: bool) {
        self.focus.request_focus(node_id, scroll);
    }

    /// Queues that `node_id` should be blurred next frame. See [`FocusModel::request_blur`].
    pub fn request_blur(&mut self, node_id: NodeId) {
        self.focus.request_blur(node_id);
    }

    /// Queues a re-check of the tab index of `node_id`.
    pub fn mark_tab_dirty(&mut self, node_id: NodeId) {
        self.focus.mark_tab_dirty(node_id);
    }

    /// Queues a focus re-check of the node and its subtree.
    pub fn mark_focus_dirty(&mut self, node_id: NodeId) {
        self.focus.mark_subtree_dirty(&self.tree, node_id);
    }

    /// Queues the focus re-check the change of the attribute `key` of `node_id` needs.
    pub(crate) fn mark_attribute_dirty(&mut self, node_id: NodeId, key: &str) {
        self.focus.mark_attribute_dirty(&self.tree, node_id, key);
    }

    /// Queues the focus re-check the write of the style prop `key` of `node_id` needs.
    pub(crate) fn mark_style_dirty(&mut self, node_id: NodeId, key: &str) {
        self.focus.mark_style_dirty(&self.tree, node_id, key);
    }

    /// Queues the focus re-check the write of the CSS property `name` of `node_id` needs.
    pub(crate) fn mark_property_dirty(&mut self, node_id: NodeId, name: &str) {
        self.focus.mark_property_dirty(&self.tree, node_id, name);
    }

    /// Takes the nodes that wait for a re-check of their tab index.
    #[cfg(any(test, feature = "test-support"))]
    pub fn take_tab_dirty(&mut self) -> std::collections::HashSet<NodeId> {
        self.focus.take_tab_dirty()
    }

    /// Records a mouse press on `node_id` as where Tab starts. See [`FocusModel::press_start`].
    pub fn press_start(&mut self, node_id: NodeId, root: NodeId, focusing: Option<NodeId>) {
        self.focus.press_start(node_id, root, focusing);
    }

    /// Records the node the window focuses now. See [`FocusModel::observe_node`].
    pub fn observe_focus(&mut self, platform: Option<NodeId>) {
        self.focus.observe_node(platform);
    }

    /// Records that the window lost (`blurred`) or regained focus, and publishes the drawn focus.
    /// Returns whether the drawn focus changed. See [`FocusModel::set_window_blurred`].
    pub fn set_window_blurred(&mut self, blurred: bool) -> bool {
        self.focus.set_window_blurred(blurred);
        self.publish_focus()
    }

    /// The focus a script query matches `:focus` and `:focus-within` against: the node that holds
    /// focus once the queued requests apply, or none while the window is inactive.
    #[must_use]
    pub fn query_focus(&self) -> MatchFocus {
        MatchFocus::Given(self.focus.visual_focus(&self.tree, &self.style))
    }

    /// Writes the drawn focus, the focus that style rules read, into the tree. Returns whether
    /// the value changed.
    fn publish_focus(&mut self) -> bool {
        let node = self.focus.visual_focus(&self.tree, &self.style);
        self.tree.set_focus(node)
    }

    /// Plans the focus of this frame from the queued requests, queues the reveal it asks for and
    /// publishes the drawn focus. See [`FocusModel::plan_frame`].
    pub fn plan_focus_frame(&mut self) -> FocusPlan {
        let plan = self.focus.plan_frame(&self.tree, &self.style);
        self.reveals.follow_plan(&plan);
        self.publish_focus();
        plan
    }

    /// Plans a Tab move from the document, `backward` or forward, and drops the reveals of earlier
    /// focus calls. Returns the node that takes focus. See [`FocusModel::plan_tab`].
    pub fn plan_tab(&mut self, backward: bool) -> (Option<NodeId>, FocusPlan) {
        let root = self.document();
        let (target, plan) = self.focus.plan_tab(&self.tree, &self.style, root, backward);
        self.reveals.follow_plan(&plan);
        (target, plan)
    }

    /// Takes focus from `node_id` and every node below it, with the reveals a focus asked for.
    /// See [`FocusModel::drop_focus_in`].
    pub fn drop_focus_in(&mut self, node_id: NodeId) {
        self.focus.drop_focus_in(&self.tree, node_id);
        self.reveals.drop_focus_in(&self.tree, node_id);
    }

    /// Queues a `scrollIntoView` of `node`, whose box the last frame lacks, for the next frame.
    pub fn queue_reveal(&mut self, node: NodeId, block: Align, inline: Align) {
        self.reveals.queue(Reveal {
            node,
            block,
            inline,
            origin: RevealOrigin::Script,
        });
    }

    /// Hands over the reveals of this frame.
    pub fn take_reveals(&mut self) -> Reveals {
        self.reveals.take()
    }

    /// Keeps `reveal`, whose node had no box, for one more frame.
    pub fn retry_reveal(&mut self, reveal: Reveal) {
        self.reveals.retry(reveal);
    }

    /// Reports a style warning of `node_id` through the sink, once for each node and line.
    pub fn warn_style(&mut self, node_id: NodeId, line: &str) {
        self.warnings.report(&self.tree, node_id, line);
    }

    /// Drops the document state of freed nodes and returns the callback ids their listeners held.
    pub(crate) fn forget(&mut self, ids: &[NodeId]) -> Vec<u32> {
        for &id in ids {
            self.warnings.forget(id);
            self.inputs.get_mut().remove(&id);
            self.focus.forget(id);
            self.reveals.forget(id);
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
