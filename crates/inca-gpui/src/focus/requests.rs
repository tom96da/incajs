// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Queued focus and blur requests, and the frame that applies them.

use super::{
    App, FocusRegistry, FocusTransition, NodeId, PendingFocus, Start, VirtualTree, Window,
};

impl FocusRegistry {
    /// Queues that `node_id` should be focused next frame. With `scroll`,
    /// the host then scrolls it into view.
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
    /// needs the resulting `track_focus` wiring is built.
    ///
    /// The transitions are returned and not dispatched here, so the caller
    /// fires them after releasing its borrow of `self`. An [`crate::EventSink`]
    /// that re-borrows the same `RefCell` while firing would otherwise
    /// panic with "already mutably borrowed".
    #[must_use]
    pub fn apply_pending(
        &mut self,
        tree: &VirtualTree,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<FocusTransition> {
        let transitions = self.apply_requests(tree, window, cx);
        let parked = self.parked.get_or_insert_with(|| cx.focus_handle());
        if window.focused(cx).is_none() {
            parked.focus(window, cx);
        }
        transitions
    }

    fn apply_requests(
        &mut self,
        tree: &VirtualTree,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<FocusTransition> {
        let mut transitions = Vec::new();
        if let Some(owed) = self.blur_owed.take()
            && self.window_node(window, cx) == Some(owed)
        {
            window.blur(cx);
        }
        for node_id in std::mem::take(&mut self.tab_dirty) {
            match Self::tab_index_of(tree, node_id) {
                Some(_) => {
                    self.get_or_create(node_id, cx);
                }
                None if !tree.is_connected(node_id) => {
                    // A node that left the tree loses focus without events.
                    let focused = self
                        .handles
                        .get(&node_id)
                        .is_some_and(|handle| handle.is_focused(window));
                    if focused {
                        window.blur(cx);
                    }
                    self.forget(node_id);
                }
                None => {
                    if let Some(handle) = self.handles.get(&node_id).cloned() {
                        // Report a click focus first, then the blur, so JS
                        // sees both. `forget` would skip the blur.
                        transitions.extend(self.sync(window, cx));
                        if handle.is_focused(window) {
                            window.blur(cx);
                        }
                        self.handles.remove(&node_id);
                        transitions.extend(self.sync(window, cx));
                    }
                }
            }
        }
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            // Nothing queued this frame doesn't mean focus didn't move:
            // GPUI's own click-to-focus moves it without ever going
            // through `request_focus`.
            transitions.extend(self.sync(window, cx));
            return transitions;
        }
        for request in pending {
            match request {
                PendingFocus::Focus { node, scroll } => {
                    // Only a node with a handle takes focus.
                    if let Some(handle) = self.handles.get(&node) {
                        handle.focus(window, cx);
                        // The last focus decides what scrolls.
                        self.reveal.clear();
                        self.retry.clear();
                        if scroll {
                            self.reveal.push(node);
                        }
                    }
                }
                PendingFocus::Blur(node_id) => {
                    let focused = window.focused(cx).and_then(|handle| self.node_for(&handle));
                    if focused == Some(node_id) {
                        window.blur(cx);
                    }
                }
            }
            transitions.extend(self.sync(window, cx));
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
    /// last call, including a change from GPUI's own click-to-focus, not
    /// only one made through [`Self::request_focus`]. A node that takes focus
    /// becomes where Tab starts, except the one the last press focused (see
    /// [`Self::press_start`]).
    fn sync(&mut self, window: &mut Window, cx: &mut App) -> Option<FocusTransition> {
        let focused_now = window.focused(cx).and_then(|handle| self.node_for(&handle));
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
    use crate::tests::*;
    use gpui::TestAppContext;
    use inca_node::AttributeValue;

    #[gpui::test]
    fn removing_tabindex_blurs_the_focused_node(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        let held = registry.handle(id).unwrap();
        assert!(cx.update(|window, _| held.is_focused(window)));

        tree.remove_attribute(id, "tabindex").unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(!cx.update(|window, _| held.is_focused(window)));
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred, Some(id));
        assert_eq!(transitions[0].focused, None);
    }

    #[gpui::test]
    fn removing_tabindex_blurs_a_node_a_click_focused_before_the_next_sync(
        cx: &mut TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        let held = registry.handle(id).unwrap();
        // Focus moved outside the registry, as a click does.
        cx.update(|window, cx| held.focus(window, cx));

        tree.remove_attribute(id, "tabindex").unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(!cx.update(|window, _| held.is_focused(window)));
        let events: Vec<_> = transitions.iter().map(|t| (t.blurred, t.focused)).collect();
        assert_eq!(events, [(None, Some(id)), (Some(id), None)]);
    }

    /// A root with `tabindex` takes focus like any node, and the parked focus
    /// leaves it alone.
    #[gpui::test]
    fn a_root_with_tabindex_still_focuses_normally(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, root) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(root);
        cx.update(|window, cx| {
            assert!(registry.apply_pending(&tree, window, cx).is_empty());
            registry.request_focus(root, true);
            let transitions = registry.apply_pending(&tree, window, cx);
            assert_eq!(transitions.len(), 1);
            assert_eq!(registry.focused_node(window, cx), Some(root));
            assert!(registry.apply_pending(&tree, window, cx).is_empty());
        });
    }

    #[gpui::test]
    fn a_disconnected_focused_node_loses_focus_without_a_transition(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(1);
        let id = ids[0];
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, id);
        let held = registry.handle(id).unwrap();
        assert!(cx.update(|window, _| held.is_focused(window)));

        tree.remove_child(ROOT, id).unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(transitions.is_empty());
        assert!(registry.handle(id).is_none());
        assert!(!cx.update(|window, _| held.is_focused(window)));
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            None
        );
    }

