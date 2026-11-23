// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tree a document starts from, and the state that joins it to its style, its listeners and
//! its focus.

use std::collections::HashMap;
use std::rc::Rc;

use inca_node::{NodeId, TreeError, VirtualTree};
use inca_style::{MatchFocus, StyleCache};

use crate::geometry::Align;
use crate::warnings::StyleWarnings;
use crate::{
    EventListeners, FocusModel, FocusPlan, InputState, Reveal, RevealOrigin, RevealQueue, Reveals,
    Signals, View,
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
    /// The sink of the style warnings with the lines each node has printed. Changed only through
    /// the operations of the document.
    warnings: StyleWarnings,
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
    /// the next pointer event. Changed only through the capture requests of the document.
    capture_request: Option<NodeId>,
    /// The text states of the `input` nodes, created on first use. Read through [`Self::input`]
    /// and changed only through the operations of the document.
    inputs: HashMap<NodeId, InputState>,
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
            capture_request: None,
            inputs: HashMap::new(),
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
    /// The text state of the `input` node `node`, `None` before its first use.
    #[must_use]
    pub fn input(&self, node: NodeId) -> Option<&InputState> {
        self.inputs.get(&node)
    }

    /// Sets the typed text of the `input` node `node` to `value`, creating its text state on first
    /// use.
    pub fn set_input_value(&mut self, node: NodeId, value: &str) {
        self.edit_input(node, |input| input.set_value(value));
    }

    /// Runs `edit` on the text state of the `input` node `node`, created on first use, and returns
    /// what `edit` returns.
    pub fn edit_input<R>(&mut self, node: NodeId, edit: impl FnOnce(&mut InputState) -> R) -> R {
        edit(self.inputs.entry(node).or_default())
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
            let typed = self.input(original).map(|input| input.value().to_owned());
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

    /// Asks for the capture of the mouse pointer for `node`, which starts at the next pointer
    /// event. The request is kept only while a button of `view` is held and `node` is connected.
    /// Returns whether `node` is connected.
    #[must_use]
    pub fn request_capture(&mut self, view: &dyn View, node: NodeId) -> bool {
        let connected = self.tree.is_connected(node);
        if connected && view.held_buttons() != 0 {
            self.capture_request = Some(node);
        }
        connected
    }

    /// Drops the capture request when it names `node`.
    pub fn release_capture_request(&mut self, node: NodeId) {
        if self.capture_request == Some(node) {
            self.capture_request = None;
        }
    }

    /// Whether the capture request names `node` and `node` is connected.
    #[must_use]
    pub fn has_capture_request(&self, node: NodeId) -> bool {
        self.capture_request == Some(node) && self.tree.is_connected(node)
    }

    /// The node the capture request names, for the window to apply. A request whose node is not
    /// connected is dropped and returns `None`.
    pub fn capture_request(&mut self) -> Option<NodeId> {
        let tree = &self.tree;
        self.capture_request = self.capture_request.filter(|&node| tree.is_connected(node));
        self.capture_request
    }

    /// Drops the capture request.
    pub fn cancel_capture_request(&mut self) {
        self.capture_request = None;
    }

    /// Replaces the sink that receives the style warning lines. The default sink writes to stderr.
    pub fn set_warning_sink(&mut self, sink: Rc<dyn Fn(&str)>) {
        self.warnings.set_sink(sink);
    }

    /// Sends the sink a line for each warning of the `<style>` sheets parsed since the last call,
    /// as `<style>: <warning>`.
    pub fn report_sheet_warnings(&mut self) {
        for warning in self.style.take_style_warnings(&self.tree) {
            self.warnings.report_sheet(&warning);
        }
    }

    /// Sets a sink that keeps each style warning line in the returned list.
    #[cfg(any(test, feature = "test-support"))]
    pub fn record_warnings(&mut self) -> Rc<std::cell::RefCell<Vec<String>>> {
        let lines = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = Rc::clone(&lines);
        self.set_warning_sink(Rc::new(move |line| sink.borrow_mut().push(line.to_owned())));
        lines
    }

    /// Reports a style warning of `node_id` through the sink, once for each node and line.
    pub(crate) fn warn_style(&mut self, node_id: NodeId, line: &str) {
        self.warnings.report(&self.tree, node_id, line);
    }

    /// Drops the document state of freed nodes and returns the callback ids their listeners held.
    pub(crate) fn forget(&mut self, ids: &[NodeId]) -> Vec<u32> {
        for &id in ids {
            self.warnings.forget(id);
            self.inputs.remove(&id);
            self.focus.forget(id);
            self.reveals.forget(id);
        }
        self.listeners.release_nodes(ids)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::RecordingView;

    /// A document with one node under its root, and a view holding the left button.
    fn mounted() -> (DocumentState, RecordingView, NodeId) {
        let view = RecordingView {
            held: 1,
            ..RecordingView::default()
        };
        let mut document = DocumentState::default();
        let node = document.create_node("div").unwrap();
        document.append_child(&view, document.root(), node).unwrap();
        (document, view, node)
    }

    #[rstest]
    #[case::held(1, true, (true, true))]
    #[case::no_button(0, true, (true, false))]
    #[case::detached(1, false, (false, false))]
    fn a_capture_request_is_kept_for_a_connected_node_while_a_button_is_held(
        #[case] held: u8,
        #[case] attached: bool,
        #[case] expected: (bool, bool),
    ) {
        let (mut document, mut view, mut node) = mounted();
        view.held = held;
        if !attached {
            node = document.create_node("div").unwrap();
        }

        let connected = document.request_capture(&view, node);

        assert_eq!((connected, document.has_capture_request(node)), expected);
    }

    #[test]
    fn a_capture_release_drops_only_the_request_of_its_node() {
        let (mut document, view, node) = mounted();
        let other = document.create_node("div").unwrap();
        document
            .append_child(&view, document.root(), other)
            .unwrap();
        assert!(document.request_capture(&view, node));

        document.release_capture_request(other);
        let kept = document.has_capture_request(node);
        document.release_capture_request(node);

        assert_eq!((kept, document.has_capture_request(node)), (true, false));
    }

    #[test]
    fn a_capture_request_of_a_node_that_left_the_tree_is_dropped() {
        let (mut document, view, node) = mounted();
        assert!(document.request_capture(&view, node));
        let root = document.root();
        document.remove_child(&view, root, node).unwrap();
        let removed = (
            document.has_capture_request(node),
            document.capture_request(),
        );

        document.append_child(&view, root, node).unwrap();

        assert_eq!(removed, (false, None));
        assert_eq!(document.capture_request(), None);
    }

    #[test]
    fn the_sheet_warnings_since_the_last_report_reach_the_sink() {
        let (mut document, view, _) = mounted();
        let lines = document.record_warnings();
        let style = document.create_node("style").unwrap();
        let text = document.create_node("#text").unwrap();
        document.set_data(text, ".a { bogus: 1 }").unwrap();
        document.append_child(&view, style, text).unwrap();
        document
            .append_child(&view, document.root(), style)
            .unwrap();

        document.report_sheet_warnings();

        assert_eq!(
            *lines.borrow(),
            ["<style>: ignoring unknown style key `bogus`"]
        );
    }

    #[test]
    fn a_freed_node_drops_the_style_lines_it_reported() {
        let (mut document, mut view, node) = mounted();
        document.set_style_prop(node, "bogus", "1").unwrap();
        let reported = !document.warnings.is_empty();

        let _released = document.destroy_node(&mut view, node);

        assert_eq!((reported, document.warnings.is_empty()), (true, true));
    }

    #[test]
    fn the_root_is_the_only_child_of_the_document() {
        let tree = new_document_tree();

        let (root, document) = (tree.root().unwrap(), tree.document().unwrap());

        assert_eq!(tree.get(document).unwrap().children(), [root]);
        assert_eq!(tree.get(root).unwrap().parent(), Some(document));
    }
}
