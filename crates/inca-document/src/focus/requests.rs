// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Queued focus and blur requests, and the frame that applies them.

use super::{
    FocusModel, FocusSurface, FocusTransition, NodeId, PendingFocus, Start, StyleCache, VirtualTree,
};

impl FocusModel {
    /// Queues that `node_id` should be focused next frame. With `scroll`,
    /// the node is then scrolled into view.
    pub fn request_focus(&mut self, node_id: NodeId, scroll: bool) {
        self.pending.push_back(PendingFocus::Focus {
            node: node_id,
            scroll,
        });
    }

    /// Queues that `node_id` should be blurred next frame. Applying it
    /// blurs only if `node_id` is the focused node at that point.
    pub fn request_blur(&mut self, node_id: NodeId) {
        self.pending.push_back(PendingFocus::Blur(node_id));
    }

    /// Applies every queued request in order and reports each transition
    /// it produced. Call once per frame, before the element tree that
    /// needs the resulting handles is built.
    ///
    /// The transitions are returned and not dispatched here, so the caller
    /// fires them after releasing its borrow of `self`. A listener that
    /// re-borrows the same `RefCell` while firing would otherwise
    /// panic with "already mutably borrowed".
    #[must_use]
    pub fn apply_pending(
        &mut self,
        tree: &VirtualTree,
        cache: &StyleCache,
        surface: &mut impl FocusSurface,
    ) -> Vec<FocusTransition> {
        let transitions = self.apply_requests(tree, cache, surface);
        surface.park();
        transitions
    }

