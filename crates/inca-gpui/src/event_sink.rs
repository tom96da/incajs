// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::Window;

use crate::tree::NodeId;

/// What [`crate::element`] needs from something that can dispatch a native
/// event into JS — `inca-bridge`'s `EventDispatcher` implements this, kept
/// as a trait here rather than a direct dependency so this crate never has
/// to depend on `QuickJS`.
pub trait EventSink {
    /// Whether anything is registered for `(node_id, event)`. The render
    /// path asks before wiring an element for input.
    fn listens(&self, node_id: NodeId, event: &str) -> bool;

    /// Calls whatever is registered for `(node_id, event)`.
    fn dispatch(&self, node_id: NodeId, event: &str, window: &mut Window);
}
