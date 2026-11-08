// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where Tab starts while no node is focused.

use super::{FocusRegistry, NodeId, VirtualTree};
use inca_node::tree::VirtualNode;

/// Where Tab starts while no node is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Start {
    /// A mouse press on `node`. `focusing` is the node the press focuses, until the registry
    /// sees that focus.
    Press {
        node: NodeId,
        focusing: Option<NodeId>,
    },
    /// The node that took focus last.
    Focus(NodeId),
}

impl Start {
    pub(super) fn node(self) -> NodeId {
        match self {
            Self::Press { node, .. } | Self::Focus(node) => node,
        }
    }
}

/// Whether `node_id` is the document or a child of it. A tree without a document has none.
fn is_document_side(tree: &VirtualTree, node_id: NodeId) -> bool {
    tree.document().is_some_and(|document| {
        node_id == document || tree.get(node_id).and_then(VirtualNode::parent) == Some(document)
    })
}

impl FocusRegistry {
    /// Records a mouse press on `node_id` as where Tab starts while no node is
    /// focused. `focusing` is the node the press focuses, `None` when it
    /// focuses none or that node already holds focus. A press on `root`
    /// clears the start.
    pub fn press_start(&mut self, node_id: NodeId, root: NodeId, focusing: Option<NodeId>) {
        self.start = (node_id != root).then_some(Start::Press {
            node: node_id,
            focusing,
        });
    }

    /// Moves where Tab starts off `node_id`'s subtree, which leaves its
    /// place in the tree, to `before`, the node right before that place
    /// ([`VirtualTree::preceding`] taken before the change). A start that
    /// lies outside the subtree stays. `before` being the root, the document or one of its
    /// children, or `None`, clears the start.
    pub fn start_leaves(&mut self, tree: &VirtualTree, node_id: NodeId, before: Option<NodeId>) {
        let Some(start) = self.start else { return };
        if tree
            .ancestors(start.node())
            .any(|ancestor| ancestor == node_id)
        {
            self.start = before
                .filter(|&before| Some(before) != tree.root() && !is_document_side(tree, before))
                .map(|node| Start::Press {
                    node,
                    focusing: None,
                });
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use gpui::{TestAppContext, VisualTestContext};

    /// A rooted tree with one node under the root per entry of `tabindex`, `None` for a node with
    /// no tab index, and a registry that holds a handle for each stop and focuses nothing.
    fn start_tree(
        cx: &mut VisualTestContext,
        tabindex: &[Option<&str>],
    ) -> (VirtualTree, Vec<NodeId>, FocusRegistry) {
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
        let mut registry = FocusRegistry::default();
        for &id in &ids {
            registry.mark_tab_dirty(id);
        }
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        (tree, ids, registry)
    }

    fn tab(
        cx: &mut VisualTestContext,
        registry: &mut FocusRegistry,
        tree: &VirtualTree,
        backward: bool,
    ) -> Option<NodeId> {
        cx.update(|window, cx| registry.tab_move(tree, ROOT, window, cx, backward))
    }

    /// Applies a focus request for `node`, then a blur, each on its own frame.
    fn focus_then_blur(
        cx: &mut VisualTestContext,
        registry: &mut FocusRegistry,
        tree: &VirtualTree,
        node: NodeId,
    ) {
        registry.request_focus(node, false);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(tree, window, cx);
        });
        registry.request_blur(node);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(tree, window, cx);
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
    fn shift_tab_after_a_press_on_a_stop_focuses_that_stop(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, ids, mut registry) = start_tree(cx, &[STOP, STOP]);
        registry.press_start(ids[1], ROOT, None);

        assert_eq!(tab(cx, &mut registry, &tree, true), Some(ids[1]));
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
            let _ = registry.apply_pending(&tree, window, cx);
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
    fn a_start_that_leaves_the_tree_moves_before_its_place(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids, mut registry) = start_tree(cx, &[STOP, PLAIN, STOP]);
        registry.press_start(ids[1], ROOT, None);
        let before = tree.preceding(ids[1]);
        tree.remove_child(ROOT, ids[1]).unwrap();
        registry.start_leaves(&tree, ids[1], before);

        assert_eq!(tab(cx, &mut registry, &tree, false), Some(ids[2]));
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
    fn a_start_that_leaves_as_the_first_child_of_the_root_clears(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, ids, mut registry) = start_tree(cx, &[PLAIN, STOP, STOP]);
        registry.press_start(ids[0], ROOT, None);
        let before = tree.preceding(ids[0]);
        tree.remove_child(ROOT, ids[0]).unwrap();
        registry.start_leaves(&tree, ids[0], before);

        assert_eq!(tab(cx, &mut registry, &tree, true), Some(ids[2]));
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

    #[rstest::rstest]
    #[case::before_is_document(false)]
    #[case::before_is_comment_sibling(true)]
    fn a_start_in_the_root_clears_when_the_root_leaves_the_document(#[case] comment: bool) {
        let (mut tree, ids) = rooted_tree(1);
        let document = tree.document().unwrap();
        if comment {
            let comment = tree.create_node("#comment").unwrap();
            tree.insert_before(document, comment, Some(ROOT)).unwrap();
        }
        let mut registry = FocusRegistry::default();
        registry.press_start(ids[0], ROOT, None);
        let before = tree.preceding(ROOT);
        tree.remove_child(document, ROOT).unwrap();

        registry.start_leaves(&tree, ROOT, before);

        assert_eq!(registry.start, None);
    }

    #[test]
    fn a_start_clears_when_it_leaves_as_the_first_child_of_a_root_without_a_document() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.append_child(root, child).unwrap();
        let mut registry = FocusRegistry::default();
        registry.press_start(child, root, None);

        registry.start_leaves(&tree, child, tree.preceding(child));

        assert_eq!(registry.start, None);
    }

    #[test]
    fn a_parentless_before_other_than_the_root_keeps_the_start_without_a_document() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        let child = tree.create_node("div").unwrap();
        let loose = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.append_child(root, child).unwrap();
        let mut registry = FocusRegistry::default();
        registry.press_start(child, root, None);

        registry.start_leaves(&tree, child, Some(loose));

        assert_eq!(
            registry.start,
            Some(Start::Press {
                node: loose,
                focusing: None
            })
        );
    }
}
