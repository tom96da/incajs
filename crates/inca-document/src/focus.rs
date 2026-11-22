// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which node holds focus, which focus and blur requests wait for the next frame, and where Tab
//! starts and moves.
//!
//! A focus or blur request can arrive while no window is at hand. The request is recorded here
//! and applied by [`FocusModel::apply_pending`], which the caller runs once per frame.
//!
//! Transitions come from comparing the node focused now with the node focused last frame. A
//! per-frame comparison sees a focus that moves in the same frame a listener is registered.

use std::collections::{HashSet, VecDeque};

use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;

mod requests;
mod start;
mod tab;

use start::Start;

/// The window side of the focus. It creates and drops the platform handle of a node, reports the
/// node the window focuses and moves the window's focus.
pub trait FocusSurface {
    /// The node whose handle the window focuses. None when the window focuses no node.
    fn focused(&self) -> Option<NodeId>;

    /// Gives `node` a handle to take focus with.
    fn track(&mut self, node: NodeId);

    /// Drops the handle of `node`.
    fn untrack(&mut self, node: NodeId);

    /// Moves the window's focus to the handle of `node`.
    fn focus(&mut self, node: NodeId);

    /// Releases the window's focus.
    fn blur(&mut self);

    /// Gives the window's focus to a handle that maps to no node while the window focuses
    /// nothing, so key events reach the root container.
    fn park(&mut self);
}

/// What changed since [`FocusModel::apply_pending`] was last called.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusTransition {
    blurred: Option<NodeId>,
    focused: Option<NodeId>,
}

impl FocusTransition {
    /// The window lost (`active` false) or regained the focus of `node`. The node keeps its focus
    /// in the tree. No other node is involved.
    #[must_use]
    pub fn window_activation(node: NodeId, active: bool) -> Self {
        Self {
            blurred: (!active).then_some(node),
            focused: active.then_some(node),
        }
    }

    /// The node that lost focus.
    #[must_use]
    pub fn blurred(&self) -> Option<NodeId> {
        self.blurred
    }

    /// The node that took focus.
    #[must_use]
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }
}

/// A focus or blur request left for the next frame to apply.
#[derive(Debug, Clone, Copy)]
enum PendingFocus {
    /// Focus the node, and scroll it into view when `scroll` is set.
    Focus {
        node: NodeId,
        scroll: bool,
    },
    Blur(NodeId),
}

/// The nodes that can take focus, the node focused as of the last frame this was checked, and
/// every pending request left by a caller that had no window at hand, oldest first. Two requests
/// queued before the next frame both apply, in order.
///
/// A tree change that takes focus from a node (see [`FocusModel::drop_focus_in`]) owes the window
/// a blur. The next [`FocusModel::apply_pending`] pays it.
#[derive(Debug, Default)]
pub struct FocusModel {
    // The nodes that have a handle on the window side.
    tracked: HashSet<NodeId>,
    focused: Option<NodeId>,
    // The focused node as the window last reported it. See `observe_node`.
    observed: Option<NodeId>,
    pending: VecDeque<PendingFocus>,
    // The node a tree change took focus from. The next `apply_pending`
    // releases the window's focus if it still rests on it.
    blur_owed: Option<NodeId>,
    // The latest focused node waiting to be scrolled into view, and the
    // node found lacking a box that waits for the next frame.
    reveal: Vec<NodeId>,
    retry: Vec<NodeId>,
    // Set while the window is inactive, after the focused node got its `blur`.
    window_blurred: bool,
    /// Nodes whose `tabindex` changed and wait for the next re-check.
    tab_dirty: HashSet<NodeId>,
    /// Where the next Tab starts while no node is focused.
    start: Option<Start>,
}

impl FocusModel {
    /// Queues the focus re-check the change of the attribute `key` of `node_id` needs.
    pub fn mark_attribute_dirty(&mut self, tree: &VirtualTree, node_id: NodeId, key: &str) {
        match key {
            "tabindex" | "disabled" | "href" | "xlink:href" => self.mark_tab_dirty(node_id),
            "hidden" | "inert" | "style" => self.mark_subtree_dirty(tree, node_id),
            _ => {}
        }
    }

    /// Whether a request, a changed `tabindex` or a blur owed by a tree change waits for
    /// [`Self::apply_pending`].
    #[must_use]
    pub fn has_queued_work(&self) -> bool {
        !self.pending.is_empty() || !self.tab_dirty.is_empty() || self.blur_owed.is_some()
    }

    /// The node holding focus when the window focuses `window_focus`. A node whose focus a tree
    /// change took is not held.
    #[must_use]
    pub fn held_node(&self, window_focus: Option<NodeId>) -> Option<NodeId> {
        window_focus.filter(|&id| self.blur_owed != Some(id))
    }

    /// Records the node the window focuses now. Call it before a read of [`Self::active_node`],
    /// so the read starts from the focus the window applied itself.
    pub fn observe_node(&mut self, window_focus: Option<NodeId>) {
        self.observed = self.held_node(window_focus);
    }

