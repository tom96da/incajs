// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persists GPUI's per-node focus state across frames (`FocusHandle`s
//! don't come from the retained tree, so nothing else keeps them alive)
//! and turns GPUI's own focus changes into the four focus events.
//!
//! A focus or blur request can arrive outside any GPUI render or dispatch,
//! with no `Window`/`App` to act on immediately. The request is recorded here
//! and applied by [`FocusRegistry::apply_pending`], which the caller runs
//! once per frame before it builds the element tree.
//!
//! Transitions come from comparing the node focused now with the node
//! focused last frame. `Window::on_focus_in`/`on_focus_out` return a
//! `Subscription` whose activation is deferred (`cx.defer`) to after the
//! current update finishes, so one registered in the same frame focus moves
//! to it would miss that very transition. A per-frame diff has no such gap.

use std::collections::{HashMap, HashSet, VecDeque};

use gpui::{App, FocusHandle, Window};

use crate::tree::TEXT_TAG;
use crate::tree::js_number_text;
use crate::{
    AttributeValue, EventPayload, EventSink, NodeId, VirtualTree, focusable_by_default,
    hidden_by_attribute, is_disabled_control,
};

mod requests;
mod start;
mod tab;

use start::Start;

/// What changed since [`FocusRegistry::apply_pending`] was last called.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusTransition {
    blurred: Option<NodeId>,
    focused: Option<NodeId>,
}

impl FocusTransition {
    /// The window lost (`active` false) or regained the focus of `node`. The
    /// node keeps its focus in the tree. No other node is involved.
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

