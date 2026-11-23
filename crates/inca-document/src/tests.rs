// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test helpers that the tests of several modules share.

#![allow(clippy::unwrap_used)]

use crate::geometry::Align;
use crate::{
    FocusModel, FocusPlan, FocusStep, FocusTransition, InputState, NodeBox, Reveal, RevealQueue,
    ScrollRange, View, new_document_tree,
};
use inca_node::tree::{AttributeValue, NodeId, VirtualTree};
use inca_style::StyleCache;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Range;

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

/// A window that applies focus plans as the real one does, with no platform behind it: a node
/// panics on a focus step that names a node with no handle, and a park only counts.
#[derive(Default)]
pub(crate) struct FakeWindow {
    pub(crate) handles: HashSet<NodeId>,
    pub(crate) focus: Option<NodeId>,
    pub(crate) parks: usize,
    pub(crate) reveals: RevealQueue,
}

impl FakeWindow {
    pub(crate) fn apply(&mut self, plan: &FocusPlan) {
        self.handles.extend(plan.gained.iter().copied());
        for step in &plan.steps {
            match *step {
                FocusStep::Focus(node) => {
                    assert!(
                        self.handles.contains(&node),
                        "the focus plan names {node}, which has no handle"
                    );
                    self.focus = Some(node);
                }
                FocusStep::Blur => self.focus = None,
            }
        }
        for node in &plan.lost {
            self.handles.remove(node);
        }
        if plan.park {
            self.parks += 1;
        }
        self.reveals.follow_plan(plan);
    }

    /// The nodes of the reveals a focus queued: `(retries, fresh)`.
    pub(crate) fn take_reveals(&mut self) -> (Vec<NodeId>, Vec<NodeId>) {
        let reveals = self.reveals.take();
        let nodes = |list: Vec<Reveal>| list.into_iter().map(|reveal| reveal.node).collect();
        (nodes(reveals.retries), nodes(reveals.fresh))
    }
}

/// Runs a frame of `model` on `window` as the window does: it observes the window's focus, plans
/// the frame and applies the plan. Returns the transitions.
pub(crate) fn apply_frame(
    model: &mut FocusModel,
    tree: &VirtualTree,
    window: &mut FakeWindow,
) -> Vec<FocusTransition> {
    model.observe_node(window.focus);
    let plan = model.plan_frame(tree, &StyleCache::new());
    window.apply(&plan);
    plan.transitions
}

/// An input state holding `value` with `selection` selected. The caret sits at the start of the
/// selection when `reversed`, else at its end.
pub(crate) fn at(value: &str, selection: Range<usize>, reversed: bool) -> InputState {
    let mut state = InputState::default();
    state.set_value(value);
    if reversed {
        state.select(selection.end, selection.start);
    } else {
        state.select(selection.start, selection.end);
    }
    state
}

/// An input state with a caret at `caret` in `value`.
pub(crate) fn caret_at(value: &str, caret: usize) -> InputState {
    at(value, caret..caret, false)
}

/// A `View` that draws nothing, holds the buttons `held`, and records the nodes whose scroll reset
/// and the nodes it forgot.
#[derive(Default)]
pub(crate) struct RecordingView {
    pub(crate) held: u8,
    pub(crate) resets: RefCell<Vec<NodeId>>,
    pub(crate) forgotten: Vec<NodeId>,
}

impl View for RecordingView {
    fn node_box(&self, _id: NodeId) -> Option<NodeBox> {
        None
    }

    fn scroll_range(&self, _id: NodeId) -> ScrollRange {
        ScrollRange {
            offset: (0.0, 0.0),
            max: (0.0, 0.0),
        }
    }

    fn scroll_to(&self, _: &VirtualTree, _: NodeId, _: Option<f64>, _: Option<f64>) {}

    fn reveal(&self, _: &VirtualTree, _: &StyleCache, _: NodeId, _: Align, _: Align) -> bool {
        false
    }

    fn held_buttons(&self) -> u8 {
        self.held
    }

    fn reset_scroll(&self, _: &VirtualTree, id: NodeId) {
        self.resets.borrow_mut().push(id);
    }

    fn forget(&mut self, ids: &[NodeId]) {
        self.forgotten.extend_from_slice(ids);
    }
}
