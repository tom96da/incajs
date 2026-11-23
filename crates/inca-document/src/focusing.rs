// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The operations of the document on its focus and its reveals: the focus and blur requests, the
//! re-checks a change queues, the plan of each frame and of a Tab move, and the reveals that wait
//! for a box.

use inca_node::NodeId;
use inca_style::MatchFocus;

use crate::geometry::Align;
use crate::{DocumentState, FocusModel, FocusPlan, Reveal, RevealOrigin, Reveals};

impl DocumentState {
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

    /// Queues a focus re-check of the node and its subtree.
    pub(crate) fn mark_focus_dirty(&mut self, node_id: NodeId) {
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
}
