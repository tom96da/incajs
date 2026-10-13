// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persists GPUI's per-node focus state across frames (`FocusHandle`s
//! don't come from the retained tree, so nothing else keeps them alive)
//! and turns GPUI's own focus changes into the four focus events.
//!
//! A focus or blur request arrives outside any GPUI render or dispatch, with
//! no `Window`/`App` to act on immediately. The request is recorded here and
//! applied the next time something holds a live `Window`/`App`: the host's
//! per-frame render, before it builds the element tree.
//!
//! Dispatching is done by comparing which node is focused now against
//! which was focused last frame, rather than GPUI's own
//! `Window::on_focus_in`/`on_focus_out`. Those return a `Subscription`
//! whose activation is deferred (`cx.defer`) to after the current update
//! finishes, so one registered in the same frame focus moves to it would
//! miss that very transition. A plain per-frame diff has no such gap.

use std::collections::{HashMap, HashSet, VecDeque};

use gpui::{App, FocusHandle, Window};

use crate::{
    AttributeValue, EventPayload, EventSink, NodeId, VirtualTree, hidden_by_attribute,
    is_disabled_button,
};

/// What changed since [`FocusRegistry::apply_pending`] was last called.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusTransition {
    blurred: Option<NodeId>,
    focused: Option<NodeId>,
}

impl FocusTransition {
    /// The window lost (`active` false) or regained the focus of `node`. The
    /// node keeps its focus in the tree. No other node is involved.
    #[must_use]
    pub fn window_activation(node: NodeId, active: bool) -> Self {
        Self {
            blurred: (!active).then_some(node),
            focused: active.then_some(node),
        }
    }

    /// Dispatches `blur`, `focusout`, `focus` and `focusin`, in that order,
    /// for whichever of `blurred`/`focused` this transition carries. Each
    /// carries the other node as `relatedTarget`. Call after releasing
    /// whatever borrow produced this transition. See
    /// [`FocusRegistry::apply_pending`].
    pub fn dispatch(
        &self,
        dispatch: &(impl EventSink + Clone + 'static),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(blurred) = self.blurred {
            let payload = EventPayload::Focus {
                related_target: self.focused,
            };
            for event in ["blur", "focusout"] {
                dispatch.fire(blurred, event, &payload, window, cx);
            }
        }
        if let Some(focused) = self.focused {
            let payload = EventPayload::Focus {
                related_target: self.blurred,
            };
            for event in ["focus", "focusin"] {
                dispatch.fire(focused, event, &payload, window, cx);
            }
        }
    }
}

/// What a `focusNode`/`blurNode` call left for the next frame to apply.
#[derive(Debug, Clone, Copy)]
enum PendingFocus {
    /// Focus the node, and scroll it into view when `scroll` is set.
    Focus {
        node: NodeId,
        scroll: bool,
    },
    Blur(NodeId),
}

/// `NodeId` → its persistent `FocusHandle`, the node focused as of the last
/// frame this was checked, and every pending request left by a binding
/// that had no live `Window`/`App`, oldest first. See the module doc for
/// why. Two requests queued before the next frame both apply, in order.
#[derive(Debug, Default)]
pub struct FocusRegistry {
    handles: HashMap<NodeId, FocusHandle>,
    focused: Option<NodeId>,
    // The focused node as the window last reported it. See `observe`.
    observed: Option<NodeId>,
    // Holds the window's focus while no node does, so key events reach the
    // root container. It maps to no node.
    parked: Option<FocusHandle>,
    pending: VecDeque<PendingFocus>,
    // The latest focused node waiting to be scrolled into view, and the
    // node found lacking a box that waits for the next frame.
    reveal: Vec<NodeId>,
    retry: Vec<NodeId>,
    // Set while the window is inactive, after the focused node got its `blur`.
    window_blurred: bool,
    /// Nodes whose `tabindex` changed since the host last took them.
    pub tab_dirty: HashSet<NodeId>,
}

/// Parses a `tabindex` value: optional leading ASCII whitespace, an optional
/// sign, then digits. Trailing characters are ignored. Returns `None` when the
/// digits are missing or exceed `i32`.
fn parse_tab_index(value: &str) -> Option<i32> {
    let value = value.trim_start_matches([' ', '\t', '\n', '\x0c', '\r']);
    let (negative, rest) = match value.as_bytes().first()? {
        b'-' => (true, &value[1..]),
        b'+' => (false, &value[1..]),
        _ => (false, value),
    };
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let magnitude: i64 = rest[..digits].parse().ok()?;
    i32::try_from(if negative { -magnitude } else { magnitude }).ok()
}

