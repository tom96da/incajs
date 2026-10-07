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

use inca_gpui::{AttributeValue, EventPayload, EventSink, NodeId, VirtualTree};

/// What changed since [`FocusRegistry::apply_pending`] was last called.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusTransition {
    blurred: Option<NodeId>,
    focused: Option<NodeId>,
}

impl FocusTransition {
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
                dispatch.dispatch(blurred, event, &payload, window, cx);
            }
        }
        if let Some(focused) = self.focused {
            let payload = EventPayload::Focus {
                related_target: self.blurred,
            };
            for event in ["focus", "focusin"] {
                dispatch.dispatch(focused, event, &payload, window, cx);
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
    pending: VecDeque<PendingFocus>,
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

/// The gpui `(tab_index, tab_stop)` for a web `tabindex`. gpui orders tab
/// stops ascending, so web order 0 maps after every positive value.
fn gpui_tab_order(index: i32) -> (isize, bool) {
    if index > 0 {
        (isize::try_from(index).unwrap_or(isize::MAX), true)
    } else {
        (isize::MAX, index == 0)
    }
}

impl FocusRegistry {
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

    /// The `tabindex` of `node_id`. A text node, a node whose own `display`
    /// is `none` and a `disabled` button have none. A button whose
    /// `tabindex` is missing or unparsable has 0.
    fn tab_index_of(tree: &VirtualTree, node_id: NodeId) -> Option<i32> {
        let node = tree.get(node_id)?;
        let hidden = matches!(
            node.style_props().get("display"),
            Some(AttributeValue::String(value)) if value == "none"
        );
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
        let mut transitions = Vec::new();
        for node_id in std::mem::take(&mut self.tab_dirty) {
            match Self::tab_index_of(tree, node_id) {
                Some(index) => {
                    let (order, stop) = gpui_tab_order(index);
                    let handle = self
                        .get_or_create(node_id, cx)
                        .tab_index(order)
                        .tab_stop(stop);
                    self.handles.insert(node_id, handle);
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

    #[test]
    fn gpui_tab_order_maps_web_values() {
        assert_eq!(gpui_tab_order(3), (3, true));
        assert_eq!(gpui_tab_order(0), (isize::MAX, true));
        assert_eq!(gpui_tab_order(-1), (isize::MAX, false));
    }

    #[gpui::test]
    fn apply_pending_gives_a_handle_with_the_mapped_order(cx: &mut TestAppContext) {
        let cx = cx.add_empty_window();
        for (text, order, stop) in [
            ("2", 2, true),
            ("0", isize::MAX, true),
            ("-1", isize::MAX, false),
        ] {
            let (tree, id) = tree_with("div", Some(AttributeValue::from(text)));
            let mut registry = FocusRegistry::default();
            registry.mark_tab_dirty(id);
            cx.update(|window, cx| {
                let _ = registry.apply_pending(&tree, window, cx);
            });
            let handle = registry.handle(id).unwrap();
            assert_eq!((handle.tab_index, handle.tab_stop), (order, stop), "{text}");
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
        assert!(!after.tab_stop);
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