    #[gpui::test]
    fn a_connected_node_losing_tabindex_reports_the_blur(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(1);
        let id = ids[0];
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, id);

        tree.remove_attribute(id, "tabindex").unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred, Some(id));
    }

    #[gpui::test]
    fn a_pending_focus_takes_effect_after_a_silent_drop(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(2);
        let (gone, other) = (ids[0], ids[1]);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);

        tree.remove_child(ROOT, gone).unwrap();
        registry.mark_tab_dirty(gone);
        registry.request_focus(other, false);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            Some(other)
        );
        let events: Vec<_> = transitions.iter().map(|t| (t.blurred, t.focused)).collect();
        assert_eq!(events, [(None, Some(other))]);
    }

    #[gpui::test]
    fn dropping_focus_releases_the_window_at_the_next_apply(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let (gone, other) = (ids[0], ids[1]);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.request_focus(gone, false);

        registry.drop_focus_in(&tree, gone);

        let held = registry.handle(gone).unwrap();
        let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
        assert!(!cx.update(|window, _| held.is_focused(window)));
        assert!(registry.handle(other).is_some());
    }

    #[gpui::test]
    fn a_dropped_node_is_not_revealed_on_the_next_frame(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let (gone, kept) = (ids[0], ids[1]);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        registry.request_focus(gone, true);
        let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
        registry.retry_reveal(gone);
        registry.retry_reveal(kept);

        registry.drop_focus_in(&tree, gone);

        assert_eq!(registry.take_reveals(), (vec![kept], vec![]));
    }

    #[gpui::test]
    fn a_drop_on_an_inactive_window_leaves_nothing_owed_after_the_frame(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(1);
        let id = ids[0];
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, id);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.set_window_blurred(true);
        registry.drop_focus_in(&tree, id);

        let first = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
        registry.set_window_blurred(false);
        let held = registry.handle(id).unwrap();
        cx.update(|window, cx| held.focus(window, cx));
        let second = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(first.is_empty());
        let events: Vec<_> = second.iter().map(|t| (t.blurred, t.focused)).collect();
        assert_eq!(events, [(None, Some(id))]);
        assert!(cx.update(|window, _| held.is_focused(window)));
    }
}
