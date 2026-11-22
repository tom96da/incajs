// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use inca_document::geometry::Align;
use inca_document::{DocumentState, View, apply_removals};
use inca_jsenv::{EngineError, Frames, Timers};
use inca_node::{NodeId, Removal};

/// The document and window cells the bindings share with the dispatch of a window, and the JS cell
/// of the bindings. A cell is borrowed for one step at a time. No borrow is held across a call into
/// JS, and nothing that runs under one cell's borrow needs another held cell mutably.
pub struct Host {
    /// The tree, its cascade, the listeners, the signals and the focus.
    pub document: Rc<RefCell<DocumentState>>,
    /// What the window drew and holds, as the document reads it: boxes, scroll offsets and held
    /// buttons.
    pub window: Rc<RefCell<dyn View>>,
    /// What belongs to the JS side: failures, timers, frame callbacks and running clicks.
    pub js: RefCell<JsState>,
}

/// The JS cell of a [`Host`].
pub struct JsState {
    /// Errors listeners threw, in order; the dispatcher reports them once the engine is idle.
    pub(crate) failures: Vec<EngineError>,
    /// The nodes whose `click()` is running.
    pub(crate) clicking: HashSet<NodeId>,
    /// The timers the script started. Plain data: ids, due times and periods.
    pub timers: Rc<Timers>,
    /// The callbacks of `requestAnimationFrame` that wait for a frame.
    pub frames: Rc<Frames>,
}

impl JsState {
    /// The JS cell over `timers` and `frames`, with no failure and no running click.
    #[must_use]
    pub fn new(timers: Rc<Timers>, frames: Rc<Frames>) -> Self {
        Self {
            failures: Vec::new(),
            clicking: HashSet::new(),
            timers,
            frames,
        }
    }
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("document", &self.document)
            .finish_non_exhaustive()
    }
}

impl Host {
    /// A host over the `document` and `window` cells, with the JS cell `js`.
    #[must_use]
    pub fn new(
        document: Rc<RefCell<DocumentState>>,
        window: Rc<RefCell<dyn View>>,
        js: JsState,
    ) -> Self {
        Self {
            document,
            window,
            js: RefCell::new(js),
        }
    }

    /// A host over a new document, a window that draws nothing and timers that read `clock`.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn with_clock(clock: inca_jsenv::Clock) -> Self {
        let view = Rc::new(RefCell::new(crate::test_support::TestView::default()));
        Self::over_window(clock, view)
    }

    /// [`Host::with_clock`] with the [`TestView`](crate::test_support::TestView) that stands in
    /// for its window.
    #[cfg(test)]
    pub(crate) fn with_test_view(
        clock: inca_jsenv::Clock,
    ) -> (Self, Rc<RefCell<crate::test_support::TestView>>) {
        let view = Rc::new(RefCell::new(crate::test_support::TestView::default()));
        (
            Self::over_window(clock, Rc::clone(&view) as Rc<RefCell<dyn View>>),
            view,
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    fn over_window(clock: inca_jsenv::Clock, window: Rc<RefCell<dyn View>>) -> Self {
        let send: Rc<dyn Fn()> = Rc::new(|| {});
        let timers = Timers::new(clock, Rc::clone(&send));
        let frames = Frames::new(send);
        Self::new(
            Rc::new(RefCell::new(DocumentState::default())),
            window,
            JsState::new(timers, frames),
        )
    }

    /// Runs the steps that follow a tree change that took `removals` out of the tree. The
    /// document crate drops the focus, moves the Tab start and queues the focus re-check. Every
    /// node resets the scroll offsets of its subtree. `document` is the borrow of the document
    /// cell the caller holds.
    pub(super) fn settle(&self, document: &mut DocumentState, removals: &[Removal]) {
        apply_removals(&mut document.focus, &document.tree, removals);
        let window = self.window.borrow();
        for removal in removals {
            window.reset_scroll(&document.tree, removal.id);
        }
    }

    /// Drops the per-node state of freed nodes and returns the callback ids
    /// their listeners held. `document` is the borrow of the document cell the caller holds.
    pub(crate) fn free_ids(&self, document: &mut DocumentState, ids: &[NodeId]) -> Vec<u32> {
        self.window.borrow_mut().forget(ids);
        document.forget(ids)
    }

    /// Frees every orphan of the tree with its subtree and drops their
    /// per-node state. Returns the freed node ids and the callback ids their
    /// listeners held, or `None` when nothing was freed.
    #[cfg(test)]
    pub(crate) fn sweep_orphans(&self) -> Option<(Vec<NodeId>, Vec<u32>)> {
        let mut document = self.document.borrow_mut();
        let nodes = document.tree.sweep();
        if nodes.is_empty() {
            return None;
        }
        let callbacks = self.free_ids(&mut document, &nodes);
        Some((nodes, callbacks))
    }

    /// Scrolls every scroll container above `node` so the node shows. Returns
    /// false when the last frame recorded no box for the node.
    pub(crate) fn reveal(&self, node: NodeId, block: Align, inline: Align) -> bool {
        let document = self.document.borrow();
        self.window
            .borrow()
            .reveal(&document.tree, &document.style, node, block, inline)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Default for Host {
    fn default() -> Self {
        Self::with_clock(inca_jsenv::system_clock())
    }
}