    /// Dispatches `blur`, `focusout`, `focus` and `focusin`, in that order,
    /// for whichever of `blurred`/`focused` this transition carries. Each
    /// carries the other node as `relatedTarget`. Call after releasing
    /// whatever borrow produced this transition. See
    /// [`FocusRegistry::apply_pending`].
    pub fn dispatch(
        &self,
        dispatch: &(impl EventSink + Clone + 'static),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(blurred) = self.blurred {
            let payload = EventPayload::Focus {
                related_target: self.focused,
            };
            for event in ["blur", "focusout"] {
                dispatch.fire(blurred, event, &payload, window, cx);
            }
        }
        if let Some(focused) = self.focused {
            let payload = EventPayload::Focus {
                related_target: self.blurred,
            };
            for event in ["focus", "focusin"] {
                dispatch.fire(focused, event, &payload, window, cx);
            }
        }
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

/// `NodeId` → its persistent `FocusHandle`, the node focused as of the last
/// frame this was checked, and every pending request left by a caller that
/// had no live `Window`/`App`, oldest first. See the module doc for why. Two
/// requests queued before the next frame both apply, in order.
///
/// A tree change that takes focus from a node (see
/// [`FocusRegistry::drop_focus_in`]) owes the window a blur. The next
/// [`FocusRegistry::apply_pending`] pays it.
#[derive(Debug, Default)]
pub struct FocusRegistry {
    handles: HashMap<NodeId, FocusHandle>,
    focused: Option<NodeId>,
    // The focused node as the window last reported it. See `observe`.
    observed: Option<NodeId>,
    // Holds the window's focus while no node does, so key events reach the
    // root container. It maps to no node.
    parked: Option<FocusHandle>,
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
    /// Nodes whose `tabindex` changed since the host last took them.
    tab_dirty: HashSet<NodeId>,
    /// Where the next Tab starts while no node is focused.
    start: Option<Start>,
}

impl FocusRegistry {
    /// The handle to `.track_focus(...)` `node_id` with, if it has one.
    /// This is the handle an [`EventSink`] returns from [`EventSink::focus_handle`].
    #[must_use]
    pub fn handle(&self, node_id: NodeId) -> Option<FocusHandle> {
        self.handles.get(&node_id).cloned()
    }

    /// Whether any node can take focus.
    #[must_use]
    pub fn has_handles(&self) -> bool {
        !self.handles.is_empty()
    }

    /// The node holding focus in `window` right now. A node whose focus a
    /// tree change took is not held.
    #[must_use]
    pub fn focused_node(&self, window: &Window, cx: &App) -> Option<NodeId> {
        self.window_node(window, cx)
            .filter(|&id| self.blur_owed != Some(id))
    }

    /// The node whose handle the window focuses, owed blur or not.
    fn window_node(&self, window: &Window, cx: &App) -> Option<NodeId> {
        window.focused(cx).and_then(|handle| self.node_for(&handle))
    }

    /// Records the node the window focuses now. Call it before a read of
    /// [`Self::active_node`], so the read starts from the focus GPUI applied
    /// itself.
    pub fn observe(&mut self, window: &Window, cx: &App) {
        self.observed = self.focused_node(window, cx);
    }

    /// The node holding focus once the queued requests apply. It starts from
    /// the node last observed, drops it when a queued check removes its
    /// focusability, then replays the queue in order.
    #[must_use]
    pub fn active_node(&self, tree: &VirtualTree) -> Option<NodeId> {
        let mut active = self
            .observed
            .filter(|&id| self.focusable_after_frame(tree, id));
        for request in &self.pending {
            match *request {
                PendingFocus::Focus { node: id, .. } => {
                    if self.focusable_after_frame(tree, id) {
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
    pub fn visual_focus(&self, tree: &VirtualTree) -> Option<NodeId> {
        self.active_node(tree).filter(|_| !self.window_blurred)
    }

    /// The handle the root container tracks, once [`Self::apply_pending`]
    /// has run. It holds the window's focus while no node is focused.
    #[must_use]
    pub fn parked_handle(&self) -> Option<FocusHandle> {
        self.parked.clone()
    }

    /// Records that the window lost (`blurred`) or regained focus after the
    /// focused node's `blur` or `focus` was reported.
    pub fn set_window_blurred(&mut self, blurred: bool) {
        self.window_blurred = blurred;
    }

    /// Drops `node_id`'s handle and any queued request to focus it.
    /// Call it for every node a destroyed subtree freed.
    pub fn forget(&mut self, node_id: NodeId) {
        self.handles.remove(&node_id);
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

    fn get_or_create(&mut self, node_id: NodeId, cx: &mut App) -> FocusHandle {
        self.handles
            .entry(node_id)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    fn node_for(&self, handle: &FocusHandle) -> Option<NodeId> {
        self.handles
            .iter()
            .find(|(_, h)| *h == handle)
            .map(|(&id, _)| id)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn changing_tabindex_keeps_the_focused_handle(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        let before = registry.handle(id).unwrap();

        tree.set_attribute(id, "tabindex", "-1").unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        let after = registry.handle(id).unwrap();
        assert_eq!(before, after);
        assert!(transitions.is_empty());
        assert!(cx.update(|window, _| after.is_focused(window)));
    }

    #[gpui::test]
    fn removing_tabindex_drops_the_focus_handle(cx: &mut TestAppContext) {
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
        let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(registry.handle(id).is_none());
    }

    #[gpui::test]
    fn focus_on_an_untracked_node_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", None);
        let mut registry = FocusRegistry::default();
        registry.request_focus(id, true);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
        assert!(transitions.is_empty());
        assert!(registry.handle(id).is_none());
    }

    #[gpui::test]
    fn forget_clears_a_queued_tab_check(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.forget(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        assert!(registry.handle(id).is_none());
    }

    #[test]
    fn visual_focus_counts_a_queued_request() {
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);

        assert_eq!(registry.visual_focus(&tree), Some(id));
    }

    #[test]
    fn visual_focus_is_none_while_the_window_is_inactive() {
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);
        registry.set_window_blurred(true);

        assert_eq!(registry.visual_focus(&tree), None);
    }

    #[gpui::test]
    fn get_or_create_reuses_the_same_handle_for_one_node(cx: &mut TestAppContext) {
        let mut registry = FocusRegistry::default();
        let cx = cx.add_empty_window();

        let (first, second) = cx.update(|_, cx| {
            let first = registry.get_or_create(1, cx);
            let second = registry.get_or_create(1, cx);
            (first, second)
        });

        assert_eq!(first, second);
    }

    #[gpui::test]
    fn forget_lets_a_later_call_mint_a_new_handle(cx: &mut TestAppContext) {
        let mut registry = FocusRegistry::default();
        let cx = cx.add_empty_window();

        let before = cx.update(|_, cx| registry.get_or_create(1, cx));
        registry.forget(1);
        let after = cx.update(|_, cx| registry.get_or_create(1, cx));

        assert_ne!(before, after);
        assert!(registry.handle(1).is_some());
    }

    #[gpui::test]
    fn focused_node_names_the_node_holding_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.focused_node(window, cx), None);
            registry.request_focus(id, true);
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.focused_node(window, cx), Some(id));
        });
    }

    #[gpui::test]
    fn observe_takes_the_node_the_window_focuses(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
            registry.handle(id).unwrap().focus(window, cx);
            registry.observe(window, cx);
        });
        assert_eq!(registry.active_node(&tree), Some(id));
    }

    #[gpui::test]
    fn a_pending_focus_on_a_node_with_a_handle_is_active(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        registry.request_focus(id, true);
        assert_eq!(registry.active_node(&tree), Some(id));
    }

    #[test]
    fn a_pending_blur_of_the_observed_node_clears_it() {
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        registry.request_blur(id);
        assert_eq!(registry.active_node(&tree), None);
    }

    #[test]
    fn a_pending_blur_of_another_node_keeps_the_observed_one() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let other = tree.create_node("div").unwrap();
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        registry.request_blur(other);
        assert_eq!(registry.active_node(&tree), Some(id));
    }

