// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tab order and tab-index reads.

mod index;
mod order;

use super::{App, FocusRegistry, NodeId, PendingFocus, Start, VirtualTree, Window};

impl FocusRegistry {
    /// Moves focus one tab stop forward, or `backward`. With no node focused,
    /// the move starts from the last press or focus (see
    /// [`Self::press_start`]). Past the last stop nothing holds focus, and the
    /// next Tab starts over at the first. A move that finds no stop forgets
    /// where it started. The transition is reported by the next
    /// [`Self::apply_pending`]. Returns the node that took focus. The move
    /// also drops the scroll requests of earlier focus calls. From a node that no longer takes
    /// focus, the move goes to the nearest stop after its place in tree order (before it when
    /// `backward`), or forward with none after it to the first stop.
    pub fn tab_move(
        &mut self,
        tree: &VirtualTree,
        root: NodeId,
        window: &mut Window,
        cx: &mut App,
        backward: bool,
    ) -> Option<NodeId> {
        let focused = self.focused_node(window, cx);
        let start = if focused.is_none() { self.start } else { None };
        let mut stops = Vec::new();
        let from = start.map(Start::node).or(focused);
        let mut from_at = None;
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if from == Some(id) {
                from_at = Some(stops.len());
            }
            if self.handles.contains_key(&id)
                && let Some(index) = Self::tab_index_of(tree, id)
            {
                stops.push((id, index));
            }
            stack.extend(node.children().iter().rev());
        }
        let target = match (start, from.zip(from_at)) {
            (Some(Start::Press { .. }), Some((node, at))) => {
                order::after_press(&stops, node, at, backward)
            }
            (_, Some((node, at))) if stops.get(at).is_none_or(|stop| stop.0 != node) => {
                order::after_press(&stops, node, at, backward)
            }
            (Some(Start::Focus(node)), _) => order::next_stop(&stops, Some(node), backward),
            _ => order::next_stop(&stops, focused, backward),
        };
        if target.is_none() {
            self.start = None;
        }
        self.reveal.clear();
        self.retry.clear();
        if let Some((id, handle)) = target.and_then(|id| Some((id, self.handles.get(&id)?))) {
            handle.focus(window, cx);
            Some(id)
        } else {
            window.blur(cx);
            None
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
    use crate::tests::*;
    use gpui::TestAppContext;
    use inca_node::AttributeValue;
    use inca_test_macros::gpui_cases;

    #[gpui::test]
    fn tab_move_returns_the_node_it_focused_and_none_past_the_end(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.tab_move(&tree, id, window, cx, false), Some(id));
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.tab_move(&tree, id, window, cx, false), None);
        });
    }

    #[gpui::test]
    fn a_tab_move_drops_the_scroll_requests_of_earlier_focus_calls(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, a) = tree_with("div", Some(AttributeValue::from("0")));
        let b = tree.create_node("div").unwrap();
        tree.set_attribute(b, "tabindex", "0").unwrap();
        tree.append_child(a, b).unwrap();
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(a);
        registry.mark_tab_dirty(b);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
            registry.request_focus(a, true);
            let _ = registry.apply_pending(&tree, window, cx);
            registry.retry_reveal(a);
            assert_eq!(registry.tab_move(&tree, a, window, cx, false), Some(b));
            assert_eq!(registry.take_reveals(), (vec![], vec![]));
        });
    }

    #[gpui::test]
    fn the_last_focus_request_decides_what_scrolls(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, a) = tree_with("div", Some(AttributeValue::from("0")));
        let b = tree.create_node("div").unwrap();
        tree.set_attribute(b, "tabindex", "0").unwrap();
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(a);
        registry.mark_tab_dirty(b);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
            registry.request_focus(a, true);
            registry.request_focus(b, true);
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.take_reveals(), (vec![], vec![b]));
            registry.request_focus(a, true);
            registry.request_focus(b, false);
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.take_reveals(), (vec![], vec![]));
        });
    }

    #[gpui::test]
    fn after_a_drop_no_node_is_focused(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let mut registry = FocusRegistry::default();
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
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        cx.update(|window, cx| registry.observe(window, cx));
        registry.drop_focus_in(&tree, ids[1]);

        let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, false));

        assert_eq!(moved, Some(ids[2]));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::forward(false, 2)]
    #[case::backward(true, 0)]
    fn a_tab_move_from_a_negative_stop_continues_in_tree_order(
        cx: &mut TestAppContext,
        backward: bool,
        want: usize,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_tree(3);
        tree.set_attribute(ids[1], "tabindex", "-1").unwrap();
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);

        let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, backward));

        assert_eq!(moved, Some(ids[want]));
    }

    #[gpui::test]
    fn a_tab_move_past_the_last_stop_leaves_no_node_focused(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_tree(2);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);

        let focused = cx.update(|window, cx| {
            let _ = registry.tab_move(&tree, ROOT, window, cx, false);
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
    fn tab_visits_a_link_and_skips_an_a_without_href(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_nodes(&[
            ("button", &[]),
            ("a", &[("href", "u")]),
            ("button", &[]),
            ("a", &[]),
        ]);
        let mut registry = FocusRegistry::default();
        for &id in &ids {
            registry.mark_tab_dirty(id);
        }
        let visited = cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
            (0..4)
                .map(|_| {
                    let moved = registry.tab_move(&tree, ROOT, window, cx, false);
                    let _ = registry.apply_pending(&tree, window, cx);
                    moved
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(visited, [Some(ids[0]), Some(ids[1]), Some(ids[2]), None]);
    }

    #[gpui::test]
    fn focus_on_an_a_without_href_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_nodes(&[("a", &[])]);
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(ids[0]);
        registry.request_focus(ids[0], false);

        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

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
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[lost.0]);
        tree.remove_attribute(ids[lost.0], lost.1).unwrap();
        registry.mark_tab_dirty(ids[lost.0]);
        cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, backward));

        assert_eq!(moved, want.map(|at| ids[at]));
    }

    #[gpui::test]
    fn a_second_tab_after_the_wrap_to_the_first_stop_focuses_nothing(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) = rooted_nodes(&[("button", &[]), ("a", &[("href", "u")])]);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        tree.remove_attribute(ids[1], "href").unwrap();
        registry.mark_tab_dirty(ids[1]);
        cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        let moved = cx.update(|window, cx| {
            let first = registry.tab_move(&tree, ROOT, window, cx, false);
            let _ = registry.apply_pending(&tree, window, cx);
            (first, registry.tab_move(&tree, ROOT, window, cx, false))
        });

        assert_eq!(moved, (Some(ids[0]), None));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::forward(false, 2)]
    #[case::backward(true, 1)]
    fn a_focused_stop_keeps_the_positive_tab_index_order(
        cx: &mut TestAppContext,
        backward: bool,
        want: usize,
    ) {
        let cx = cx.add_empty_window();
        let (tree, ids) = rooted_nodes(&[
            ("button", &[("tabindex", "2")]),
            ("button", &[("tabindex", "1")]),
            ("button", &[("tabindex", "3")]),
        ]);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[0]);

        let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, backward));

        assert_eq!(moved, Some(ids[want]));
    }

    #[gpui::test]
    fn a_tab_move_from_a_focused_a_that_lost_href_before_the_frame_continues_after_it(
        cx: &mut TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, ids) =
            rooted_nodes(&[("button", &[]), ("a", &[("href", "u")]), ("button", &[])]);
        let mut registry = FocusRegistry::default();
        focus_all(cx, &mut registry, &tree, &ids, ids[1]);
        tree.remove_attribute(ids[1], "href").unwrap();

        let moved = cx.update(|window, cx| registry.tab_move(&tree, ROOT, window, cx, false));

        assert_eq!(moved, Some(ids[2]));
    }
}
