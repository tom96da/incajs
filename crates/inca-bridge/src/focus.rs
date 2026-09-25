// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persists GPUI's per-node focus state across frames — `FocusHandle`s
//! don't come from the retained tree, so nothing else keeps them alive —
//! and turns GPUI's own focus changes into `"focus"`/`"blur"` dispatches.
//!
//! `focusNode`/`blurNode` (`crate::bindings`) run outside any GPUI render
//! or dispatch, with no `Window`/`App` to act on immediately — every
//! native binding closure only ever closes over `Rc<RefCell<Host>>`, never
//! a borrow that can't outlive the call that produced it. A request is
//! recorded here instead and applied the next time something does hold a
//! live `Window`/`App`: `crates/inca-host/src/main.rs`'s `HostedApp::render`,
//! once per frame, before building the element tree.
//!
//! Dispatching is done by comparing which node is focused now against
//! which was focused last frame, rather than GPUI's own
//! `Window::on_focus_in`/`on_focus_out` — those return a `Subscription`
//! whose activation is deferred (`cx.defer`) to after the current update
//! finishes, so one registered in the same frame focus moves to it would
//! miss that very transition. A plain per-frame diff has no such gap.

use std::collections::{HashMap, VecDeque};

use gpui::{App, FocusHandle, Window};

use inca_gpui::{EventPayload, EventSink, NodeId};

/// What changed since [`FocusRegistry::apply_pending`] was last called.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusTransition {
    blurred: Option<NodeId>,
    focused: Option<NodeId>,
}

impl FocusTransition {
    /// Dispatches `"blur"`/`"focus"` for whichever of `blurred`/`focused`
    /// this transition carries. Call after releasing whatever borrow
    /// produced this transition — see [`FocusRegistry::apply_pending`].
    pub fn dispatch(
        &self,
        dispatch: &(impl EventSink + Clone + 'static),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(blurred) = self.blurred {
            dispatch.dispatch(blurred, "blur", &EventPayload::None, window, cx);
        }
        if let Some(focused) = self.focused {
            dispatch.dispatch(focused, "focus", &EventPayload::None, window, cx);
        }
    }
}

/// What a `focusNode`/`blurNode` call left for the next frame to apply.
#[derive(Debug, Clone, Copy)]
enum PendingFocus {
    Focus(NodeId),
    Blur,
}

/// `NodeId` → its persistent `FocusHandle`, the node focused as of the last
/// frame this was checked, and every pending request left by a binding
/// that had no live `Window`/`App`, oldest first — see the module doc for
/// why. Two requests queued before the next frame both apply, in order —
/// the same as the DOM's synchronous `.focus()`.
#[derive(Debug, Default)]
pub struct FocusRegistry {
    handles: HashMap<NodeId, FocusHandle>,
    focused: Option<NodeId>,
    pending: VecDeque<PendingFocus>,
}

impl FocusRegistry {
    /// The handle to `.track_focus(...)` `node_id` with, if it has one —
    /// backs [`EventSink::focus_handle`] for `EventDispatcher`.
    #[must_use]
    pub fn handle(&self, node_id: NodeId) -> Option<FocusHandle> {
        self.handles.get(&node_id).cloned()
    }

    /// Queues that `node_id` should be focused next frame.
    pub fn request_focus(&mut self, node_id: NodeId) {
        self.pending.push_back(PendingFocus::Focus(node_id));
    }

    /// Queues that whatever's focused at that point should be blurred next
    /// frame.
    pub fn request_blur(&mut self) {
        self.pending.push_back(PendingFocus::Blur);
    }

    /// Applies every queued request in order and reports each transition
    /// it produced — called once per frame, before the element tree that
    /// needs the resulting `track_focus` wiring is built.
    ///
    /// Returns the transitions rather than dispatching them directly: the
    /// caller holds `self` through a borrow of the same `Rc<RefCell<Host>>`
    /// that `EventSink::dispatch` re-borrows internally to look up
    /// callbacks, so dispatching from inside this call would panic with
    /// "already mutably borrowed". The caller dispatches after this
    /// returns, once that borrow is released.
    #[must_use]
    pub fn apply_pending(&mut self, window: &mut Window, cx: &mut App) -> Vec<FocusTransition> {
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            // Nothing queued this frame doesn't mean focus didn't move —
            // GPUI's own click-to-focus moves it without ever going
            // through `request_focus`.
            return self.sync(window, cx).into_iter().collect();
        }
        let mut transitions = Vec::with_capacity(pending.len());
        for request in pending {
            match request {
                PendingFocus::Focus(node_id) => {
                    let handle = self.get_or_create(node_id, cx);
                    handle.focus(window, cx);
                }
                PendingFocus::Blur => window.blur(cx),
            }
            transitions.extend(self.sync(window, cx));
        }
        transitions
    }

    /// Reports a transition if which node is focused changed since the
    /// last call, including a change from GPUI's own click-to-focus, not
    /// only one made through [`Self::request_focus`].
    fn sync(&mut self, window: &mut Window, cx: &mut App) -> Option<FocusTransition> {
        let focused_now = window.focused(cx).and_then(|handle| self.node_for(&handle));
        if focused_now == self.focused {
            return None;
        }
        let blurred = self.focused;
        self.focused = focused_now;
        Some(FocusTransition {
            blurred,
            focused: focused_now,
        })
    }

    /// Drops `node_id`'s handle, if it has one. `destroyNode` calls this
    /// for every node a destroyed subtree freed, mirroring how it already
    /// frees callback ids.
    pub fn forget(&mut self, node_id: NodeId) {
        self.handles.remove(&node_id);
        if self.focused == Some(node_id) {
            // The node is gone — nothing left to dispatch a "blur" to.
            self.focused = None;
        }
    }

    fn get_or_create(&mut self, node_id: NodeId, cx: &mut App) -> FocusHandle {
        self.handles
            .entry(node_id)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    fn node_for(&self, handle: &FocusHandle) -> Option<NodeId> {
        self.handles
            .iter()
            .find(|(_, h)| *h == handle)
            .map(|(&id, _)| id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn get_or_create_reuses_the_same_handle_for_one_node(cx: &mut TestAppContext) {
        let mut registry = FocusRegistry::default();
        let cx = cx.add_empty_window();

        let (first, second) = cx.update(|_, cx| {
            let first = registry.get_or_create(1, cx);
            let second = registry.get_or_create(1, cx);
            (first, second)
        });

        assert_eq!(first, second);
    }

    #[gpui::test]
    fn forget_lets_a_later_call_mint_a_new_handle(cx: &mut TestAppContext) {
        let mut registry = FocusRegistry::default();
        let cx = cx.add_empty_window();

        let before = cx.update(|_, cx| registry.get_or_create(1, cx));
        registry.forget(1);
        let after = cx.update(|_, cx| registry.get_or_create(1, cx));

        assert_ne!(before, after);
        assert!(registry.handle(1).is_some());
    }

    #[gpui::test]
    fn handle_is_none_for_a_node_never_asked_for(cx: &mut TestAppContext) {
        let registry = FocusRegistry::default();
        let _cx = cx.add_empty_window();

        assert!(registry.handle(1).is_none());
    }
}