    fn apply_requests(
        &mut self,
        tree: &VirtualTree,
        cache: &StyleCache,
        surface: &mut impl FocusSurface,
    ) -> Vec<FocusTransition> {
        let mut transitions = Vec::new();
        if let Some(owed) = self.blur_owed.take()
            && surface.focused() == Some(owed)
        {
            surface.blur();
        }
        for node_id in std::mem::take(&mut self.tab_dirty) {
            match Self::tab_index_of(tree, cache, node_id) {
                Some(_) => {
                    self.tracked.insert(node_id);
                    surface.track(node_id);
                }
                None if !tree.is_connected(node_id) => {
                    // A node that left the tree loses focus without events.
                    if surface.focused() == Some(node_id) {
                        surface.blur();
                    }
                    self.forget(node_id);
                    surface.untrack(node_id);
                }
                None => {
                    if self.tracked.contains(&node_id) {
                        // Report a click focus first, then the blur, so JS
                        // sees both. `forget` would skip the blur.
                        transitions.extend(self.sync(surface));
                        if surface.focused() == Some(node_id) {
                            surface.blur();
                        }
                        self.tracked.remove(&node_id);
                        surface.untrack(node_id);
                        transitions.extend(self.sync(surface));
                    }
                }
            }
        }
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            // Nothing queued this frame doesn't mean focus didn't move:
            // the window's own click-to-focus moves it without ever going
            // through `request_focus`.
            transitions.extend(self.sync(surface));
            return transitions;
        }
        for request in pending {
            match request {
                PendingFocus::Focus { node, scroll } => {
                    // Only a node with a handle takes focus.
                    if self.tracked.contains(&node) {
                        surface.focus(node);
                        // The last focus decides what scrolls.
                        self.reveal.clear();
                        self.retry.clear();
                        if scroll {
                            self.reveal.push(node);
                        }
                    }
                }
                PendingFocus::Blur(node_id) => {
                    if surface.focused() == Some(node_id) {
                        surface.blur();
                    }
                }
            }
            transitions.extend(self.sync(surface));
        }
        transitions
    }

    /// Hands over the focused nodes to scroll into view this frame:
    /// `(retries, fresh)`. A retry is a node whose box the previous frame
    /// lacked.
    pub fn take_reveals(&mut self) -> (Vec<NodeId>, Vec<NodeId>) {
        (
            std::mem::take(&mut self.retry),
            std::mem::take(&mut self.reveal),
        )
    }

    /// Keeps `node_id` for one more frame.
    pub fn retry_reveal(&mut self, node_id: NodeId) {
        self.retry.push(node_id);
    }

    /// Reports a transition if which node is focused changed since the
    /// last call, including a change from the window's own click-to-focus, not
    /// only one made through [`Self::request_focus`]. A node that takes focus
    /// becomes where Tab starts, except the one the last press focused (see
    /// [`Self::press_start`]).
    fn sync(&mut self, surface: &impl FocusSurface) -> Option<FocusTransition> {
        let focused_now = surface.focused();
        self.observed = focused_now;
        if focused_now == self.focused {
            return None;
        }
        let blurred = self.focused;
        self.focused = focused_now;
        if let Some(node) = focused_now {
            self.start = match self.start {
                Some(Start::Press {
                    node: pressed,
                    focusing: Some(focusing),
                }) if focusing == node => Some(Start::Press {
                    node: pressed,
                    focusing: None,
                }),
                _ => Some(Start::Focus(node)),
            };
        }
        // An inactive window holds the events. Its deactivation already
        // blurred the old node and its activation focuses the new one.
        if self.window_blurred || (blurred.is_none() && focused_now.is_none()) {
            return None;
        }
        Some(FocusTransition {
            blurred,
            focused: focused_now,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{FakeSurface, rooted_tree};
    use std::collections::HashSet;

    fn applied(
        model: &mut FocusModel,
        tree: &VirtualTree,
        surface: &mut FakeSurface,
    ) -> Vec<(Option<NodeId>, Option<NodeId>)> {
        model
            .apply_pending(tree, &StyleCache::new(), surface)
            .iter()
            .map(|t| (t.blurred(), t.focused()))
            .collect()
    }

    fn tracked_pair() -> (VirtualTree, [NodeId; 2], FocusModel, FakeSurface) {
        let (tree, ids) = rooted_tree(2);
        let mut model = FocusModel::default();
        let mut surface = FakeSurface::default();
        model.mark_tab_dirty(ids[0]);
        model.mark_tab_dirty(ids[1]);
        assert!(applied(&mut model, &tree, &mut surface).is_empty());
        (tree, [ids[0], ids[1]], model, surface)
    }

    #[test]
    fn a_node_with_a_tab_index_gets_a_handle() {
        let (_tree, [first, second], _model, surface) = tracked_pair();

        assert_eq!(surface.handles, HashSet::from([first, second]));
    }

    #[test]
    fn a_frame_parks_the_window_focus() {
        let (_tree, _ids, _model, surface) = tracked_pair();

        assert_eq!(surface.parks, 1);
    }

    #[test]
    fn a_queued_focus_moves_the_window_focus_and_reports_it() {
        let (tree, [first, _], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);

        assert_eq!(
            applied(&mut model, &tree, &mut surface),
            [(None, Some(first))]
        );
        assert_eq!(surface.focus, Some(first));
    }

    #[test]
    fn a_queued_focus_on_a_node_without_a_handle_changes_nothing() {
        let (mut tree, _ids, mut model, mut surface) = tracked_pair();
        let plain = tree.create_node("div").unwrap();
        model.request_focus(plain, true);

        assert!(applied(&mut model, &tree, &mut surface).is_empty());
        assert_eq!(surface.focus, None);
        assert_eq!(model.take_reveals(), (vec![], vec![]));
    }

    #[test]
    fn two_queued_focuses_apply_in_order_and_report_each() {
        let (tree, [first, second], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);
        model.request_focus(second, false);

        assert_eq!(
            applied(&mut model, &tree, &mut surface),
            [(None, Some(first)), (Some(first), Some(second))]
        );
    }

    #[test]
    fn the_last_queued_focus_decides_what_scrolls() {
        let (tree, [first, second], mut model, mut surface) = tracked_pair();
        model.request_focus(first, true);
        model.request_focus(second, false);
        let _ = applied(&mut model, &tree, &mut surface);

        assert_eq!(model.take_reveals(), (vec![], vec![]));
    }

    #[test]
    fn a_queued_focus_with_scroll_hands_the_node_over_once() {
        let (tree, [first, _], mut model, mut surface) = tracked_pair();
        model.request_focus(first, true);
        let _ = applied(&mut model, &tree, &mut surface);

        assert_eq!(model.take_reveals(), (vec![], vec![first]));
        assert_eq!(model.take_reveals(), (vec![], vec![]));
    }

    #[test]
    fn a_reveal_retry_comes_back_as_a_retry() {
        let mut model = FocusModel::default();
        model.retry_reveal(4);

        assert_eq!(model.take_reveals(), (vec![4], vec![]));
    }

    #[test]
    fn a_queued_blur_releases_the_focused_node_and_reports_it() {
        let (tree, [first, _], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);
        let _ = applied(&mut model, &tree, &mut surface);
        model.request_blur(first);

        assert_eq!(
            applied(&mut model, &tree, &mut surface),
            [(Some(first), None)]
        );
        assert_eq!(surface.focus, None);
    }

    #[test]
    fn a_queued_blur_of_another_node_keeps_the_focus() {
        let (tree, [first, second], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);
        let _ = applied(&mut model, &tree, &mut surface);
        model.request_blur(second);

        assert!(applied(&mut model, &tree, &mut surface).is_empty());
        assert_eq!(surface.focus, Some(first));
    }

    #[test]
    fn a_focus_the_window_made_itself_is_reported_without_a_request() {
        let (tree, [first, _], mut model, mut surface) = tracked_pair();
        surface.focus = Some(first);

        assert_eq!(
            applied(&mut model, &tree, &mut surface),
            [(None, Some(first))]
        );
    }

    #[test]
    fn a_focus_change_while_the_window_is_inactive_reports_nothing() {
        let (tree, [first, _], mut model, mut surface) = tracked_pair();
        model.set_window_blurred(true);
        model.request_focus(first, false);

        assert!(applied(&mut model, &tree, &mut surface).is_empty());
        assert_eq!(surface.focus, Some(first));
    }

    #[test]
    fn a_focus_taken_by_a_tree_change_is_released_at_the_next_frame_silently() {
        let (tree, [first, _], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);
        let _ = applied(&mut model, &tree, &mut surface);
        model.drop_focus_in(&tree, first);

        let transitions = applied(&mut model, &tree, &mut surface);

        assert_eq!(surface.focus, None);
        assert!(transitions.is_empty());
    }

    #[test]
    fn a_blur_owed_for_a_node_the_window_left_is_not_paid() {
        let (tree, [first, second], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);
        let _ = applied(&mut model, &tree, &mut surface);
        model.drop_focus_in(&tree, first);
        surface.focus = Some(second);

        let _ = applied(&mut model, &tree, &mut surface);

        assert_eq!(surface.focus, Some(second));
    }

    #[test]
    fn a_node_that_loses_its_tab_index_is_blurred_and_loses_its_handle() {
        let (mut tree, [first, _], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);
        let _ = applied(&mut model, &tree, &mut surface);
        tree.remove_attribute(first, "tabindex").unwrap();
        model.mark_tab_dirty(first);

        let transitions = applied(&mut model, &tree, &mut surface);

        assert_eq!(transitions, [(Some(first), None)]);
        assert!(!surface.handles.contains(&first));
    }

    #[test]
    fn a_click_focus_before_the_recheck_is_reported_ahead_of_its_blur() {
        let (mut tree, [first, _], mut model, mut surface) = tracked_pair();
        surface.focus = Some(first);
        tree.remove_attribute(first, "tabindex").unwrap();
        model.mark_tab_dirty(first);

        assert_eq!(
            applied(&mut model, &tree, &mut surface),
            [(None, Some(first)), (Some(first), None)]
        );
    }

    #[test]
    fn a_disconnected_focused_node_is_blurred_without_a_transition() {
        let (mut tree, [first, _], mut model, mut surface) = tracked_pair();
        model.request_focus(first, false);
        let _ = applied(&mut model, &tree, &mut surface);
        tree.remove_child(crate::tests::ROOT, first).unwrap();
        model.mark_tab_dirty(first);

        let transitions = applied(&mut model, &tree, &mut surface);

        assert!(transitions.is_empty());
        assert_eq!(surface.focus, None);
        assert!(!surface.handles.contains(&first));
    }
}
