// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{Pixels, ScrollHandle, point, px};

use inca_document::geometry::{Align, scroll_chain};
use inca_document::{
    EventListeners, InputState, Signals, StyleWarnings, apply_removals, new_document_tree,
};
use inca_gpui::{InputLayout, NodeLayout, rect_of};
use inca_jsenv::{Clock, EngineError, Frames, Timers, system_clock};
use inca_node::{NodeId, Removal, VirtualTree};
use inca_style::{StyleCache, is_scroll_container};

use crate::focus::FocusRegistry;

/// The three cells one `__inca_native__` binding set and its dispatcher share. A cell is borrowed
/// for one step at a time. No borrow is held across a call into JS, and nothing that runs under
/// one cell's borrow needs another held cell mutably.
pub struct Host {
    /// The tree, its cascade, the listeners, the signals and the focus.
    pub document: RefCell<DocumentState>,
    /// What the window drew and holds: boxes, scroll offsets, held buttons and the capture.
    pub window: RefCell<WindowState>,
    /// What belongs to the JS side: failures, timers, frame callbacks and running clicks.
    pub js: RefCell<JsState>,
}

/// The document cell of a [`Host`]: the retained tree it mutates, the root a mounting app attaches
/// under, and the event-listener registrations it records.
pub struct DocumentState {
    pub tree: VirtualTree,
    /// The cascade of the tree, rebuilt when the tree changes.
    pub style: StyleCache,
    /// Receives one line for each style a `setStyle` call stores with an unknown key, an invalid
    /// color or a value of a shape the host does not support. A node prints each line once.
    /// Writes to stderr by default.
    pub warnings: StyleWarnings,
    pub listeners: EventListeners,
    /// The state of every `AbortSignal`, by id. Plain data. The objects live in JS.
    pub(crate) signals: Signals,
    pub focus: FocusRegistry,
    /// Last number handed out as an `eventId`. Each event takes the next one.
    pub last_event_id: u64,
    /// The node a script asked to capture the mouse pointer. Its `gotpointercapture` fires before
    /// the next pointer event.
    pub(crate) capture_pending: Option<NodeId>,
    /// The text states of the `input` nodes, created on first use. Its own cell, as the draw runs
    /// under a shared borrow.
    pub(crate) inputs: RefCell<HashMap<NodeId, InputState>>,
}

/// A `scrollIntoViewNode` call: the node and the block and inline alignment.
type ScrollCall = (NodeId, Align, Align);

/// The window cell of a [`Host`].
pub struct WindowState {
    /// Scroll state of the containers, created on first ask. Its own cell, so
    /// it can be written while a shared borrow of the window cell is held.
    pub(crate) scroll_handles: RefCell<HashMap<NodeId, ScrollHandle>>,
    /// The offset of each container as of the last `scroll` check.
    pub(super) scroll_reported: HashMap<NodeId, gpui::Point<gpui::Pixels>>,
    /// Boxes of the last drawn frame. Its own cell, as the draw runs under a shared borrow.
    pub(crate) layout: RefCell<HashMap<NodeId, NodeLayout>>,
    /// Where the last frame drew the text of each `input` node. Its own cell, as the draw runs
    /// under a shared borrow.
    pub(crate) input_layouts: RefCell<HashMap<NodeId, InputLayout>>,
    /// Every mouse button currently held, as the `buttons` bitmask of the events. GPUI's own mouse
    /// events carry only the one button each is about, not which others are held alongside it.
    pub(crate) held_buttons: u8,
    /// The node the pointer events go to while a capture is active. A node that left the tree
    /// loses the capture at the next pointer event.
    pub(crate) capture_active: Option<NodeId>,
    /// `scrollIntoViewNode` calls whose node had no box yet: awaiting the next
    /// `reveal_focus`, and awaiting the frame after it.
    pub(super) scroll_fresh: Vec<ScrollCall>,
    pub(super) scroll_retry: Vec<ScrollCall>,
    /// Receives one message each time the timers start a timer or the frames queue a callback.
    pub(crate) wake: async_channel::Receiver<()>,
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

impl std::fmt::Debug for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Host")
            .field("document", &self.document)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DocumentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentState")
            .field("tree", &self.tree)
            .field("listeners", &self.listeners)
            .field("focus", &self.focus)
            .finish_non_exhaustive()
    }
}

