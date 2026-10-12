// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{ScrollHandle, px};

use inca_gpui::{NodeId, NodeLayout, VirtualTree};
use inca_jsenv::EngineError;

use crate::focus::FocusRegistry;
use crate::scroll_into_view::Align;

use super::listeners::EventListeners;

/// Everything one `__inca_native__` binding set shares: the retained tree
/// it mutates, the root a mounting app attaches under, and the
/// event-listener registrations it records.
pub struct Host {
    pub tree: VirtualTree,
    /// Receives one line for each style a `setStyle` call stores with an
    /// unknown key or an invalid color. Writes to stderr by default.
    pub warn: Rc<dyn Fn(&str)>,
    /// Allocated with the tree, so `rootNodeId` always resolves.
    pub root: NodeId,
    pub listeners: EventListeners,
    pub focus: FocusRegistry,
    /// Last number handed out as an `eventId`. Each event takes the next one.
    pub last_event_id: u64,
    // Scroll state of the containers, created on first ask. Its own cell, so
    // it can be written while a shared borrow of `Host` is held.
    pub(crate) scroll_handles: RefCell<HashMap<NodeId, ScrollHandle>>,
    // The offset of each container as of the last `scroll` check.
    pub(super) scroll_reported: HashMap<NodeId, gpui::Point<gpui::Pixels>>,
    // Boxes of the last drawn frame. Its own cell, as the draw runs under a shared borrow.
    pub(crate) layout: RefCell<HashMap<NodeId, NodeLayout>>,
    // Errors listeners threw, in order; the dispatcher reports them once the engine is idle.
    pub(crate) failures: Vec<EngineError>,
    // The nodes whose `click()` is running.
    pub(crate) clicking: HashSet<NodeId>,
    // `scrollIntoViewNode` calls whose node had no box yet: awaiting the next
    // `reveal_focus`, and awaiting the frame after it.
    pub(super) scroll_fresh: Vec<(NodeId, Align, Align)>,
    pub(super) scroll_retry: Vec<(NodeId, Align, Align)>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("tree", &self.tree)
            .field("root", &self.root)
            .field("listeners", &self.listeners)
            .field("focus", &self.focus)
            .finish_non_exhaustive()
    }
}

impl Default for Host {
    fn default() -> Self {
        let mut tree = VirtualTree::new();
        let root = tree
            .create_node("div")
            .expect("a new tree has free node ids");
        Self {
            tree,
            warn: Rc::new(|line| eprintln!("{line}")),
            root,
            listeners: EventListeners::default(),
            focus: FocusRegistry::default(),
            last_event_id: 0,
            scroll_handles: RefCell::new(HashMap::new()),
            scroll_reported: HashMap::new(),
            layout: RefCell::new(HashMap::new()),
            failures: Vec::new(),
            clicking: HashSet::new(),
            scroll_fresh: Vec::new(),
            scroll_retry: Vec::new(),
        }
    }
}

impl Host {
    // Containers whose clamped offset moved since the last call and whose
    // `scroll` a listener reaches: one on the container, or a capture one on
    // an ancestor. Every container's offset is recorded, so a listener added
    // later sees only later movement.
    pub(crate) fn scrolled_nodes(&mut self) -> Vec<NodeId> {
        let mut moved = Vec::new();
        for (&id, handle) in self.scroll_handles.get_mut().iter() {
            let (offset, max) = (handle.offset(), handle.max_offset());
            let now = gpui::point(
                offset.x.clamp(-max.x, px(0.)),
                offset.y.clamp(-max.y, px(0.)),
            );
            let before = self.scroll_reported.insert(id, now).unwrap_or_default();
            if before != now {
                moved.push(id);
            }
        }
        let flags = crate::dispatch::spec_of("scroll").flags;
        let host: &Host = self;
        moved.retain(|&id| {
            let path = crate::event::path_from(host, id);
            crate::event::listened(host, "scroll", flags, &path, &[])
        });
        moved.sort_unstable();
        moved
    }

    /// Scrolls the scroll containers above the node whose focus request asked
    /// for it, using the boxes of the last drawn frame. A node whose box the
    /// frame lacks gets one more frame to receive it. It also runs the
    /// `scrollIntoViewNode` calls that waited for a box, with the same one-frame
    /// retry. Call it once per frame after `FocusRegistry::apply_pending`.
    pub fn reveal_focus(&mut self, window: &mut gpui::Window) {
        let (retries, fresh) = self.focus.take_reveals();
        for node in retries {
            crate::scroll_into_view::reveal(self, node, Align::Reveal, Align::CenterOutside);
        }
        let mut wait = false;
        for node in fresh {
            if !crate::scroll_into_view::reveal(self, node, Align::Reveal, Align::CenterOutside) {
                self.focus.retry_reveal(node);
                wait = true;
            }
        }
        wait |= self.reveal_scrolls();
        if wait {
            window.request_animation_frame();
        }
    }

    // Runs the queued `scrollIntoViewNode` calls. A call whose node still has
    // no box waits one more frame. Returns whether one waits.
    pub(super) fn reveal_scrolls(&mut self) -> bool {
        for (node, block, inline) in std::mem::take(&mut self.scroll_retry) {
            crate::scroll_into_view::reveal(self, node, block, inline);
        }
        for (node, block, inline) in std::mem::take(&mut self.scroll_fresh) {
            if !crate::scroll_into_view::reveal(self, node, block, inline) {
                self.scroll_retry.push((node, block, inline));
            }
        }
        !self.scroll_retry.is_empty()
    }

    // Returns a clone of the node's handle, creating it on first ask.
    pub(crate) fn scroll_handle(&self, node_id: NodeId) -> ScrollHandle {
        self.scroll_handles
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .clone()
    }
}
