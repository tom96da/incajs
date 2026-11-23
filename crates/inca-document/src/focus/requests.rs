// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Queued focus and blur requests, and the plan of the frame that applies them.

use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;

use super::{FocusModel, FocusPlan, FocusReveal, FocusStep, FocusTransition, PendingFocus, Start};

/// A plan being built, with the platform focus it leaves after each step.
struct Draft {
    plan: FocusPlan,
    platform: Option<NodeId>,
}

impl Draft {
    fn focus(&mut self, node: NodeId) {
        self.plan.steps.push(FocusStep::Focus(node));
        self.platform = Some(node);
    }

    fn blur(&mut self) {
        self.plan.steps.push(FocusStep::Blur);
        self.platform = None;
    }

    /// Blurs when the platform focus rests on `node`.
    fn blur_if(&mut self, node: NodeId) {
        if self.platform == Some(node) {
            self.blur();
        }
    }
}

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

    /// Applies every queued request in order to the model, starting from the platform focus last
    /// observed, and returns what the window does and the transitions it reports. Call once per
    /// frame, after [`Self::observe_node`] and before the element tree that needs the resulting
    /// handles is built.
    ///
    /// The plan records the platform focus each step leaves: a focus of a node that takes focus
    /// moves it there, and a blur releases it. The window applies the steps in the same order.
    pub fn plan_frame(&mut self, tree: &VirtualTree, cache: &StyleCache) -> FocusPlan {
        let mut draft = Draft {
            plan: FocusPlan {
                park: true,
                ..FocusPlan::default()
            },
            platform: self.observed,
        };
        if let Some(owed) = self.blur_owed.take() {
            draft.blur_if(owed);
        }
        for node_id in std::mem::take(&mut self.tab_dirty) {
            match Self::tab_index_of(tree, cache, node_id) {
                Some(_) => {
                    self.tracked.insert(node_id);
                    draft.plan.gained.push(node_id);
                }
                None if !tree.is_connected(node_id) => {
                    // A node that left the tree loses focus without events.
                    draft.blur_if(node_id);
                    self.forget(node_id);
                    draft.plan.lost.push(node_id);
                }
                None => {
                    if self.tracked.contains(&node_id) {
                        // Report a click focus first, then the blur, so the
                        // caller sees both. `forget` would skip the blur.
                        self.sync(&mut draft);
                        draft.blur_if(node_id);
                        self.tracked.remove(&node_id);
                        draft.plan.lost.push(node_id);
                        self.sync(&mut draft);
                    }
                }
            }
        }
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            // Nothing queued this frame doesn't mean focus didn't move:
            // the window's own click-to-focus moves it without ever going
            // through `request_focus`.
            self.sync(&mut draft);
            return draft.plan;
        }
        for request in pending {
            match request {
                PendingFocus::Focus { node, scroll } => {
                    // Only a node that takes focus gets it.
                    if self.tracked.contains(&node) {
                        draft.focus(node);
                        draft.plan.reveal = FocusReveal::Replace(scroll.then_some(node));
                    }
                }
                PendingFocus::Blur(node_id) => draft.blur_if(node_id),
            }
            self.sync(&mut draft);
        }
        draft.plan
    }

    /// Reports a transition if which node is focused changed since the
    /// last call, including a change from the window's own click-to-focus, not
    /// only one made through [`Self::request_focus`]. A node that takes focus
    /// becomes where Tab starts, except the one the last press focused (see
    /// [`Self::press_start`]).
    fn sync(&mut self, draft: &mut Draft) {
        let focused_now = draft.platform;
        self.observed = focused_now;
        if focused_now == self.focused {
            return;
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
            return;
        }
        draft.plan.transitions.push(FocusTransition {
            blurred,
            focused: focused_now,
        });
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{FakeWindow, apply_frame, rooted_tree};
    use std::collections::HashSet;

    fn applied(
        model: &mut FocusModel,
        tree: &VirtualTree,
        surface: &mut FakeWindow,
    ) -> Vec<(Option<NodeId>, Option<NodeId>)> {
        apply_frame(model, tree, surface)
            .iter()
            .map(|t| (t.blurred(), t.focused()))
            .collect()
    }

    fn tracked_pair() -> (VirtualTree, [NodeId; 2], FocusModel, FakeWindow) {
        let (tree, ids) = rooted_tree(2);
        let mut model = FocusModel::default();
        let mut surface = FakeWindow::default();
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
    fn a_node_that_gains_its_tab_index_is_focused_in_the_same_apply() {
        let (tree, ids) = rooted_tree(1);
        let mut model = FocusModel::default();
        let mut surface = FakeWindow::default();
        model.mark_tab_dirty(ids[0]);
        model.request_focus(ids[0], false);

        assert_eq!(
            applied(&mut model, &tree, &mut surface),
            [(None, Some(ids[0]))]
        );
        assert_eq!(surface.focus, Some(ids[0]));
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
        assert_eq!(surface.take_reveals(), (vec![], vec![]));
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

        assert_eq!(surface.take_reveals(), (vec![], vec![]));
    }

    #[test]
    fn a_queued_focus_with_scroll_hands_the_node_over_once() {
        let (tree, [first, _], mut model, mut surface) = tracked_pair();
        model.request_focus(first, true);
        let _ = applied(&mut model, &tree, &mut surface);

        assert_eq!(surface.take_reveals(), (vec![], vec![first]));
        assert_eq!(surface.take_reveals(), (vec![], vec![]));
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
