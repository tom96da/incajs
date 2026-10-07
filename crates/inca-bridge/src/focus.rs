// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persists GPUI's per-node focus state across frames — `FocusHandle`s
//! don't come from the retained tree, so nothing else keeps them alive —
//! and turns GPUI's own focus changes into the four focus events.
//!
//! `focusNode`/`blurNode` (see [`crate::bindings`]) run outside any GPUI render
//! or dispatch, with no `Window`/`App` to act on immediately — every
//! native binding closure only ever closes over `Rc<RefCell<Host>>`, never
//! a borrow that can't outlive the call that produced it. A request is
//! recorded here instead and applied the next time something does hold a
//! live `Window`/`App` — the host's own per-frame render, before building
//! the element tree.
//!
//! Dispatching is done by comparing which node is focused now against
//! which was focused last frame, rather than GPUI's own
//! `Window::on_focus_in`/`on_focus_out` — those return a `Subscription`
//! whose activation is deferred (`cx.defer`) to after the current update
//! finishes, so one registered in the same frame focus moves to it would
//! miss that very transition. A plain per-frame diff has no such gap.

use std::collections::{HashMap, HashSet, VecDeque};

use gpui::{App, FocusHandle, Window};

use inca_gpui::{
    AttributeValue, EventPayload, EventSink, NodeId, VirtualTree, hidden_by_attribute,
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
    /// whatever borrow produced this transition — see
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
    Focus(NodeId),
    Blur(NodeId),
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
    // Holds the window's focus while no node does, so key events reach the
    // root container. It maps to no node.
    parked: Option<FocusHandle>,
    pending: VecDeque<PendingFocus>,
    // Set while the window is inactive, after the focused node got its `blur`.
    window_blurred: bool,
    pub(crate) tab_dirty: HashSet<NodeId>,
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
    /// transition is reported by the next [`Self::apply_pending`].
    pub fn tab_move(
        &self,
        tree: &VirtualTree,
        root: NodeId,
        window: &mut Window,
        cx: &mut App,
        backward: bool,
    ) {
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
        let target = next_stop(&stops, self.focused_node(window, cx), backward)
            .and_then(|id| self.handles.get(&id));
        match target {
            Some(handle) => handle.focus(window, cx),
            None => window.blur(cx),
        }
    }

    /// The handle to `.track_focus(...)` `node_id` with, if it has one —
    /// backs [`EventSink::focus_handle`] for `EventDispatcher`.
    #[must_use]
    pub fn handle(&self, node_id: NodeId) -> Option<FocusHandle> {
        self.handles.get(&node_id).cloned()
    }

    /// The node holding focus in `window` right now.
    #[must_use]
    pub fn focused_node(&self, window: &Window, cx: &App) -> Option<NodeId> {
        window.focused(cx).and_then(|handle| self.node_for(&handle))
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
        let mut hidden = false;
        let mut cursor = Some(node_id);
        while let Some(ancestor) = cursor.and_then(|id| tree.get(id)) {
            let attrs = ancestor.attributes();
            hidden |= hidden_by_attribute(ancestor)
                || attrs.contains_key("inert")
                || matches!(
                    ancestor.style_props().get("display"),
                    Some(AttributeValue::String(value)) if value == "none"
                );
            cursor = ancestor.parent();
        }
        let button = node.tag_name() == "button";
        if node.tag_name() == "text"
            || hidden
            || (button && node.attributes().contains_key("disabled"))
        {
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

    /// Queues that `node_id` should be focused next frame.
    pub fn request_focus(&mut self, node_id: NodeId) {
        self.pending.push_back(PendingFocus::Focus(node_id));
    }

    /// Queues that `node_id` should be blurred next frame. Applying it
    /// blurs only if `node_id` is the focused node at that point.
    pub fn request_blur(&mut self, node_id: NodeId) {
        self.pending.push_back(PendingFocus::Blur(node_id));
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
            // Nothing queued this frame doesn't mean focus didn't move —
            // GPUI's own click-to-focus moves it without ever going
            // through `request_focus`.
            transitions.extend(self.sync(window, cx));
            return transitions;
        }
        for request in pending {
            match request {
                PendingFocus::Focus(node_id) => {
                    // Only a node with a handle takes focus.
                    if let Some(handle) = self.handles.get(&node_id) {
                        handle.focus(window, cx);
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
        self.pending
            .retain(|request| !matches!(request, PendingFocus::Focus(id) if *id == node_id));
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

    fn tree_with(tag: &str, tabindex: Option<AttributeValue>) -> (VirtualTree, NodeId) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node(tag).unwrap();
        if let Some(value) = tabindex {
            tree.set_attribute(id, "tabindex", value).unwrap();
        }
        (tree, id)
    }

    #[test]
    fn tab_index_parsing_follows_the_leading_integer() {
        let cases: [(&str, Option<i32>); 14] = [
            (" 1.5", Some(1)),
            ("+1", Some(1)),
            ("-0", Some(0)),
            ("-1", Some(-1)),
            ("\t\n 7x", Some(7)),
            ("1e3", Some(1)),
            ("abc", None),
            ("", None),
            ("+", None),
            ("-", None),
            (".5", None),
            ("2147483648", None),
            ("-2147483649", None),
            ("2147483647", Some(i32::MAX)),
        ];
        let mismatches: Vec<_> = cases
            .iter()
            .filter(|(text, want)| parse_tab_index(text) != *want)
            .collect();
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }

    #[test]
    fn tab_index_of_reads_strings_numbers_and_booleans() {
        let cases = [
            (AttributeValue::from("3"), Some(3)),
            (AttributeValue::Number(0.0), Some(0)),
            (AttributeValue::Number(-1.0), Some(-1)),
            (AttributeValue::Number(1.5), Some(1)),
            (AttributeValue::Number(1e21), Some(1)),
            (AttributeValue::Number(2.5e-7), Some(2)),
            (AttributeValue::Number(f64::NAN), None),
            (AttributeValue::Bool(true), None),
            (AttributeValue::Bool(false), None),
        ];
        for (value, want) in cases {
            let (tree, id) = tree_with("div", Some(value.clone()));
            assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{value:?}");
        }
        let (tree, id) = tree_with("div", None);
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
        assert_eq!(FocusRegistry::tab_index_of(&tree, 999), None);
    }

    #[test]
    fn a_text_node_and_a_display_none_node_have_no_tab_index() {
        let (tree, id) = tree_with("text", Some(AttributeValue::from("0")));
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        tree.set_style(id, "display", "none").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
        tree.set_style(id, "display", "flex").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
    }

    #[test]
    fn a_node_under_display_none_hidden_or_inert_has_no_tab_index() {
        let mut mismatches = Vec::new();
        for hide in ["display", "hidden", "inert"] {
            let (mut tree, child) = tree_with("div", Some(AttributeValue::from("0")));
            let parent = tree.create_node("div").unwrap();
            tree.append_child(parent, child).unwrap();
            for (target, name) in [(child, "self"), (parent, "ancestor")] {
                if hide == "display" {
                    tree.set_style(target, "display", "none").unwrap();
                } else {
                    tree.set_attribute(target, hide, true).unwrap();
                }
                if FocusRegistry::tab_index_of(&tree, child).is_some() {
                    mismatches.push(format!("{hide} on {name} keeps the tab index"));
                }
                if hide == "display" {
                    tree.remove_style(target, "display").unwrap();
                } else {
                    tree.remove_attribute(target, hide).unwrap();
                }
                if FocusRegistry::tab_index_of(&tree, child) != Some(0) {
                    mismatches.push(format!("removing {hide} on {name} loses the tab index"));
                }
            }
        }
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    #[test]
    fn hidden_with_an_own_display_or_until_found_keeps_the_tab_index() {
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        tree.set_attribute(id, "hidden", "").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
        tree.set_style(id, "display", "flex").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
        tree.remove_style(id, "display").unwrap();
        tree.set_attribute(id, "hidden", "until-found").unwrap();
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
    }

    #[gpui::test]
    fn mark_subtree_dirty_queues_nodes_that_can_hold_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
        let parent = tree.create_node("div").unwrap();
        let plain = tree.create_node("div").unwrap();
        let button = tree.create_node("button").unwrap();
        let held = tree.create_node("div").unwrap();
        for child in [focusable, plain, button, held] {
            tree.append_child(parent, child).unwrap();
        }
        let mut registry = FocusRegistry::default();
        cx.update(|_, cx| registry.get_or_create(held, cx));
        registry.mark_subtree_dirty(&tree, parent);
        assert_eq!(registry.tab_dirty, HashSet::from([focusable, button, held]));
    }

    #[test]
    fn a_button_has_tab_index_0_unless_told_otherwise() {
        let cases = [
            (None, Some(0)),
            (Some(AttributeValue::from("3")), Some(3)),
            (Some(AttributeValue::from("-1")), Some(-1)),
            (Some(AttributeValue::from("abc")), Some(0)),
            (Some(AttributeValue::Bool(true)), Some(0)),
        ];
        for (value, want) in cases {
            let (tree, id) = tree_with("button", value.clone());
            assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{value:?}");
        }
    }

    #[test]
    fn a_disabled_button_has_no_tab_index_but_a_disabled_div_keeps_it() {
        for (tag, want) in [("button", None), ("div", Some(2))] {
            let (mut tree, id) = tree_with(tag, Some(AttributeValue::from("2")));
            tree.set_attribute(id, "disabled", true).unwrap();
            assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{tag}");
            tree.remove_attribute(id, "disabled").unwrap();
            assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(2), "{tag}");
        }
    }

    // Nodes 1 to 6 have tabindex 3, 1, 0, 0, -1 and 2.
    const STOPS: [(NodeId, i32); 6] = [(1, 3), (2, 1), (3, 0), (4, 0), (5, -1), (6, 2)];

    fn walk(stops: &[(NodeId, i32)], start: Option<NodeId>, backward: bool) -> Vec<Option<NodeId>> {
        let mut current = start;
        let mut moves = Vec::new();
        for _ in 0..stops.len() + 2 {
            let target = next_stop(stops, current, backward);
            moves.push(target);
            current = target;
        }
        moves
    }

    #[test]
    fn next_stop_orders_positive_values_then_zeros_in_tree_order() {
        assert_eq!(
            walk(&STOPS, None, false),
            [
                Some(2),
                Some(6),
                Some(1),
                Some(3),
                Some(4),
                None,
                Some(2),
                Some(6)
            ]
        );
        assert_eq!(
            walk(&STOPS, None, true),
            [
                Some(4),
                Some(3),
                Some(1),
                Some(6),
                Some(2),
                None,
                Some(4),
                Some(3)
            ]
        );
    }

    #[test]
    fn next_stop_from_a_negative_stop_continues_in_tree_order() {
        assert_eq!(next_stop(&STOPS, Some(5), false), Some(6));
        assert_eq!(next_stop(&STOPS, Some(5), true), Some(4));
        let stops = [(1, 3), (2, -1), (3, 0), (4, 1)];
        assert_eq!(next_stop(&stops, Some(2), false), Some(3));
        assert_eq!(next_stop(&stops, Some(2), true), Some(1));
        let stops = [(1, 1), (2, 0), (3, -1)];
        assert_eq!(next_stop(&stops, Some(3), false), None);
        let stops = [(1, -1), (2, 1)];
        assert_eq!(next_stop(&stops, Some(1), true), None);
    }

    #[test]
    fn next_stop_starts_at_an_end_from_an_unknown_node_and_ignores_empty_lists() {
        assert_eq!(next_stop(&STOPS, Some(99), false), Some(2));
        assert_eq!(next_stop(&STOPS, Some(99), true), Some(4));
        for backward in [false, true] {
            assert_eq!(next_stop(&[], None, backward), None);
            assert_eq!(next_stop(&[(1, -1)], None, backward), None);
        }
    }

    #[gpui::test]
    fn changing_tabindex_keeps_the_focused_handle(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        let before = registry.handle(id).unwrap();

        tree.set_attribute(id, "tabindex", "-1").unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        let after = registry.handle(id).unwrap();
        assert_eq!(before, after);
        assert!(transitions.is_empty());
        assert!(cx.update(|window, _| after.is_focused(window)));
    }

    #[gpui::test]
    fn removing_tabindex_blurs_the_focused_node_and_drops_the_handle(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.request_focus(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        let held = registry.handle(id).unwrap();
        assert!(cx.update(|window, _| held.is_focused(window)));

        tree.remove_attribute(id, "tabindex").unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(registry.handle(id).is_none());
        assert!(!cx.update(|window, _| held.is_focused(window)));
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].blurred, Some(id));
        assert_eq!(transitions[0].focused, None);
    }

    #[gpui::test]
    fn focus_on_an_untracked_node_is_ignored(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", None);
        let mut registry = FocusRegistry::default();
        registry.request_focus(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));
        assert!(transitions.is_empty());
        assert!(registry.handle(id).is_none());
    }

    #[gpui::test]
    fn removing_tabindex_blurs_a_node_a_click_focused_before_the_next_sync(
        cx: &mut TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        let held = registry.handle(id).unwrap();
        // Focus moved outside the registry, as a click does.
        cx.update(|window, cx| held.focus(window, cx));

        tree.remove_attribute(id, "tabindex").unwrap();
        registry.mark_tab_dirty(id);
        let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

        assert!(!cx.update(|window, _| held.is_focused(window)));
        let events: Vec<_> = transitions.iter().map(|t| (t.blurred, t.focused)).collect();
        assert_eq!(events, [(None, Some(id)), (Some(id), None)]);
    }

    #[gpui::test]
    fn an_unparsable_tabindex_drops_the_handle(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        tree.set_attribute(id, "tabindex", "abc").unwrap();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        assert!(registry.handle(id).is_none());
    }

    #[gpui::test]
    fn forget_clears_a_queued_tab_check(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        registry.forget(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
        });
        assert!(registry.handle(id).is_none());
    }

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

    #[gpui::test]
    fn focused_node_names_the_node_holding_focus(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        let (tree, id) = tree_with("div", Some(AttributeValue::from("0")));
        let mut registry = FocusRegistry::default();
        registry.mark_tab_dirty(id);
        cx.update(|window, cx| {
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.focused_node(window, cx), None);
            registry.request_focus(id);
            let _ = registry.apply_pending(&tree, window, cx);
            assert_eq!(registry.focused_node(window, cx), Some(id));
        });
    }
}