impl Default for Host {
    fn default() -> Self {
        Self::with_clock(system_clock())
    }
}

impl Host {
    /// A host whose timers read `clock`. [`Host::default`] reads the system clock.
    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        let (sender, wake) = async_channel::bounded(1);
        let send: Rc<dyn Fn()> = Rc::new(move || {
            let _ = sender.try_send(());
        });
        let timers = Timers::new(clock, Rc::clone(&send));
        let frames = Frames::new(send);
        Self {
            document: RefCell::new(DocumentState {
                tree: new_document_tree(),
                style: StyleCache::new(),
                warnings: StyleWarnings::default(),
                listeners: EventListeners::default(),
                signals: Signals::default(),
                focus: FocusRegistry::default(),
                last_event_id: 0,
                capture_pending: None,
                inputs: RefCell::new(HashMap::new()),
            }),
            window: RefCell::new(WindowState {
                scroll_handles: RefCell::new(HashMap::new()),
                scroll_reported: HashMap::new(),
                layout: RefCell::new(HashMap::new()),
                input_layouts: RefCell::new(HashMap::new()),
                held_buttons: 0,
                capture_active: None,
                scroll_fresh: Vec::new(),
                scroll_retry: Vec::new(),
                wake,
            }),
            js: RefCell::new(JsState {
                failures: Vec::new(),
                clicking: HashSet::new(),
                timers,
                frames,
            }),
        }
    }
}

/// A scroll `offset` clamped into `[-max, 0]`, the range gpui draws.
pub(super) fn clamped_offset(offset: Pixels, max: Pixels) -> Pixels {
    offset.clamp(-max, px(0.))
}

impl Host {
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
    pub(crate) fn sweep_orphans(&self) -> Option<(Vec<NodeId>, Vec<u32>)> {
        let mut document = self.document.borrow_mut();
        let nodes = document.tree.sweep();
        if nodes.is_empty() {
            return None;
        }
        let callbacks = self.free_ids(&mut document, &nodes);
        Some((nodes, callbacks))
    }

    /// The deepest container whose last drawn box holds the point, else the top node.
    pub(crate) fn container_at(&self, x: f32, y: f32) -> NodeId {
        let document = self.document.borrow();
        self.window
            .borrow()
            .container_at(&document.tree, x, y)
            .unwrap_or_else(|| document.top())
    }

    /// Containers whose clamped offset moved since the last call and whose
    /// `scroll` a listener reaches: one on the container, or a capture one on
    /// an ancestor. Every container's offset is recorded, so a listener added
    /// later sees only later movement.
    pub(crate) fn scrolled_nodes(&self) -> Vec<NodeId> {
        let mut moved = self.window.borrow_mut().moved_containers();
        let flags = crate::dispatch::spec_of("scroll").flags;
        let document = self.document.borrow();
        moved.retain(|&id| {
            crate::event::listened(
                &document,
                "scroll",
                flags,
                crate::EventTargetId::Node(id),
                &[],
            )
        });
        moved.sort_unstable();
        moved
    }