// The text JavaScript makes of a number. Only the leading digits matter, and
// Rust's `{:e}` and JavaScript's exponent form share them.
fn number_text(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude >= 1e21 || (magnitude != 0.0 && magnitude < 1e-6) {
        format!("{value:e}")
    } else {
        value.to_string()
    }
}

/// The target of a Tab press. `stops` lists every focusable node with its
/// `tabindex` in tree order. Positive values come first in ascending order,
/// then 0 in tree order, and negative values are outside the order. Past either
/// end the result is `None`. From a node outside the order the result is the
/// nearest stop after it (before it when `backward`) in tree order. `current`
/// `None` starts at the first (or, `backward`, last) stop.
fn next_stop(stops: &[(NodeId, i32)], current: Option<NodeId>, backward: bool) -> Option<NodeId> {
    // (sort key, tree position, node)
    let mut ordered: Vec<(i32, usize, NodeId)> = stops
        .iter()
        .enumerate()
        .filter(|(_, (_, index))| *index >= 0)
        .map(|(pos, &(id, index))| (if index > 0 { index } else { i32::MAX }, pos, id))
        .collect();
    ordered.sort_by_key(|&(key, ..)| key);
    let at = current.and_then(|id| ordered.iter().position(|&(_, _, node)| node == id));
    let found = match (current, at) {
        (Some(_), Some(at)) => {
            if backward {
                at.checked_sub(1).and_then(|i| ordered.get(i))
            } else {
                ordered.get(at + 1)
            }
        }
        (Some(id), None) => match stops.iter().position(|&(node, _)| node == id) {
            Some(here) if backward => ordered.iter().filter(|s| s.1 < here).max_by_key(|s| s.1),
            Some(here) => ordered.iter().filter(|s| s.1 > here).min_by_key(|s| s.1),
            None if backward => ordered.last(),
            None => ordered.first(),
        },
        (None, _) if backward => ordered.last(),
        (None, _) => ordered.first(),
    };
    found.map(|&(_, _, id)| id)
}

