// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What lives with the window: the boxes of the last drawn frame, the scroll handles, the held
//! mouse buttons and the active pointer capture, and the [`View`] the document reads them through.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{App, Pixels, ScrollHandle, Window, point, px};

use inca_document::geometry::{Align, scroll_chain};
use inca_document::{DocumentState, NodeBox, ScrollRange, View};
use inca_gpui::{InputLayout, NodeLayout, rect_of};
use inca_node::{NodeId, VirtualTree};
use inca_style::{StyleCache, is_scroll_container};

use crate::focus::FocusRegistry;

/// The two cells the dispatch of a window shares with its document: the document cell and the
/// window cell. A cell is borrowed for one step at a time. No borrow is held across a call into the
/// script, and nothing that runs under one cell's borrow needs another held cell mutably.
pub struct Cells {
    /// The tree, its cascade, the listeners, the signals and the focus model.
    pub document: Rc<RefCell<DocumentState>>,
    /// What the window drew and holds: boxes, scroll offsets, held buttons and the capture.
    pub window: Rc<RefCell<WindowState>>,
}

/// A `scrollIntoViewNode` call: the node and the block and inline alignment.
type ScrollCall = (NodeId, Align, Align);

/// The window cell of [`Cells`].
pub struct WindowState {
    /// Scroll state of the containers, created on first ask. Its own cell, so
    /// it can be written while a shared borrow of the window cell is held.
    pub(crate) scroll_handles: RefCell<HashMap<NodeId, ScrollHandle>>,
    /// The offset of each container as of the last `scroll` check.
    pub(crate) scroll_reported: HashMap<NodeId, gpui::Point<gpui::Pixels>>,
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
    pub(crate) scroll_fresh: Vec<ScrollCall>,
    pub(crate) scroll_retry: Vec<ScrollCall>,
    /// Receives one message each time the timers start a timer or the frames queue a callback.
    pub(crate) wake: async_channel::Receiver<()>,
    /// The focus handle of each node that can take focus.
    pub(crate) focus: FocusRegistry,
}

impl WindowState {
    /// A window state whose timer pump wakes on `wake`.
    #[must_use]
    pub fn new(wake: async_channel::Receiver<()>) -> Self {
        Self {
            scroll_handles: RefCell::new(HashMap::new()),
            scroll_reported: HashMap::new(),
            layout: RefCell::new(HashMap::new()),
            input_layouts: RefCell::new(HashMap::new()),
            held_buttons: 0,
            capture_active: None,
            scroll_fresh: Vec::new(),
            scroll_retry: Vec::new(),
            wake,
            focus: FocusRegistry::default(),
        }
    }
}

/// A scroll `offset` clamped into `[-max, 0]`, the range gpui draws.
fn clamped_offset(offset: Pixels, max: Pixels) -> Pixels {
    offset.clamp(-max, px(0.))
}

impl View for WindowState {
    fn node_box(&self, id: NodeId) -> Option<NodeBox> {
        self.layout_of(id).map(|layout| NodeBox {
            rect: rect_of(layout.bounds, px(0.0)),
            border: layout.border.into(),
            scroll: (layout.scroll.x.into(), layout.scroll.y.into()),
            gutter: (layout.gutter.width.into(), layout.gutter.height.into()),
        })
    }

    fn scroll_range(&self, id: NodeId) -> ScrollRange {
        let handle = self.scroll_handle(id);
        let (offset, max) = (handle.offset(), handle.max_offset());
        // gpui clamps on the next draw; report the same value.
        ScrollRange {
            offset: (
                (-clamped_offset(offset.x, max.x)).into(),
                (-clamped_offset(offset.y, max.y)).into(),
            ),
            max: (max.x.into(), max.y.into()),
        }
    }

    fn scroll_to(&self, tree: &VirtualTree, id: NodeId, left: Option<f64>, top: Option<f64>) {
        let handle = self.scroll_handle(id);
        let mut offset = handle.offset();
        // Narrowing is fine for pixel offsets; the upper clamp is gpui's.
        #[allow(clippy::cast_possible_truncation)]
        let negated = |v: f64| px(-(v.max(0.0) as f32));
        if let Some(left) = left {
            offset.x = negated(left);
        }
        if let Some(top) = top {
            offset.y = negated(top);
        }
        let (before, max) = (handle.offset(), handle.max_offset());
        let clamped = |v, max| -f32::from(clamped_offset(v, max));
        handle.set_offset(offset);
        // Recorded boxes follow the new offset, so a reveal in the same tick
        // starts from where the content now is.
        self.shift_descendants(
            tree,
            id,
            clamped(offset.x, max.x) - clamped(before.x, max.x),
            clamped(offset.y, max.y) - clamped(before.y, max.y),
        );
    }

    fn reveal(
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

    /// Queues a `scrollIntoViewNode` call whose node has no box yet for the next
    /// [`Cells::reveal_focus`].
    fn queue_reveal(&mut self, node: NodeId, block: Align, inline: Align) {
        self.scroll_fresh.push((node, block, inline));
    }

    fn held_buttons(&self) -> u8 {
        self.held_buttons
    }

    fn set_held_buttons(&mut self, held: u8) {
        self.held_buttons = held;
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
            self.focus.forget(id);
        }
    }
}

impl WindowState {
    /// The box of node `id` in the last drawn frame, `None` for a node that frame did not draw.
    #[must_use]
    pub fn layout_of(&self, id: NodeId) -> Option<NodeLayout> {
        self.layout.borrow().get(&id).copied()
    }

