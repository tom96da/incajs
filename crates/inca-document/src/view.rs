// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the document reads from the last drawn frame and writes back to the window: the boxes, the
//! scroll offsets, the held mouse buttons and the queue of reveals that wait for a box.

use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;

use crate::geometry::{Align, Rect};

/// The box of a node in the last drawn frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeBox {
    /// The border box in window coordinates.
    pub rect: Rect,
    /// The width of the border on every side.
    pub border: f32,
    /// The scroll offset the parent applied to the box when it was recorded.
    pub scroll: (f32, f32),
    /// The width of the vertical scrollbar gutter and the height of the horizontal one.
    pub gutter: (f32, f32),
}

/// The scroll position of a container: how far its content moved towards the end of each axis, and
/// how far it can move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollRange {
    /// The offset in `[0, max]` on each axis, as `(left, top)`.
    pub offset: (f32, f32),
    /// The largest offset on each axis, as `(left, top)`.
    pub max: (f32, f32),
}

/// The window as the document sees it.
pub trait View {
    /// The box of `id` in the last drawn frame, `None` for a node that frame did not draw.
    fn node_box(&self, id: NodeId) -> Option<NodeBox>;

    /// The scroll position of `id`.
    fn scroll_range(&self, id: NodeId) -> ScrollRange;

    /// Moves the scroll offset of `id` on the given axes. A negative value counts as `0`. The
    /// recorded boxes below `id` follow the move.
    fn scroll_to(&self, tree: &VirtualTree, id: NodeId, left: Option<f64>, top: Option<f64>);

    /// Scrolls every scroll container above `node` so the node shows. Returns `false` when the
    /// last frame recorded no box for the node.
    fn reveal(
        &self,
        tree: &VirtualTree,
        style: &StyleCache,
        node: NodeId,
        block: Align,
        inline: Align,
    ) -> bool;

    /// Queues a reveal of `node` whose box the last frame lacks, for the next frame.
    fn queue_reveal(&mut self, node: NodeId, block: Align, inline: Align);

    /// The mouse buttons held now, as the `buttons` bitmask of the events.
    fn held_buttons(&self) -> u8;

    /// Records the mouse buttons held now.
    fn set_held_buttons(&mut self, held: u8);
}
