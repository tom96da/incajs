// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The focus handle of each node that can take focus, and the focus events of a transition.

use std::collections::HashMap;

use gpui::{App, FocusHandle, Window};

use inca_document::{EventPayload, FocusPlan, FocusStep, FocusTransition};
use inca_gpui::EventSink;
use inca_node::NodeId;

/// Dispatches `blur`, `focusout`, `focus` and `focusin`, in that order, for whichever of
/// `blurred`/`focused` `transition` carries. Each carries the other node as `relatedTarget`. Call
/// after releasing whatever borrow produced the transition. See
/// [`crate::Cells::apply_focus`].
pub fn dispatch_transition(
    transition: &FocusTransition,
    dispatch: &(impl EventSink + Clone + 'static),
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(blurred) = transition.blurred() {
        let payload = EventPayload::Focus {
            related_target: transition.focused(),
        };
        for event in ["blur", "focusout"] {
            dispatch.fire(blurred, event, &payload, window, cx);
        }
    }
    if let Some(focused) = transition.focused() {
        let payload = EventPayload::Focus {
            related_target: transition.blurred(),
        };
        for event in ["focus", "focusin"] {
            dispatch.fire(focused, event, &payload, window, cx);
        }
    }
}

/// The `FocusHandle` of each node that can take focus. Only [`Self::apply`] writes the handles,
/// and a freed node drops its handle.
#[derive(Debug, Default)]
pub struct FocusRegistry {
    pub(super) handles: HashMap<NodeId, FocusHandle>,
    // Holds the window's focus while no node does, so key events reach the
    // root container. It maps to no node.
    pub(super) parked: Option<FocusHandle>,
}

impl FocusRegistry {
    /// Applies `plan` to the handles and to the focus of `window`: creates the handle of each
    /// gained node that has none, runs the steps, drops the handles of the lost nodes and parks.
    pub fn apply(&mut self, plan: &FocusPlan, window: &mut Window, cx: &mut App) {
        for &node in &plan.gained {
            get_or_create(&mut self.handles, node, cx);
        }
        for step in &plan.steps {
            match *step {
                FocusStep::Focus(node) => {
                    let handle = self.handles.get(&node);
                    // A plan focuses only a node it tracks, and each tracked node has a handle.
                    debug_assert!(
                        handle.is_some(),
                        "the focus plan names {node}, which has no handle"
                    );
                    if let Some(handle) = handle {
                        handle.focus(window, cx);
                    }
                }
                FocusStep::Blur => window.blur(cx),
            }
        }
        for node in &plan.lost {
            self.handles.remove(node);
        }
        if plan.park {
            let parked = self.parked.get_or_insert_with(|| cx.focus_handle());
            if window.focused(cx).is_none() {
                parked.focus(window, cx);
            }
        }
    }
}

fn get_or_create(
    handles: &mut HashMap<NodeId, FocusHandle>,
    node_id: NodeId,
    cx: &mut App,
) -> FocusHandle {
    handles
        .entry(node_id)
        .or_insert_with(|| cx.focus_handle())
        .clone()
}

fn node_for(handles: &HashMap<NodeId, FocusHandle>, handle: &FocusHandle) -> Option<NodeId> {
    handles
        .iter()
        .find(|(_, h)| *h == handle)
        .map(|(&id, _)| id)
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

    /// The node whose handle `window` focuses right now.
    #[must_use]
    pub fn focused_node(&self, window: &Window, cx: &App) -> Option<NodeId> {
        node_for(&self.handles, &window.focused(cx)?)
    }

    /// The handle the root container tracks, once a frame [`Self::apply`] has
    /// run. It holds the window's focus while no node is focused.
    #[must_use]
    pub fn parked_handle(&self) -> Option<FocusHandle> {
        self.parked.clone()
    }

    /// Drops `node_id`'s handle. Call it for every node a destroyed subtree freed, with the
    /// `forget` of the document.
    pub(crate) fn forget(&mut self, node_id: NodeId) {
        self.handles.remove(&node_id);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::focus::Registry;
    use crate::tests::*;
    use gpui::TestAppContext;
    use inca_node::AttributeValue;
    use inca_style::StyleCache;
    use inca_test_macros::gpui_cases;

    #[gpui::test]
    fn changing_tabindex_keeps_the_focused_handle(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
        });
        let before = registry.handle(id).unwrap();

        tree.set_attribute(id, "tabindex", "-1").unwrap();
        registry.mark_tab_dirty(id);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        let after = registry.handle(id).unwrap();
        assert_eq!(before, after);
        assert!(transitions.is_empty());
        assert!(cx.update(|window, _| after.is_focused(window)));
    }

    #[gpui::test]
    fn removing_tabindex_drops_the_focus_handle(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, true);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
        });
        let held = registry.handle(id).unwrap();
        assert!(cx.update(|window, _| held.is_focused(window)));

        tree.remove_attribute(id, "tabindex").unwrap();
        registry.mark_tab_dirty(id);
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(registry.handle(id).is_none());
    }

    #[gpui::test]
    fn focus_on_an_untracked_node_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", None);
        let mut registry = Registry::default();
        registry.request_focus(id, true);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
        assert!(transitions.is_empty());
        assert!(registry.handle(id).is_none());
    }

    #[gpui::test]
    fn a_node_that_gains_its_tab_index_and_a_focus_in_one_frame_takes_it(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(1);
        let mut registry = Registry::default();
        registry.mark_tab_dirty(ids[0]);
        registry.request_focus(ids[0], false);

        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        let events: Vec<_> = transitions
            .iter()
            .map(|t| (t.blurred(), t.focused()))
            .collect();
        assert_eq!(events, [(None, Some(ids[0]))]);
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            Some(ids[0])
        );
    }

    #[gpui::test]
    fn a_node_removed_and_added_again_before_a_frame_keeps_its_handle_and_focus(
        cx: &mut TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(1);
        let id = ids[0];
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, id);
        let before = registry.handle(id).unwrap();

        tree.remove_child(ROOT, id).unwrap();
        tree.append_child(ROOT, id).unwrap();
        registry.mark_tab_dirty(id);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(transitions.is_empty());
        assert_eq!(registry.handle(id), Some(before.clone()));
        assert!(cx.update(|window, _| before.is_focused(window)));
    }

    #[gpui::test]
    fn a_mixed_frame_leaves_a_handle_for_exactly_the_nodes_that_take_focus(
        cx: &mut TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(5);
        let [kept, lost, gone, gained, never] = [ids[0], ids[1], ids[2], ids[3], ids[4]];
        tree.remove_attribute(gained, "tabindex").unwrap();
        tree.remove_attribute(never, "tabindex").unwrap();
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, kept);

        tree.remove_attribute(lost, "tabindex").unwrap();
        tree.remove_child(ROOT, gone).unwrap();
        tree.set_attribute(gained, "tabindex", "0").unwrap();
        for id in ids.iter().copied() {
            registry.mark_tab_dirty(id);
        }
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        let with_handle: Vec<bool> = [kept, lost, gone, gained, never]
            .iter()
            .map(|&id| registry.handle(id).is_some())
            .collect();
        assert_eq!(with_handle, [true, false, false, true, false]);
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            Some(kept)
        );
    }

    #[gpui::test]
    fn a_click_focus_then_a_request_for_another_node_reports_the_request(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let (clicked, requested) = (ids[0], ids[1]);
        let mut registry = Registry::default();
        for &id in &ids {
            registry.mark_tab_dirty(id);
        }
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
        let held = registry.handle(clicked).unwrap();
        // Focus moved outside the registry, as a click does.
        cx.update(|window, cx| held.focus(window, cx));
        registry.request_focus(requested, false);

        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        let events: Vec<_> = transitions
            .iter()
            .map(|t| (t.blurred(), t.focused()))
            .collect();
        assert_eq!(events, [(None, Some(requested))]);
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            Some(requested)
        );
    }

    #[gpui::test]
    fn a_node_whose_handle_was_forgotten_takes_no_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(1);
        let id = ids[0];
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
        registry.forget(id);
        registry.request_focus(id, false);

        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(transitions.is_empty());
        assert!(registry.handle(id).is_none());
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            None
        );
    }

    #[gpui::test]
    fn forget_clears_a_queued_tab_check(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        registry.forget(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
        });
        assert!(registry.handle(id).is_none());
    }

    #[gpui::test]
    fn get_or_create_reuses_the_same_handle_for_one_node(cx: &mut TestAppContext) {
        let mut registry = Registry::default();
        let cx = cx.add_empty_window();

        let (first, second) = cx.update(|_, cx| {
            let first = get_or_create(&mut registry.handles.handles, 1, cx);
            let second = get_or_create(&mut registry.handles.handles, 1, cx);
            (first, second)
        });

        assert_eq!(first, second);
    }

    #[gpui::test]
    fn forget_lets_a_later_call_mint_a_new_handle(cx: &mut TestAppContext) {
        let mut registry = Registry::default();
        let cx = cx.add_empty_window();

        let before = cx.update(|_, cx| get_or_create(&mut registry.handles.handles, 1, cx));
        registry.forget(1);
        let after = cx.update(|_, cx| get_or_create(&mut registry.handles.handles, 1, cx));

        assert_ne!(before, after);
        assert!(registry.handle(1).is_some());
    }

    #[gpui::test]
    fn focused_node_names_the_node_holding_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
            assert_eq!(registry.focused_node(window, cx), None);
            registry.request_focus(id, true);
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
            assert_eq!(registry.focused_node(window, cx), Some(id));
        });
    }

    #[gpui::test]
    fn observe_takes_the_node_the_window_focuses(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
            registry.handle(id).unwrap().focus(window, cx);
            registry.observe(window, cx);
        });
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), Some(id));
    }

    #[gpui::test]
    fn a_pending_focus_on_a_node_with_a_handle_is_active(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
        });
        registry.request_focus(id, true);
        assert_eq!(registry.active_node(&tree, &StyleCache::new()), Some(id));
    }

    /// Parking the window focus on the root container reports no transition,
    /// maps to no node and does not flip per frame.
    #[gpui::test]
    fn the_parked_focus_fires_no_focus_event(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, _root) = tree_with("div", None);
        let mut registry = Registry::default();
        cx.update(|window, cx| {
            for _ in 0..2 {
                let transitions = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
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
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.request_focus(gone, false);

        registry.drop_focus_in(&tree, gone);

        assert_eq!(registry.active_node(&tree, &StyleCache::new()), None);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
        assert!(transitions.is_empty());
    }

    #[gpui::test]
    fn dropping_focus_in_an_unrelated_subtree_keeps_the_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[0]);
        cx.update(|window, cx| registry.observe(window, cx));

        registry.drop_focus_in(&tree, ids[1]);

        assert_eq!(
            registry.active_node(&tree, &StyleCache::new()),
            Some(ids[0])
        );
        let held = registry.handle(ids[0]).unwrap();
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
        assert!(cx.update(|window, _| held.is_focused(window)));
    }

    #[gpui::test]
    fn a_node_focused_elsewhere_after_a_drop_keeps_its_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let (gone, other) = (ids[0], ids[1]);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.drop_focus_in(&tree, gone);
        // Focus moved outside the registry, as a click does.
        let held = registry.handle(other).unwrap();
        cx.update(|window, cx| held.focus(window, cx));

        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(cx.update(|window, _| held.is_focused(window)));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::focus(|registry, id| registry.request_focus(id, true))]
    #[case::blur(|registry, id| registry.request_blur(id))]
    #[case::tab_dirty(|registry, id| registry.mark_tab_dirty(id))]
    fn queued_work_is_reported_until_the_frame_applies_it(
        cx: &mut TestAppContext,
        queue: fn(&mut Registry, u32),
    ) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(1);
        let mut registry = Registry::default();
        assert!(!registry.has_queued_work());

        queue(&mut registry, ids[0]);
        assert!(registry.has_queued_work());
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(!registry.has_queued_work());
    }

    #[gpui::test]
    fn a_blur_owed_after_a_drop_is_queued_work_until_applied(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(1);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[0]);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.drop_focus_in(&tree, ids[0]);
        assert!(registry.has_queued_work());

        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(!registry.has_queued_work());
    }
}
