// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test helpers that the tests of several modules share.

#![allow(clippy::unwrap_used)]

use crate::geometry::Align;
use crate::{
    FocusSurface, InputState, NodeBox, ScrollRange, StyleWarnings, View, new_document_tree,
};
use inca_node::tree::{AttributeValue, NodeId, VirtualTree};
use inca_style::StyleCache;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

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

/// A window side that records what the focus model asks of it.
#[derive(Default)]
pub(crate) struct FakeSurface {
    pub(crate) handles: HashSet<NodeId>,
    pub(crate) focus: Option<NodeId>,
    pub(crate) parks: usize,
}

impl FocusSurface for FakeSurface {
    fn focused(&self) -> Option<NodeId> {
        self.focus
    }

    fn track(&mut self, node: NodeId) {
        self.handles.insert(node);
    }

    fn untrack(&mut self, node: NodeId) {
        self.handles.remove(&node);
    }

    fn focus(&mut self, node: NodeId) {
        if self.handles.contains(&node) {
            self.focus = Some(node);
        }
    }

    fn blur(&mut self) {
        self.focus = None;
    }

    fn park(&mut self) {
        self.parks += 1;
    }
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

/// A `StyleWarnings` that sends each line to a list the caller reads.
pub(crate) fn collecting() -> (StyleWarnings, Rc<RefCell<Vec<String>>>) {
    let lines = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&lines);
    let mut warnings = StyleWarnings::default();
    warnings.set_sink(Rc::new(move |line| sink.borrow_mut().push(line.to_owned())));
    (warnings, lines)
}

/// A `View` that draws nothing and records the nodes whose scroll reset and the nodes it forgot.
#[derive(Default)]
pub(crate) struct RecordingView {
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

    fn queue_reveal(&mut self, _: NodeId, _: Align, _: Align) {}

    fn held_buttons(&self) -> u8 {
        0
    }

    fn set_held_buttons(&mut self, _: u8) {}

    fn reset_scroll(&self, _: &VirtualTree, id: NodeId) {
        self.resets.borrow_mut().push(id);
    }

    fn forget(&mut self, ids: &[NodeId]) {
        self.forgotten.extend_from_slice(ids);
    }
}