impl FocusRegistry {
    /// Moves focus one tab stop forward, or `backward`. Past the last stop
    /// nothing holds focus, and the next press starts over at the first. The
    /// transition is reported by the next [`Self::apply_pending`]. Returns
    /// the node that took focus. The move also drops the scroll requests of
    /// earlier focus calls.
    pub fn tab_move(
        &mut self,
        tree: &VirtualTree,
        root: NodeId,
        window: &mut Window,
        cx: &mut App,
        backward: bool,
    ) -> Option<NodeId> {
        let mut stops = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if self.handles.contains_key(&id)
                && let Some(index) = Self::tab_index_of(tree, id)
            {
                stops.push((id, index));
            }
            stack.extend(node.children().iter().rev());
        }
        let target = next_stop(&stops, self.focused_node(window, cx), backward);
        self.reveal.clear();
        self.retry.clear();
        if let Some((id, handle)) = target.and_then(|id| Some((id, self.handles.get(&id)?))) {
            handle.focus(window, cx);
            Some(id)
        } else {
            window.blur(cx);
            None
        }
    }

    /// The handle to `.track_focus(...)` `node_id` with, if it has one.
    /// Backs [`EventSink::focus_handle`] for `EventDispatcher`.
    #[must_use]
    pub fn handle(&self, node_id: NodeId) -> Option<FocusHandle> {
        self.handles.get(&node_id).cloned()
    }

    /// Whether any node can take focus.
    #[must_use]
    pub fn has_handles(&self) -> bool {
        !self.handles.is_empty()
    }

    /// The node holding focus in `window` right now.
    #[must_use]
    pub fn focused_node(&self, window: &Window, cx: &App) -> Option<NodeId> {
        window.focused(cx).and_then(|handle| self.node_for(&handle))
    }

    /// Records the node the window focuses now. Call it before a read of
    /// [`Self::active_node`], so the read starts from the focus GPUI applied
    /// itself.
    pub fn observe(&mut self, window: &Window, cx: &App) {
        self.observed = self.focused_node(window, cx);
    }

    /// The node holding focus once the queued requests apply. It starts from
    /// the node last observed, drops it when a queued check removes its
    /// focusability, then replays the queue in order.
    #[must_use]
    pub fn active_node(&self, tree: &VirtualTree) -> Option<NodeId> {
        let mut active = self
            .observed
            .filter(|&id| self.focusable_after_frame(tree, id));
        for request in &self.pending {
            match *request {
                PendingFocus::Focus { node: id, .. } => {
                    if self.focusable_after_frame(tree, id) {
                        active = Some(id);
                    }
                }
                PendingFocus::Blur(id) => {
                    if active == Some(id) {
                        active = None;
                    }
                }
            }
        }
        active
    }

    /// Whether `node_id` can hold focus once this frame's tab check has run.
    /// A node in `tab_dirty` holds it when it has a tab index, and any other
    /// node holds it when it has a handle.
    fn focusable_after_frame(&self, tree: &VirtualTree, node_id: NodeId) -> bool {
        if self.tab_dirty.contains(&node_id) {
            Self::tab_index_of(tree, node_id).is_some()
        } else {
            self.handles.contains_key(&node_id)
        }
    }

    /// Records that `node_id`'s `tabindex` may have changed. The next
    /// [`Self::apply_pending`] re-reads it.
    pub fn mark_tab_dirty(&mut self, node_id: NodeId) {
        self.tab_dirty.insert(node_id);
    }

    /// Like [`Self::mark_tab_dirty`] for `node_id` and its descendants that
    /// have, or could take, a handle.
    pub fn mark_subtree_dirty(&mut self, tree: &VirtualTree, node_id: NodeId) {
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get(id) else { continue };
            if self.handles.contains_key(&id)
                || node.tag_name() == "button"
                || node.attributes().contains_key("tabindex")
            {
                self.tab_dirty.insert(id);
            }
            stack.extend(node.children());
        }
    }

    /// The `tabindex` of `node_id`. A text node, a node that has an ancestor
    /// (or itself) with `display: none`, `inert` or a hiding `hidden`, and a
    /// `disabled` button have none. A button whose `tabindex` is missing or
    /// unparsable has 0.
    fn tab_index_of(tree: &VirtualTree, node_id: NodeId) -> Option<i32> {
        let node = tree.get(node_id)?;
        let hidden = tree
            .ancestors(node_id)
            .filter_map(|id| tree.get(id))
            .any(|ancestor| {
                hidden_by_attribute(ancestor)
                    || ancestor.attributes().contains_key("inert")
                    || matches!(
                        ancestor.style_props().get("display"),
                        Some(AttributeValue::String(value)) if value == "none"
                    )
            });
        let button = node.tag_name() == "button";
        if node.tag_name() == "text" || hidden || is_disabled_button(node) {
            return None;
        }
        let explicit = match node.attributes().get("tabindex") {
            Some(AttributeValue::String(value)) => parse_tab_index(value),
            Some(AttributeValue::Number(value)) => parse_tab_index(&number_text(*value)),
            Some(AttributeValue::Bool(_)) | None => None,
        };
        explicit.or(button.then_some(0))
    }

    /// The handle the root container tracks, once [`Self::apply_pending`]
    /// has run. It holds the window's focus while no node is focused.
    #[must_use]
    pub fn parked_handle(&self) -> Option<FocusHandle> {
        self.parked.clone()
    }

    /// Records that the window lost (`blurred`) or regained focus after the
    /// focused node's `blur` or `focus` was reported.
    pub fn set_window_blurred(&mut self, blurred: bool) {
        self.window_blurred = blurred;
    }

    /// Queues that `node_id` should be focused next frame. With `scroll`,
    /// the host then scrolls it into view.
    pub fn request_focus(&mut self, node_id: NodeId, scroll: bool) {
        self.pending.push_back(PendingFocus::Focus {
            node: node_id,
            scroll,
        });
    }

    /// Queues that `node_id` should be blurred next frame. Applying it
    /// blurs only if `node_id` is the focused node at that point.
    pub fn request_blur(&mut self, node_id: NodeId) {
        self.pending.push_back(PendingFocus::Blur(node_id));
    }

    /// Applies every queued request in order and reports each transition
    /// it produced. Call once per frame, before the element tree that
    /// needs the resulting `track_focus` wiring is built.
    ///
    /// Returns the transitions rather than dispatching them directly: the
    /// caller holds `self` through a borrow of the same `Rc<RefCell<Host>>`
    /// that `EventSink::fire` re-borrows internally to look up
    /// callbacks, so dispatching from inside this call would panic with
    /// "already mutably borrowed". The caller dispatches after this
    /// returns, once that borrow is released.
    #[must_use]
    pub fn apply_pending(
        &mut self,
        tree: &VirtualTree,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<FocusTransition> {
        let transitions = self.apply_requests(tree, window, cx);
        let parked = self.parked.get_or_insert_with(|| cx.focus_handle());
        if window.focused(cx).is_none() {
            parked.focus(window, cx);
        }
        transitions
    }

    fn apply_requests(
        &mut self,
        tree: &VirtualTree,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<FocusTransition> {
        let mut transitions = Vec::new();
        for node_id in std::mem::take(&mut self.tab_dirty) {
            match Self::tab_index_of(tree, node_id) {
                Some(_) => {
                    self.get_or_create(node_id, cx);
                }
                None => {
                    if let Some(handle) = self.handles.get(&node_id).cloned() {
                        // Report a click focus first, then the blur, so JS
                        // sees both. `forget` would skip the blur.
                        transitions.extend(self.sync(window, cx));
                        if handle.is_focused(window) {
                            window.blur(cx);
                        }
                        self.handles.remove(&node_id);
                        transitions.extend(self.sync(window, cx));
                    }
                }
            }
        }
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            // Nothing queued this frame doesn't mean focus didn't move:
            // GPUI's own click-to-focus moves it without ever going
            // through `request_focus`.
            transitions.extend(self.sync(window, cx));
            return transitions;
        }
        for request in pending {
            match request {
                PendingFocus::Focus { node, scroll } => {
                    // Only a node with a handle takes focus.
                    if let Some(handle) = self.handles.get(&node) {
                        handle.focus(window, cx);
                        // The last focus decides what scrolls.
                        self.reveal.clear();
                        self.retry.clear();
                        if scroll {
                            self.reveal.push(node);
                        }
                    }
                }
                PendingFocus::Blur(node_id) => {
                    let focused = window.focused(cx).and_then(|handle| self.node_for(&handle));
                    if focused == Some(node_id) {
                        window.blur(cx);
                    }
                }
            }
            transitions.extend(self.sync(window, cx));
        }
        transitions
    }

    /// Hands over the focused nodes to scroll into view this frame:
    /// `(retries, fresh)`. A retry is a node whose box the previous frame
    /// lacked.
    pub fn take_reveals(&mut self) -> (Vec<NodeId>, Vec<NodeId>) {
        (
            std::mem::take(&mut self.retry),
            std::mem::take(&mut self.reveal),
        )
    }

    /// Keeps `node_id` for one more frame.
    pub fn retry_reveal(&mut self, node_id: NodeId) {
        self.retry.push(node_id);
    }

    /// Reports a transition if which node is focused changed since the
    /// last call, including a change from GPUI's own click-to-focus, not
    /// only one made through [`Self::request_focus`].
    fn sync(&mut self, window: &mut Window, cx: &mut App) -> Option<FocusTransition> {
        let focused_now = window.focused(cx).and_then(|handle| self.node_for(&handle));
        self.observed = focused_now;
        if focused_now == self.focused {
            return None;
        }
        let blurred = self.focused;
        self.focused = focused_now;
        // An inactive window holds the events. Its deactivation already
        // blurred the old node and its activation focuses the new one.
        if self.window_blurred || (blurred.is_none() && focused_now.is_none()) {
            return None;
        }
        Some(FocusTransition {
            blurred,
            focused: focused_now,
        })
    }

    /// Drops `node_id`'s handle and any queued request to focus it.
    /// `destroyNode` calls this for every node a destroyed subtree freed,
    /// mirroring how it already frees callback ids.
    pub fn forget(&mut self, node_id: NodeId) {
        self.handles.remove(&node_id);
        self.tab_dirty.remove(&node_id);
        self.pending.retain(
            |request| !matches!(request, PendingFocus::Focus { node, .. } if *node == node_id),
        );
        self.reveal.retain(|&id| id != node_id);
        self.retry.retain(|&id| id != node_id);
        if self.focused == Some(node_id) {
            // The node is gone: nothing left to dispatch a "blur" to.
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
mod tests;
