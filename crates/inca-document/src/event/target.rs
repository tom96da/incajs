// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use inca_node::NodeId;

/// Something an event can be dispatched at or travel through: a node of the
/// tree, the tag `"$window"`, which is no node, or an `AbortSignal`, which
/// Rust knows by its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventTargetId {
    /// A node of the tree.
    Node(NodeId),
    /// The window.
    Window,
    /// An `AbortSignal`, by its signal id.
    Signal(u32),
}
