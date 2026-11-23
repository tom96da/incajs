// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Tab move.

use gpui::{App, Window};
use inca_node::NodeId;

use crate::Cells;

impl Cells {
    /// Moves focus one tab stop forward, or `backward`, from the focus the document last observed,
    /// and applies the move to the window's focus. Returns the node that took focus. The next
    /// frame reports the transition. See [`inca_document::FocusModel::plan_tab`].
    pub fn tab_move(&self, backward: bool, window: &mut Window, cx: &mut App) -> Option<NodeId> {
        let (target, plan) = self.document.borrow_mut().plan_tab(backward);
        self.window.borrow_mut().focus.apply(&plan, window, cx);
        target
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::focus::Registry;
    use crate::tests::*;
    use gpui::{TestAppContext, VisualTestContext};
    use inca_node::{AttributeValue, VirtualTree};
    use inca_style::StyleCache;
    use inca_test_macros::gpui_cases;

    /// A rooted tree with one `button`, disabled when `disabled`.
    fn rooted_button(disabled: bool) -> (VirtualTree, NodeId) {
        let (mut tree, _) = rooted_tree(0);
        let id = tree.create_node("button").unwrap();
        tree.append_child(ROOT, id).unwrap();
        if disabled {
            tree.set_attribute(id, "disabled", true).unwrap();
        }
        (tree, id)
    }

    /// A rooted tree with one node under the root per entry of `tabindex`, `None` for a node with
    /// no tab index, and a registry that holds a handle for each stop and focuses nothing.
    fn start_tree(
        cx: &mut VisualTestContext,
        tabindex: &[Option<&str>],
    ) -> (VirtualTree, Vec<NodeId>, Registry) {
        let (mut tree, _) = rooted_tree(0);
        let ids: Vec<NodeId> = tabindex
            .iter()
            .map(|value| {
                let id = tree.create_node("div").unwrap();
                if let Some(value) = value {
                    tree.set_attribute(id, "tabindex", *value).unwrap();
                }
                tree.append_child(ROOT, id).unwrap();
                id
            })
            .collect();
        let mut registry = Registry::default();
        for &id in &ids {
            registry.mark_tab_dirty(id);
        }
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
        });
        (tree, ids, registry)
    }

    fn tab(
        cx: &mut VisualTestContext,
        registry: &mut Registry,
        tree: &VirtualTree,
        backward: bool,
    ) -> Option<NodeId> {
        cx.update(|window, cx| {
            registry.tab_move(tree, &StyleCache::new(), ROOT, window, cx, backward)
        })
    }

    /// Applies a focus request for `node`, then a blur, each on its own frame.
    fn focus_then_blur(
        cx: &mut VisualTestContext,
        registry: &mut Registry,
        tree: &VirtualTree,
        node: NodeId,
    ) {
        registry.request_focus(node, false);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(tree, &StyleCache::new(), window, cx);
        });
        registry.request_blur(node);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(tree, &StyleCache::new(), window, cx);
        });
    }

    const STOP: Option<&str> = Some("0");

    const PLAIN: Option<&str> = None;

    #[gpui::test]
    fn tab_after_a_press_moves_to_the_next_stop(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, PLAIN, STOP]);
        registry.press_start(ids[1], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[2]));
    }

    #[gpui::test]
    fn shift_tab_after_a_press_moves_to_the_previous_stop(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, PLAIN, STOP]);
        registry.press_start(ids[1], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, true), Some(ids[0]));
    }

    #[gpui::test]
    fn tab_after_a_press_on_the_last_stop_focuses_nothing(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, STOP]);
        registry.press_start(ids[1], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, false), None);
    }

    #[gpui::test]
    fn tab_after_a_press_past_the_last_stop_moves_to_the_first(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, STOP, PLAIN]);
        registry.press_start(ids[2], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[0]));
    }

    #[gpui::test]
    fn shift_tab_after_a_press_before_the_first_stop_focuses_nothing(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[PLAIN, STOP, STOP]);
        registry.press_start(ids[0], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, true), None);
    }

    #[gpui::test]
    fn a_tab_move_that_finds_no_stop_forgets_its_start(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[PLAIN, STOP, STOP]);
        registry.press_start(ids[0], ROOT, None);
        tab(cx, &mut registry, &tree, true);

        assert_eq!(tab(cx, &mut registry, &tree, true), Some(ids[2]));
    }

    #[gpui::test]
    fn tab_after_a_press_ignores_a_positive_tabindex(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[PLAIN, STOP, Some("1")]);
        registry.press_start(ids[0], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[1]));
    }

    #[gpui::test]
    fn a_press_on_the_root_forgets_the_start(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, PLAIN, STOP]);
        registry.press_start(ids[1], ROOT, None);
        registry.press_start(ROOT, ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, true), Some(ids[2]));
    }

    #[gpui::test]
    fn tab_from_a_focused_node_ignores_the_start(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, STOP, STOP]);
        registry.request_focus(ids[0], false);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
        });
        registry.press_start(ids[2], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[1]));
    }

    #[gpui::test]
    fn tab_after_a_blur_continues_from_the_blurred_node(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, STOP, STOP]);
        focus_then_blur(cx, &mut registry, &tree, ids[1]);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[2]));
    }

    #[gpui::test]
    fn a_press_that_focuses_a_node_stays_the_start(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, STOP]);
        registry.press_start(ids[0], ROOT, Some(ids[0]));
        focus_then_blur(cx, &mut registry, &tree, ids[0]);

        assert_eq!(tab(cx, &mut registry, &tree, true), Some(ids[0]));
    }

    #[gpui::test]
    fn a_later_focus_of_the_pressed_node_becomes_the_start(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, STOP]);
        registry.press_start(ids[0], ROOT, Some(ids[0]));
        focus_then_blur(cx, &mut registry, &tree, ids[0]);
        focus_then_blur(cx, &mut registry, &tree, ids[0]);

        assert_eq!(tab(cx, &mut registry, &tree, true), None);
    }

    #[gpui::test]
    fn a_start_outside_the_leaving_subtree_stays(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, PLAIN, STOP]);
        registry.press_start(ids[1], ROOT, None);
        registry.start_leaves(&tree, ids[0], tree.preceding(ids[0]));

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[2]));
    }

    #[gpui::test]
    fn tab_after_a_press_skips_a_negative_stop(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[PLAIN, Some("-1"), STOP]);
        registry.press_start(ids[0], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[2]));
    }

    #[gpui::test]
    fn shift_tab_after_a_press_skips_a_negative_stop(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, Some("-1"), PLAIN]);
        registry.press_start(ids[2], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, true), Some(ids[0]));
    }

    #[gpui::test]
    fn tab_after_a_press_on_a_trailing_negative_stop_moves_to_the_first(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, Some("-1")]);
        registry.press_start(ids[1], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[0]));
    }

    #[gpui::test]
    fn a_tab_move_drops_a_pending_reveal_retry_and_scroll_request(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, a) = tree_with("div", Some(AttributeValue::from("0")));
        let b = tree.create_node("div").unwrap();
        tree.set_attribute(b, "tabindex", "0").unwrap();
        tree.append_child(a, b).unwrap();
        let mut registry = Registry::default();
        registry.mark_tab_dirty(a);
        registry.mark_tab_dirty(b);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
            registry.request_focus(a, true);
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
            registry.retry_reveal(a);
            assert_eq!(
                registry.tab_move(&tree, &StyleCache::new(), a, window, cx, false),
                Some(b)
            );
            assert_eq!(registry.take_reveals(), (vec![], vec![]));
        });
    }

    #[gpui::test]
    fn after_a_drop_no_node_is_focused(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.drop_focus_in(&tree, ids[1]);

        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            None
        );
    }

    #[gpui::test]
    fn after_a_drop_tab_continues_after_the_dropped_node(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(3);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.drop_focus_in(&tree, ids[1]);

        let moved = cx.update(|window, cx| {
            registry.tab_move(&tree, &StyleCache::new(), ROOT, window, cx, false)
        });

        assert_eq!(moved, Some(ids[2]));
    }

    #[gpui::test]
    fn a_tab_move_past_the_last_stop_leaves_no_node_focused(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);

        let focused = cx.update(|window, cx| {
            let _ = registry.tab_move(&tree, &StyleCache::new(), ROOT, window, cx, false);
            registry.focused_node(window, cx)
        });

        assert_eq!(focused, None);
    }

    type Spec<'a> = (&'a str, &'a [(&'a str, &'a str)]);

    /// Nodes of `spec` under the root, in order.
    fn rooted_nodes(spec: &[Spec]) -> (VirtualTree, Vec<NodeId>) {
        let (mut tree, _) = rooted_tree(0);
        let ids = spec
            .iter()
            .map(|&(tag, attributes)| {
                let id = tree.create_node(tag).unwrap();
                tree.append_child(ROOT, id).unwrap();
                for &(key, value) in attributes {
                    tree.set_attribute(id, key, value).unwrap();
                }
                id
            })
            .collect();
        (tree, ids)
    }

    #[gpui::test]
    fn focus_on_an_a_without_href_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_nodes(&[("a", &[])]);
        let mut registry = Registry::default();
        registry.mark_tab_dirty(ids[0]);
        registry.request_focus(ids[0], false);

        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(transitions.is_empty());
    }

    const LINK_BETWEEN_BUTTONS: &[Spec] =
        &[("button", &[]), ("a", &[("href", "u")]), ("button", &[])];
    const DIV_BETWEEN_BUTTONS: &[Spec] = &[
        ("button", &[]),
        ("div", &[("tabindex", "0")]),
        ("button", &[]),
    ];

    #[gpui_cases]
    #[gpui::test]
    #[case::link_forward(LINK_BETWEEN_BUTTONS, (1, "href"), false, Some(2))]
    #[case::link_backward(LINK_BETWEEN_BUTTONS, (1, "href"), true, Some(0))]
    #[case::div_forward(DIV_BETWEEN_BUTTONS, (1, "tabindex"), false, Some(2))]
    #[case::none_after_forward(&[("button", &[]), ("a", &[("href", "u")])], (1, "href"), false, Some(0))]
    #[case::none_before_backward(&[("a", &[("href", "u")]), ("button", &[])], (0, "href"), true, None)]
    #[case::positive_after_removed(
        &[("button", &[("tabindex", "1")]), ("div", &[("tabindex", "0")]), ("button", &[]), ("button", &[("tabindex", "2")])],
        (1, "tabindex"), false, Some(2)
    )]
    fn a_tab_move_from_a_node_that_lost_its_stop_continues_in_tree_order(
        cx: &mut TestAppContext,
        spec: &[Spec],
        lost: (usize, &str),
        backward: bool,
        want: Option<usize>,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_nodes(spec);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[lost.0]);
        tree.remove_attribute(ids[lost.0], lost.1).unwrap();
        registry.mark_tab_dirty(ids[lost.0]);
        cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        let moved = cx.update(|window, cx| {
            registry.tab_move(&tree, &StyleCache::new(), ROOT, window, cx, backward)
        });

        assert_eq!(moved, want.map(|at| ids[at]));
    }

    #[gpui::test]
    fn a_second_tab_after_the_wrap_to_the_first_stop_focuses_nothing(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_nodes(&[("button", &[]), ("a", &[("href", "u")])]);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        tree.remove_attribute(ids[1], "href").unwrap();
        registry.mark_tab_dirty(ids[1]);
        cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        let moved = cx.update(|window, cx| {
            let first = registry.tab_move(&tree, &StyleCache::new(), ROOT, window, cx, false);
            let _ = registry.apply_pending(&tree, &StyleCache::new(), window, cx);
            (
                first,
                registry.tab_move(&tree, &StyleCache::new(), ROOT, window, cx, false),
            )
        });

        assert_eq!(moved, (Some(ids[0]), None));
    }

    #[gpui::test]
    fn a_tab_move_from_a_focused_a_that_lost_href_before_the_frame_continues_after_it(
        cx: &mut TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) =
            rooted_nodes(&[("button", &[]), ("a", &[("href", "u")]), ("button", &[])]);
        let mut registry = Registry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        tree.remove_attribute(ids[1], "href").unwrap();

        let moved = cx.update(|window, cx| {
            registry.tab_move(&tree, &StyleCache::new(), ROOT, window, cx, false)
        });

        assert_eq!(moved, Some(ids[2]));
    }

    #[gpui::test]
    fn focus_on_a_disabled_button_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = rooted_button(true);
        let mut registry = Registry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id, false);

        let transitions =
            cx.update(|window, cx| registry.apply_pending(&tree, &StyleCache::new(), window, cx));

        assert!(transitions.is_empty());
        assert_eq!(
            cx.update(|window, cx| registry.focused_node(window, cx)),
            None
        );
    }
}
