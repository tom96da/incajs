// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{Pixels, ScrollHandle, point, px};

use inca_gpui::element::scroll::{Align, record_scroll_write, rect_of, scroll_chain};
use inca_gpui::{NodeId, NodeLayout, VirtualTree, is_scroll_container};
use inca_jsenv::EngineError;

use crate::focus::FocusRegistry;

use super::listeners::EventListeners;

/// Everything one `__inca_native__` binding set shares: the retained tree
/// it mutates, the root a mounting app attaches under, and the
/// event-listener registrations it records.
pub struct Host {
    pub tree: VirtualTree,
    /// Receives one line for each style a `setStyle` call stores with an
    /// unknown key or an invalid color. Writes to stderr by default.
    pub warn: Rc<dyn Fn(&str)>,
    pub listeners: EventListeners,
    pub focus: FocusRegistry,
    /// Last number handed out as an `eventId`. Each event takes the next one.
    pub last_event_id: u64,
    /// Scroll state of the containers, created on first ask. Its own cell, so
    /// it can be written while a shared borrow of `Host` is held.
    pub(crate) scroll_handles: RefCell<HashMap<NodeId, ScrollHandle>>,
    /// The offset of each container as of the last `scroll` check.
    pub(super) scroll_reported: HashMap<NodeId, gpui::Point<gpui::Pixels>>,
    /// Boxes of the last drawn frame. Its own cell, as the draw runs under a shared borrow.
    pub(crate) layout: RefCell<HashMap<NodeId, NodeLayout>>,
    /// Errors listeners threw, in order; the dispatcher reports them once the engine is idle.
    pub(crate) failures: Vec<EngineError>,
    /// The nodes whose `click()` is running.
    pub(crate) clicking: HashSet<NodeId>,
    /// `scrollIntoViewNode` calls whose node had no box yet: awaiting the next
    /// `reveal_focus`, and awaiting the frame after it.
    pub(super) scroll_fresh: Vec<(NodeId, Align, Align)>,
    pub(super) scroll_retry: Vec<(NodeId, Align, Align)>,
}

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("tree", &self.tree)
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
        tree.set_root(root);
        Self {
            tree,
            warn: Rc::new(|line| eprintln!("{line}")),
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

/// A scroll `offset` clamped into `[-max, 0]`, the range gpui draws.
pub(super) fn clamped_offset(offset: Pixels, max: Pixels) -> Pixels {
    offset.clamp(-max, px(0.))
}

impl Host {
    /// The node a mounting app attaches under. Allocated with the tree, so
    /// `rootNodeId` always resolves.
    ///
    /// # Panics
    ///
    /// Panics if the tree has no root. [`Host::default`] always declares one.
    #[must_use]
    pub fn root(&self) -> NodeId {
        self.tree.root().expect("the host declares a root")
    }

    /// Drops the per-node state of freed nodes and returns the callback ids
    /// their listeners held.
    pub(crate) fn free_ids(&mut self, ids: &[NodeId]) -> Vec<u32> {
        for id in ids {
            self.scroll_handles.get_mut().remove(id);
            self.scroll_reported.remove(id);
        }
        ids.iter()
            .flat_map(|&id| {
                self.focus.forget(id);
                self.scroll_fresh.retain(|&(node, ..)| node != id);
                self.scroll_retry.retain(|&(node, ..)| node != id);
                self.listeners.release(id)
            })
            .collect()
    }

    /// Frees every orphan of the tree with its subtree and drops their
    /// per-node state. Returns the freed node ids and the callback ids their
    /// listeners held, or `None` when nothing was freed.
    pub(crate) fn sweep_orphans(&mut self) -> Option<(Vec<NodeId>, Vec<u32>)> {
        let nodes = self.tree.sweep();
        if nodes.is_empty() {
            return None;
        }
        let callbacks = self.free_ids(&nodes);
        Some((nodes, callbacks))
    }

    /// Containers whose clamped offset moved since the last call and whose
    /// `scroll` a listener reaches: one on the container, or a capture one on
    /// an ancestor. Every container's offset is recorded, so a listener added
    /// later sees only later movement.
    pub(crate) fn scrolled_nodes(&mut self) -> Vec<NodeId> {
        let mut moved = Vec::new();
        for (&id, handle) in self.scroll_handles.get_mut().iter() {
            let (offset, max) = (handle.offset(), handle.max_offset());
            let now = gpui::point(
                clamped_offset(offset.x, max.x),
                clamped_offset(offset.y, max.y),
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
            self.reveal(node, Align::Reveal, Align::CenterOutside);
        }
        let mut wait = false;
        for node in fresh {
            if !self.reveal(node, Align::Reveal, Align::CenterOutside) {
                self.focus.retry_reveal(node);
                wait = true;
            }
        }
        wait |= self.reveal_scrolls();
        if wait {
            window.request_animation_frame();
        }
    }

    /// Runs the queued `scrollIntoViewNode` calls. A call whose node still has
    /// no box waits one more frame. Returns whether one waits.
    pub(super) fn reveal_scrolls(&mut self) -> bool {
        for (node, block, inline) in std::mem::take(&mut self.scroll_retry) {
            self.reveal(node, block, inline);
        }
        for (node, block, inline) in std::mem::take(&mut self.scroll_fresh) {
            if !self.reveal(node, block, inline) {
                self.scroll_retry.push((node, block, inline));
            }
        }
        !self.scroll_retry.is_empty()
    }

    /// Returns a clone of the node's handle, creating it on first ask.
    pub(crate) fn scroll_handle(&self, node_id: NodeId) -> ScrollHandle {
        self.scroll_handles
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .clone()
    }

    /// Scrolls every scroll container above `node` so the node shows. Returns
    /// false when the last frame recorded no box for the node.
    pub(crate) fn reveal(&self, node: NodeId, block: Align, inline: Align) -> bool {
        let (own, chain) = {
            let layout = self.layout.borrow();
            let Some(own) = layout.get(&node) else {
                return false;
            };
            let mut chain = Vec::new();
            for id in self.tree.ancestors(node).skip(1) {
                let Some(ancestor) = self.tree.get(id) else {
                    break;
                };
                if is_scroll_container(ancestor)
                    && let Some(l) = layout.get(&id)
                {
                    chain.push((id, rect_of(l.bounds, l.border)));
                }
            }
            (rect_of(own.bounds, px(0.0)), chain)
        };
        scroll_chain(own, &chain, block, inline, |id, dx, dy| {
            let handle = self.scroll_handle(id);
            let (offset, max) = (handle.offset(), handle.max_offset());
            let step = |wanted: f32, offset: Pixels, max: Pixels| {
                let from = -f32::from(clamped_offset(offset, max));
                let to = (from + wanted).clamp(0.0, f32::from(max));
                (to, to - from)
            };
            let (mut x, mut y) = (offset.x, offset.y);
            let (mut applied_x, mut applied_y) = (0.0, 0.0);
            if dx != 0.0 {
                let (to, applied) = step(dx, offset.x, max.x);
                (x, applied_x) = (px(-to), applied);
            }
            if dy != 0.0 {
                let (to, applied) = step(dy, offset.y, max.y);
                (y, applied_y) = (px(-to), applied);
            }
            handle.set_offset(point(x, y));
            if applied_x != 0.0 || applied_y != 0.0 {
                record_scroll_write(id, point(x, y));
            }
            self.shift_descendants(id, applied_x, applied_y);
            (applied_x, applied_y)
        });
        true
    }

    /// Moves the recorded boxes below `container` by the scroll just applied, so a
    /// second reveal before the next draw starts from where the content now is. The
    /// direct children also take it into their recorded scroll offset.
    pub(crate) fn shift_descendants(&self, container: NodeId, dx: f32, dy: f32) {
        let mut layout = self.layout.borrow_mut();
        let mut stack = vec![container];
        while let Some(id) = stack.pop() {
            let Some(node) = self.tree.get(id) else {
                continue;
            };
            stack.extend(node.children());
            if id != container
                && let Some(l) = layout.get_mut(&id)
            {
                l.bounds.origin.x -= px(dx);
                l.bounds.origin.y -= px(dy);
                if self.tree.get(id).and_then(inca_gpui::VirtualNode::parent) == Some(container) {
                    l.scroll.x += px(dx);
                    l.scroll.y += px(dy);
                }
            }
        }
    }
}
