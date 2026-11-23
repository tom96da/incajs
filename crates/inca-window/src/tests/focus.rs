// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A focus model with the handles of a window, for the tests that drive both.

#![allow(clippy::unwrap_used)]

use std::ops::{Deref, DerefMut};

use gpui::{App, FocusHandle, Window};
use inca_document::{
    FocusModel, FocusPlan, FocusTransition, Reveal, RevealOrigin, RevealQueue, new_document_tree,
};
use inca_node::tree::{AttributeValue, NodeId, VirtualTree};
use inca_style::StyleCache;

use crate::FocusRegistry;

/// The root of a tree `rooted_tree` builds: the first node created. The document is node 1.
pub(crate) const ROOT: NodeId = 0;

/// A tree whose document holds the root, with `count` nodes attached under the root, each
/// `tabindex="0"`.
pub(crate) fn rooted_tree(count: usize) -> (VirtualTree, Vec<NodeId>) {
    let mut tree = new_document_tree();
    let root = tree.root().unwrap();
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

pub(crate) fn tree_with(tag: &str, tabindex: Option<AttributeValue>) -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node(tag).unwrap();
    if let Some(value) = tabindex {
        tree.set_attribute(id, "tabindex", value).unwrap();
    }
    (tree, id)
}

/// The focus model, the reveal queue and the handles of one window. The model's methods are
/// reached through `Deref`.
#[derive(Default)]
pub(crate) struct Registry {
    pub(crate) model: FocusModel,
    pub(crate) reveals: RevealQueue,
    pub(crate) handles: FocusRegistry,
}

impl Deref for Registry {
    type Target = FocusModel;

    fn deref(&self) -> &FocusModel {
        &self.model
    }
}

impl DerefMut for Registry {
    fn deref_mut(&mut self) -> &mut FocusModel {
        &mut self.model
    }
}

impl Registry {
    pub(crate) fn handle(&self, node: NodeId) -> Option<FocusHandle> {
        self.handles.handle(node)
    }

    pub(crate) fn focused_node(&self, window: &Window, cx: &App) -> Option<NodeId> {
        self.model.held_node(self.handles.focused_node(window, cx))
    }

    pub(crate) fn observe(&mut self, window: &Window, cx: &App) {
        self.model
            .observe_node(self.handles.focused_node(window, cx));
    }

    pub(crate) fn forget(&mut self, node: NodeId) {
        self.model.forget(node);
        self.reveals.forget(node);
        self.handles.forget(node);
    }

    pub(crate) fn drop_focus_in(&mut self, tree: &VirtualTree, node: NodeId) {
        self.model.drop_focus_in(tree, node);
        self.reveals.drop_focus_in(tree, node);
    }

    pub(crate) fn retry_reveal(&mut self, node: NodeId) {
        self.reveals.retry(Reveal::focus(node));
    }

    /// The nodes of the reveals a focus queued: `(retries, fresh)`.
    pub(crate) fn take_reveals(&mut self) -> (Vec<NodeId>, Vec<NodeId>) {
        let reveals = self.reveals.take();
        let nodes = |list: Vec<Reveal>| {
            list.into_iter()
                .filter(|reveal| reveal.origin == RevealOrigin::Focus)
                .map(|reveal| reveal.node)
                .collect()
        };
        (nodes(reveals.retries), nodes(reveals.fresh))
    }

    /// Applies `plan` as the window does.
    fn apply(&mut self, plan: &FocusPlan, window: &mut Window, cx: &mut App) {
        self.reveals.follow_plan(plan);
        self.handles.apply(plan, window, cx);
    }

    /// Runs a frame as the window does: observes the window's focus, plans and applies.
    pub(crate) fn apply_pending(
        &mut self,
        tree: &VirtualTree,
        cache: &StyleCache,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<FocusTransition> {
        self.observe(window, cx);
        let plan = self.model.plan_frame(tree, cache);
        self.apply(&plan, window, cx);
        plan.transitions
    }

    /// Runs a Tab press as the window does: observes the window's focus, plans and applies.
    pub(crate) fn tab_move(
        &mut self,
        tree: &VirtualTree,
        cache: &StyleCache,
        root: NodeId,
        window: &mut Window,
        cx: &mut App,
        backward: bool,
    ) -> Option<NodeId> {
        self.observe(window, cx);
        let (target, plan) = self.model.plan_tab(tree, cache, root, backward);
        self.apply(&plan, window, cx);
        target
    }
}

pub(crate) fn focus_all(
    cx: &mut gpui::VisualTestContext,
    registry: &mut Registry,
    tree: &VirtualTree,
    ids: &[NodeId],
    focus: NodeId,
) {
    for &id in ids {
        registry.mark_tab_dirty(id);
    }
    registry.request_focus(focus, false);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(tree, &StyleCache::new(), window, cx);
    });
}
