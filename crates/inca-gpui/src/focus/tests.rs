// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use gpui::TestAppContext;
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::*;

mod disabled;
mod handles;
mod tab_index;
mod tab_stops;

fn tree_with(tag: &str, tabindex: Option<AttributeValue>) -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node(tag).unwrap();
    if let Some(value) = tabindex {
        tree.set_attribute(id, "tabindex", value).unwrap();
    }
    (tree, id)
}

// Nodes 1 to 6 have tabindex 3, 1, 0, 0, -1 and 2.
const STOPS: [(NodeId, i32); 6] = [(1, 3), (2, 1), (3, 0), (4, 0), (5, -1), (6, 2)];

fn walk(stops: &[(NodeId, i32)], start: Option<NodeId>, backward: bool) -> Vec<Option<NodeId>> {
    let mut current = start;
    let mut moves = Vec::new();
    for _ in 0..stops.len() + 2 {
        let target = next_stop(stops, current, backward);
        moves.push(target);
        current = target;
    }
    moves
}

/// The root of a tree `rooted_tree` builds: the first node created.
const ROOT: NodeId = 0;

/// A rooted tree with `count` nodes attached under the root, each
/// `tabindex="0"`.
fn rooted_tree(count: usize) -> (VirtualTree, Vec<NodeId>) {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
    let ids = (0..count)
        .map(|_| {
            let id = tree.create_node("div").unwrap();
            tree.set_attribute(id, "tabindex", "0").unwrap();
            tree.append_child(root, id).unwrap();
            id
        })
        .collect();
    (tree, ids)
}

fn focus_all(
    cx: &mut gpui::VisualTestContext,
    registry: &mut FocusRegistry,
    tree: &VirtualTree,
    ids: &[NodeId],
    focus: NodeId,
) {
    for &id in ids {
        registry.mark_tab_dirty(id);
    }
    registry.request_focus(focus, false);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(tree, window, cx);
    });
}

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