    /// Scrolls the scroll containers above the node whose focus request asked
    /// for it, using the boxes of the last drawn frame. A node whose box the
    /// frame lacks gets one more frame to receive it. It also runs the
    /// `scrollIntoViewNode` calls that waited for a box, with the same one-frame
    /// retry. Call it once per frame after `FocusRegistry::apply_pending`.
    pub fn reveal_focus(&self, window: &mut gpui::Window) {
        let (retries, fresh) = self.document.borrow_mut().focus.take_reveals();
        for node in retries {
            self.reveal(node, Align::Reveal, Align::CenterOutside);
        }
        let mut wait = false;
        for node in fresh {
            if !self.reveal(node, Align::Reveal, Align::CenterOutside) {
                self.document.borrow_mut().focus.retry_reveal(node);
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
    pub(super) fn reveal_scrolls(&self) -> bool {
        let (retries, fresh) = self.window.borrow_mut().take_scroll_calls();
        for (node, block, inline) in retries {
            self.reveal(node, block, inline);
        }
        let mut retry = Vec::new();
        for (node, block, inline) in fresh {
            if !self.reveal(node, block, inline) {
                retry.push((node, block, inline));
            }
        }
        let mut window = self.window.borrow_mut();
        window.scroll_retry.extend(retry);
        !window.scroll_retry.is_empty()
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

impl DocumentState {
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

    /// The document, the parent of the node a mounting app attaches under.
    ///
    /// # Panics
    ///
    /// Panics if the tree has no document. [`Host::default`] always declares one.
    #[must_use]
    pub fn document(&self) -> NodeId {
        self.tree.document().expect("the host declares a document")
    }

    /// The node events fall back to: the document element, else the document.
    #[must_use]
    pub fn top(&self) -> NodeId {
        self.tree
            .document_element()
            .unwrap_or_else(|| self.document())
    }

    /// Moves where Tab starts off `node_id`'s subtree to `before`, the node right before its
    /// place.
    pub(super) fn move_tab_start(&mut self, node_id: NodeId, before: Option<NodeId>) {
        self.focus.start_leaves(&self.tree, node_id, before);
    }

    /// Queues a focus re-check of the node and its subtree.
    pub(super) fn mark_focus_dirty(&mut self, node_id: NodeId) {
        self.focus.mark_subtree_dirty(&self.tree, node_id);
    }

    /// Writes the focus that style rules read from the focus registry. Returns whether the value
    /// changed.
    pub(crate) fn sync_focus_style(&mut self) -> bool {
        let node = self.focus.visual_focus(&self.tree, &self.style);
        self.tree.set_focus(node)
    }

    /// Reports a style warning of `node_id` through the sink, once for each node and line.
    pub(crate) fn warn_style(&mut self, node_id: NodeId, line: &str) {
        self.warnings.report(&self.tree, node_id, line);
    }

    /// Drops the document state of freed nodes and returns the callback ids their listeners held.
    fn forget(&mut self, ids: &[NodeId]) -> Vec<u32> {
        for &id in ids {
            self.warnings.forget(id);
            self.inputs.get_mut().remove(&id);
            self.focus.forget(id);
        }
        self.listeners.release_nodes(ids)
    }
}

impl WindowState {
    /// The box of node `id` in the last drawn frame, `None` for a node that frame did not draw.
    #[must_use]
    pub fn layout_of(&self, id: NodeId) -> Option<NodeLayout> {
        self.layout.borrow().get(&id).copied()
    }

    /// Queues a `scrollIntoViewNode` call whose node has no box yet for the next
    /// [`Host::reveal_focus`].
    pub(crate) fn queue_reveal(&mut self, node: NodeId, block: Align, inline: Align) {
        self.scroll_fresh.push((node, block, inline));
    }

    /// Returns a clone of the node's handle, creating it on first ask.
    pub(crate) fn scroll_handle(&self, node_id: NodeId) -> ScrollHandle {
        self.scroll_handles
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .clone()
    }

    /// Sets the scroll offset of `node_id` and of every node below it to zero.
    fn reset_scroll(&self, tree: &VirtualTree, node_id: NodeId) {
        let handles = self.scroll_handles.borrow();
        if handles.is_empty() {
            return;
        }
        let mut pending = vec![node_id];
        while let Some(id) = pending.pop() {
            if let Some(handle) = handles.get(&id) {
                handle.set_offset(gpui::Point::default());
            }
            if let Some(node) = tree.get(id) {
                pending.extend(node.children().iter().copied());
            }
        }
    }

    /// Drops the window state of freed nodes.
    fn forget(&mut self, ids: &[NodeId]) {
        for id in ids {
            self.scroll_handles.get_mut().remove(id);
            self.scroll_reported.remove(id);
            self.input_layouts.get_mut().remove(id);
        }
        for &id in ids {
            self.scroll_fresh.retain(|&(node, ..)| node != id);
            self.scroll_retry.retain(|&(node, ..)| node != id);
        }
    }

    /// The deepest container whose last drawn box holds the point.
    fn container_at(&self, tree: &VirtualTree, x: f32, y: f32) -> Option<NodeId> {
        let point = point(px(x), px(y));
        self.layout
            .borrow()
            .iter()
            .filter(|(_, layout)| layout.bounds.contains(&point))
            .map(|(&id, _)| id)
            .max_by_key(|&id| (tree.ancestors(id).count(), id))
    }

    /// Containers whose clamped offset moved since the last call. Every container's offset is
    /// recorded.
    fn moved_containers(&mut self) -> Vec<NodeId> {
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
        moved
    }

    /// Takes the queued `scrollIntoViewNode` calls, the ones that waited a frame first.
    fn take_scroll_calls(&mut self) -> (Vec<ScrollCall>, Vec<ScrollCall>) {
        (
            std::mem::take(&mut self.scroll_retry),
            std::mem::take(&mut self.scroll_fresh),
        )
    }

    /// Scrolls every scroll container above `node` so the node shows. Returns
    /// false when the last frame recorded no box for the node.
    pub(crate) fn reveal(
        &self,
        tree: &VirtualTree,
        style: &StyleCache,
        node: NodeId,
        block: Align,
        inline: Align,
    ) -> bool {
        let (own, chain) = {
            let layout = self.layout.borrow();
            let Some(own) = layout.get(&node) else {
                return false;
            };
            let mut chain = Vec::new();
            for id in tree.ancestors(node).skip(1) {
                if tree.get(id).is_none() {
                    break;
                }
                if is_scroll_container(tree, style, id)
                    && let Some(l) = layout.get(&id)
                {
                    chain.push((id, l.client_rect()));
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
            self.shift_descendants(tree, id, applied_x, applied_y);
            (applied_x, applied_y)
        });
        true
    }

    /// Moves the recorded boxes below `container` by the scroll just applied, so a
    /// second reveal before the next draw starts from where the content now is. The
    /// direct children also take it into their recorded scroll offset.
    pub(crate) fn shift_descendants(
        &self,
        tree: &VirtualTree,
        container: NodeId,
        dx: f32,
        dy: f32,
    ) {
        let mut layout = self.layout.borrow_mut();
        let mut stack = vec![container];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else {
                continue;
            };
            stack.extend(node.children());
            if id != container
                && let Some(l) = layout.get_mut(&id)
            {
                l.bounds.origin.x -= px(dx);
                l.bounds.origin.y -= px(dy);
                if tree.get(id).and_then(inca_node::VirtualNode::parent) == Some(container) {
                    l.scroll.x += px(dx);
                    l.scroll.y += px(dy);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::Host;

    #[test]
    fn container_at_without_a_box_gives_the_root_while_it_is_in_the_document() {
        let host = Host::default();

        assert_eq!(host.container_at(1.0, 1.0), host.document.borrow().root());
    }

    #[test]
    fn container_at_without_a_box_gives_the_document_after_the_root_left() {
        let host = Host::default();
        let (document, root) = {
            let state = host.document.borrow();
            (state.document(), state.root())
        };
        host.document
            .borrow_mut()
            .tree
            .remove_child(document, root)
            .unwrap();

        assert_eq!(host.container_at(1.0, 1.0), document);
    }
}