    /// The node holding focus once the queued requests apply. It starts from the node last
    /// observed, drops it when a queued check removes its focusability, then replays the queue in
    /// order.
    #[must_use]
    pub fn active_node(&self, tree: &VirtualTree, cache: &StyleCache) -> Option<NodeId> {
        let mut active = self
            .observed
            .filter(|&id| self.focusable_after_frame(tree, cache, id));
        for request in &self.pending {
            match *request {
                PendingFocus::Focus { node: id, .. } => {
                    if self.focusable_after_frame(tree, cache, id) {
                        active = Some(id);
                    }
                }
                PendingFocus::Blur(id) => {
                    if active == Some(id) {
                        active = None;
                    }
                }
            }
        }
        active
    }

    /// The node that style rules treat as focused: [`Self::active_node`], or none while the
    /// window is inactive.
    #[must_use]
    pub fn visual_focus(&self, tree: &VirtualTree, cache: &StyleCache) -> Option<NodeId> {
        self.active_node(tree, cache)
            .filter(|_| !self.window_blurred)
    }

    /// Records that the window lost (`blurred`) or regained focus after the focused node's `blur`
    /// or `focus` was reported.
    pub fn set_window_blurred(&mut self, blurred: bool) {
        self.window_blurred = blurred;
    }

    /// Drops `node_id` from the nodes that take focus, and any queued request to focus it. Call it
    /// for every node a destroyed subtree freed.
    pub fn forget(&mut self, node_id: NodeId) {
        self.tracked.remove(&node_id);
        self.tab_dirty.remove(&node_id);
        self.pending.retain(
            |request| !matches!(request, PendingFocus::Focus { node, .. } if *node == node_id),
        );
        self.reveal.retain(|&id| id != node_id);
        self.retry.retain(|&id| id != node_id);
        if self.focused == Some(node_id) {
            // The node is gone: nothing left to dispatch a "blur" to.
            self.focused = None;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::tree_with;
    use inca_node::AttributeValue;

    #[test]
    fn visual_focus_counts_a_queued_request() {
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusModel::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);

        assert_eq!(registry.visual_focus(&tree, &StyleCache::new()), Some(id));
    }

    #[test]
    fn visual_focus_is_none_while_the_window_is_inactive() {
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusModel::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);
        registry.set_window_blurred(true);

        assert_eq!(registry.visual_focus(&tree, &StyleCache::new()), None);
    }

    #[test]
    fn a_pending_blur_of_the_observed_node_clears_it() {
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusModel::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        registry.request_blur(id);
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), None);
    }

    #[test]
    fn a_pending_blur_of_another_node_keeps_the_observed_one() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let other = tree.create_node("div").unwrap();
        let mut registry = FocusModel::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        registry.request_blur(other);
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), Some(id));
    }

    #[test]
    fn a_pending_focus_on_an_unfocusable_node_changes_nothing() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let plain = tree.create_node("div").unwrap();
        let mut registry = FocusModel::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        registry.request_focus(plain, true);
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), Some(id));
    }

    #[test]
    fn a_tab_dirty_node_is_focusable_only_with_a_tab_index() {
        let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
        let plain = tree.create_node("div").unwrap();
        let mut registry = FocusModel::default();
        registry.mark_tab_dirty(focusable);
        registry.mark_tab_dirty(plain);
        registry.request_focus(plain, true);
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), None);
        registry.request_focus(focusable, true);
        assert_eq!(
            registry.active_node(&tree, &StyleCache::new()),
            Some(focusable)
        );
    }

    #[test]
    fn a_tab_dirty_node_that_loses_its_tab_index_drops_out_of_active() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusModel::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), Some(id));
        tree.remove_attribute(id, "tabindex").unwrap();
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), None);
    }

    #[rstest::rstest]
    #[case::tabindex("tabindex", true, false)]
    #[case::disabled("disabled", true, false)]
    #[case::href("href", true, false)]
    #[case::xlink_href("xlink:href", true, false)]
    #[case::hidden("hidden", false, true)]
    #[case::inert("inert", false, true)]
    #[case::style("style", false, true)]
    #[case::other("label", false, false)]
    fn an_attribute_change_queues_a_recheck_for_the_focus_attributes(
        #[case] key: &str,
        #[case] node_queued: bool,
        #[case] child_queued: bool,
    ) {
        let (mut tree, id) = tree_with("div", None);
        let child = tree.create_node("button").unwrap();
        tree.append_child(id, child).unwrap();
        let mut registry = FocusModel::default();

        registry.mark_attribute_dirty(&tree, id, key);

        let queued = registry.take_tab_dirty();
        assert_eq!(queued.contains(&id), node_queued);
        assert_eq!(queued.contains(&child), child_queued);
    }

    #[test]
    fn a_transition_names_its_nodes() {
        let lost = FocusTransition::window_activation(3, false);
        assert_eq!((lost.blurred(), lost.focused()), (Some(3), None));
        let regained = FocusTransition::window_activation(3, true);
        assert_eq!((regained.blurred(), regained.focused()), (None, Some(3)));
    }
}
