// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The focus model: the node that holds focus, the queued requests it replays, the transitions
//! it reports and the focus a tree change takes, with the window side it drives.

use std::collections::{HashSet, VecDeque};

use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;

use super::Start;

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
    pub(super) blurred: Option<NodeId>,
    pub(super) focused: Option<NodeId>,
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
pub(super) enum PendingFocus {
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
    pub(super) tracked: HashSet<NodeId>,
    pub(super) focused: Option<NodeId>,
    // The focused node as the window last reported it. See `observe_node`.
    pub(super) observed: Option<NodeId>,
    pub(super) pending: VecDeque<PendingFocus>,
    // The node a tree change took focus from. The next `apply_pending`
    // releases the window's focus if it still rests on it.
    pub(super) blur_owed: Option<NodeId>,
    // The latest focused node waiting to be scrolled into view, and the
    // node found lacking a box that waits for the next frame.
    pub(super) reveal: Vec<NodeId>,
    pub(super) retry: Vec<NodeId>,
    // Set while the window is inactive, after the focused node got its `blur`.
    pub(super) window_blurred: bool,
    /// Nodes whose `tabindex` changed and wait for the next re-check.
    pub(super) tab_dirty: HashSet<NodeId>,
    /// Where the next Tab starts while no node is focused.
    pub(super) start: Option<Start>,
}

impl FocusModel {
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

    /// Takes focus from `node_id` and from every node below it at once. No
    /// `blur` or `focusout` is reported, and requests to focus those nodes
    /// are dropped. The window's own focus is released by the next
    /// [`Self::apply_pending`].
    pub fn drop_focus_in(&mut self, tree: &VirtualTree, node_id: NodeId) {
        let inside = |id: NodeId| tree.ancestors(id).any(|ancestor| ancestor == node_id);
        self.pending.retain(
            |request| !matches!(request, PendingFocus::Focus { node, .. } if inside(*node)),
        );
        self.reveal.retain(|&id| !inside(id));
        self.retry.retain(|&id| !inside(id));
        if let Some(held) = self.observed.filter(|&id| inside(id)) {
            self.observed = None;
            self.blur_owed = Some(held);
        }
        if self.focused.is_some_and(inside) {
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
    fn a_transition_names_its_nodes() {
        let lost = FocusTransition::window_activation(3, false);
        assert_eq!((lost.blurred(), lost.focused()), (Some(3), None));
        let regained = FocusTransition::window_activation(3, true);
        assert_eq!((regained.blurred(), regained.focused()), (None, Some(3)));
    }
}