    /// Returns a clone of the node's handle, creating it on first ask.
    pub fn scroll_handle(&self, node_id: NodeId) -> ScrollHandle {
        self.scroll_handles
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .clone()
    }

    /// The deepest container whose last drawn box holds the point.
    pub(crate) fn container_at(&self, tree: &VirtualTree, x: f32, y: f32) -> Option<NodeId> {
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
    pub(crate) fn moved_containers(&mut self) -> Vec<NodeId> {
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
    pub(crate) fn take_scroll_calls(&mut self) -> (Vec<ScrollCall>, Vec<ScrollCall>) {
        (
            std::mem::take(&mut self.scroll_retry),
            std::mem::take(&mut self.scroll_fresh),
        )
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

impl Cells {
    /// The node holding focus in `window` right now. A node whose focus a tree change took is not
    /// held.
    #[must_use]
    pub fn focused_node(&self, window: &Window, cx: &App) -> Option<NodeId> {
        self.window
            .borrow()
            .focus
            .focused_node(&self.document.borrow().focus, window, cx)
    }

    /// The handle of `node`, if it can take focus.
    #[must_use]
    pub fn focus_handle(&self, node: NodeId) -> Option<gpui::FocusHandle> {
        self.window.borrow().focus.handle(node)
    }

    /// Frees every orphan of the tree with its subtree and drops their per-node state. Returns the
    /// freed node ids and the callback ids their listeners held, or `None` when nothing was
    /// freed.
    pub(crate) fn sweep_orphans(&self) -> Option<(Vec<NodeId>, Vec<u32>)> {
        let mut document = self.document.borrow_mut();
        let nodes = document.tree.sweep();
        if nodes.is_empty() {
            return None;
        }
        self.window.borrow_mut().forget(&nodes);
        let callbacks = document.forget(&nodes);
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
        let flags = inca_document::spec_of("scroll").flags;
        let document = self.document.borrow();
        moved.retain(|&id| {
            inca_document::listened(
                &document.tree,
                &document.listeners,
                "scroll",
                flags,
                inca_document::EventTargetId::Node(id),
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
    #[must_use]
    pub fn reveal_scrolls(&self) -> bool {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gpui::{Bounds, point, px};
    use inca_document::{ScrollRange, View};

    use crate::test_support::cells;

    #[test]
    fn container_at_without_a_box_gives_the_root_while_it_is_in_the_document() {
        let (cells, _wake) = cells();

        assert_eq!(cells.container_at(1.0, 1.0), cells.document.borrow().root());
    }

    #[test]
    fn container_at_without_a_box_gives_the_document_after_the_root_left() {
        let (cells, _wake) = cells();
        let (document, root) = {
            let state = cells.document.borrow();
            (state.document(), state.root())
        };
        cells
            .document
            .borrow_mut()
            .tree
            .remove_child(document, root)
            .unwrap();

        assert_eq!(cells.container_at(1.0, 1.0), document);
    }

    #[test]
    fn scroll_range_clamps_an_offset_past_the_end_to_the_range() {
        let (cells, _wake) = cells();
        let window = cells.window.borrow();
        window.scroll_handle(7).set_offset(point(px(-5.0), px(4.0)));

        assert_eq!(
            window.scroll_range(7),
            ScrollRange {
                offset: (0.0, 0.0),
                max: (0.0, 0.0)
            }
        );
    }

    #[test]
    fn a_scroll_moves_the_recorded_boxes_below_the_container() {
        let (cells, _wake) = crate::test_support::cells();
        let mut document = cells.document.borrow_mut();
        let container = document.root();
        let child = document.tree.create_node("div").unwrap();
        let grandchild = document.tree.create_node("div").unwrap();
        document.tree.append_child(container, child).unwrap();
        document.tree.append_child(child, grandchild).unwrap();
        let window = cells.window.borrow();
        let record = |id| {
            let layout = inca_gpui::NodeLayout {
                bounds: Bounds::new(point(px(10.0), px(50.0)), gpui::size(px(5.0), px(5.0))),
                border: px(0.0),
                scroll: point(px(1.0), px(2.0)),
                gutter: gpui::Size::default(),
            };
            window.layout.borrow_mut().insert(id, layout);
        };
        for id in [container, child, grandchild] {
            record(id);
        }

        window.shift_descendants(&document.tree, container, 3.0, 20.0);

        let layout = window.layout.borrow();
        let read = |id| {
            let l = layout[&id];
            (
                f32::from(l.bounds.origin.x),
                f32::from(l.bounds.origin.y),
                f32::from(l.scroll.x),
                f32::from(l.scroll.y),
            )
        };
        assert_eq!(read(container), (10.0, 50.0, 1.0, 2.0));
        assert_eq!(read(child), (7.0, 30.0, 4.0, 22.0));
        assert_eq!(read(grandchild), (7.0, 30.0, 1.0, 2.0));
    }

    #[test]
    fn sweep_orphans_drops_the_window_state_of_the_freed_nodes() {
        let (cells, _wake) = cells();
        let orphan = cells.document.borrow_mut().tree.create_node("div").unwrap();
        cells
            .window
            .borrow()
            .scroll_handle(orphan)
            .set_offset(point(px(-3.0), px(-4.0)));

        let swept = cells.sweep_orphans();

        assert_eq!(swept.map(|(nodes, _)| nodes), Some(vec![orphan]));
        assert!(
            !cells
                .window
                .borrow()
                .scroll_handles
                .borrow()
                .contains_key(&orphan)
        );
    }
}
