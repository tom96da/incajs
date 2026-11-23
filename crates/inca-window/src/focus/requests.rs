// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frame that applies the queued focus and blur requests.

use gpui::{App, Window};
use inca_document::FocusTransition;

use crate::Cells;

impl Cells {
    /// Applies the queued focus requests of the document as a frame does: records the node the
    /// window focuses, plans the frame and applies the plan to the handles and the window's focus.
    /// Returns each transition the plan reports. Call once per frame, before the element tree
    /// that needs the resulting `track_focus` wiring is built.
    ///
    /// The transitions are returned and not dispatched here, so the caller fires them after the
    /// borrows of the cells end. An [`inca_gpui::EventSink`] that re-borrows a cell while firing
    /// would otherwise panic with "already mutably borrowed".
    pub fn apply_focus(&self, window: &mut Window, cx: &mut App) -> Vec<FocusTransition> {
        self.observe_focus(window, cx);
        let plan = self.document.borrow_mut().plan_focus_frame();
        self.window.borrow_mut().focus.apply(&plan, window, cx);
        plan.transitions
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::focus::Registry;
    use crate::tests::*;
    use gpui::TestAppContext;
    use inca_node::AttributeValue;
    use inca_style::StyleCache;

    #[gpui::test]
    fn removing_tabindex_blurs_the_focused_node(cx: &mut TestAppContext) {
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
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(!cx.update(|window, _| held.is_focused(window)));
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred(), Some(id));
        assert_eq!(transitions[0].focused(), None);
    }

    #[gpui::test]
    fn removing_tabindex_blurs_a_node_a_click_focused_before_the_next_sync(
        cx: &mut TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
        });
        let held = registry.handle(id).unwrap();
        // Focus moved outside the registry, as a click does.
        cx.update(|window, cx| held.focus(window, cx));

        tree.remove_attribute(id, "tabindex").unwrap();
        registry.mark_tab_dirty(id);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(!cx.update(|window, _| held.is_focused(window)));
        let events: Vec<_> = transitions
            .iter()
            .map(|t| (t.blurred(), t.focused()))
            .collect();
        assert_eq!(events, [(None, Some(id)), (Some(id), None)]);
    }

    /// A root with `tabindex` takes focus like any node, and the parked focus
    /// leaves it alone.
    #[gpui::test]
    fn a_root_with_tabindex_still_focuses_normally(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, root) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = Registry::default();
        registry.mark_tab_dirty(root);
        cx.update(|window, cx| {
            assert!(
                registry
                    .apply_pending(&tree, &StyleCache::new(), window, cx)
                    .is_empty()
            );
            registry.request_focus(root, true);
            let transitions = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
            assert_eq!(transitions.len(), 1);
            assert_eq!(registry.focused_node(window, cx), Some(root));
            assert!(
                registry
                    .apply_pending(&tree, &StyleCache::new(), window, cx)
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn a_disconnected_focused_node_loses_focus_without_a_transition(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(1);
        let id = ids[0];
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, id);
        let held = registry.handle(id).unwrap();
        assert!(cx.update(|window, _| held.is_focused(window)));

        tree.remove_child(ROOT, id).unwrap();
        registry.mark_tab_dirty(id);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(transitions.is_empty());
        assert!(registry.handle(id).is_none());
        assert!(!cx.update(|window, _| held.is_focused(window)));
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            None
        );
    }

    #[gpui::test]
    fn a_pending_focus_takes_effect_after_a_silent_drop(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(2);
        let (gone, other) = (ids[0], ids[1]);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);

        tree.remove_child(ROOT, gone).unwrap();
        registry.mark_tab_dirty(gone);
        registry.request_focus(other, false);
        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            Some(other)
        );
        let events: Vec<_> = transitions
            .iter()
            .map(|t| (t.blurred(), t.focused()))
            .collect();
        assert_eq!(events, [(None, Some(other))]);
    }

    #[gpui::test]
    fn dropping_focus_releases_the_window_at_the_next_apply(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let (gone, other) = (ids[0], ids[1]);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.request_focus(gone, false);

        registry.drop_focus_in(&tree, gone);

        let held = registry.handle(gone).unwrap();
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
        assert!(!cx.update(|window, _| held.is_focused(window)));
        assert!(registry.handle(other).is_some());
    }

    #[gpui::test]
    fn a_dropped_node_is_not_revealed_on_the_next_frame(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let (gone, kept) = (ids[0], ids[1]);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, gone);
        registry.request_focus(gone, true);
        let _ =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
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
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, id);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.set_window_blurred(true);
        registry.drop_focus_in(&tree, id);

        let first =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));
        registry.set_window_blurred(false);
        let held = registry.handle(id).unwrap();
        cx.update(|window, cx| held.focus(window, cx));
        let second =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(first.is_empty());
        let events: Vec<_> = second.iter().map(|t| (t.blurred(), t.focused())).collect();
        assert_eq!(events, [(None, Some(id))]);
        assert!(cx.update(|window, _| held.is_focused(window)));
    }
}