    #[test]
    fn a_pending_focus_on_an_unfocusable_node_changes_nothing() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let plain = tree.create_node("div").unwrap();
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        registry.request_focus(plain, true);
        assert_eq!(registry.active_node(&tree), Some(id));
    }

    #[test]
    fn a_tab_dirty_node_is_focusable_only_with_a_tab_index() {
        let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
        let plain = tree.create_node("div").unwrap();
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(focusable);
        registry.mark_tab_dirty(plain);
        registry.request_focus(plain, true);
        assert_eq!(registry.active_node(&tree), None);
        registry.request_focus(focusable, true);
        assert_eq!(registry.active_node(&tree), Some(focusable));
    }

    #[test]
    fn a_tab_dirty_node_that_loses_its_tab_index_drops_out_of_active() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.observed = Some(id);
        assert_eq!(registry.active_node(&tree), Some(id));
        tree.remove_attribute(id, "tabindex").unwrap();
        assert_eq!(registry.active_node(&tree), None);
    }

    /// Parking the window focus on the root container reports no transition,
    /// maps to no node and does not flip per frame.
    #[gpui::test]
    fn the_parked_focus_fires_no_focus_event(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, _root) = tree_with("div", None);
        let mut registry = FocusRegistry::default();
        cx.update(|window, cx| {
            for _ in 0..2 {
                let transitions = registry.apply_pending(&tree, window, cx);
                assert!(transitions.is_empty());
                assert!(window.focused(cx).is_some());
                assert_eq!(registry.focused_node(window, cx), None);
            }
        });
    }

    #[gpui::test]
    fn dropping_focus_is_silent(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let gone = ids[0];
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.request_focus(gone, false);

        registry.drop_focus_in(&tree, gone);

        assert_eq!(registry.active_node(&tree), None);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
        assert!(transitions.is_empty());
    }

    #[gpui::test]
    fn dropping_focus_in_an_unrelated_subtree_keeps_the_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[0]);
        cx.update(|window, cx| registry.observe(window, cx));

        registry.drop_focus_in(&tree, ids[1]);

        assert_eq!(registry.active_node(&tree), Some(ids[0]));
        let held = registry.handle(ids[0]).unwrap();
        let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
        assert!(cx.update(|window, _| held.is_focused(window)));
    }

    #[gpui::test]
    fn a_node_focused_elsewhere_after_a_drop_keeps_its_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let (gone, other) = (ids[0], ids[1]);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.drop_focus_in(&tree, gone);
        // Focus moved outside the registry, as a click does.
        let held = registry.handle(other).unwrap();
        cx.update(|window, cx| held.focus(window, cx));

        let _ = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(cx.update(|window, _| held.is_focused(window)));
    }

    #[test]
    fn a_transition_names_its_nodes() {
        let lost = FocusTransition::window_activation(3, false);
        assert_eq!((lost.blurred(), lost.focused()), (Some(3), None));
        let regained = FocusTransition::window_activation(3, true);
        assert_eq!((regained.blurred(), regained.focused()), (None, Some(3)));
    }
}
