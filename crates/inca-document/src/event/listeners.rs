// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The targets an event can reach, and the registrations `addEventListener` makes on them.

use std::collections::HashMap;

use inca_node::NodeId;

use super::Links;

/// Something an event can be dispatched at or travel through: a node of the
/// tree, the window, which is no node, or an `AbortSignal`, which
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

/// One registration: the callback id, the phase it listens in and whether it
/// is passive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listener {
    /// The id of the function in the callback table.
    pub callback: u32,
    /// Listens while the event travels down to the target.
    pub capture: bool,
    /// `preventDefault()` calls from the callback are ignored.
    pub passive: bool,
}

/// Every `(target id, event name)` a caller has registered through
/// `addEventListener`, mapped to its [`Listener`]s. Holds owned plain data only, so it can be
/// read at any time. Targets include element nodes, the global document and the global window.
///
/// Nothing expires on its own: a registration lives until the document removes it, or releases
/// it when its node or signal goes away.
#[derive(Debug, Default)]
pub struct EventListeners {
    by_target: HashMap<EventTargetId, HashMap<String, Vec<Listener>>>,
    /// The registrations a signal removes when it aborts.
    pub(crate) links: Links,
}

impl EventListeners {
    /// Adds `callback_id` to `(target, event)` with the given phase and
    /// passivity.
    pub(crate) fn insert(
        &mut self,
        target: EventTargetId,
        event: impl Into<String>,
        callback_id: u32,
        capture: bool,
        passive: bool,
    ) {
        let listeners = self
            .by_target
            .entry(target)
            .or_default()
            .entry(event.into())
            .or_default();
        listeners.push(Listener {
            callback: callback_id,
            capture,
            passive,
        });
    }

    /// Drops `callback_id` from `(target, event)`, and reports whether it
    /// was registered.
    pub(crate) fn remove(&mut self, target: EventTargetId, event: &str, callback_id: u32) -> bool {
        let Some(events) = self.by_target.get_mut(&target) else {
            return false;
        };
        let Some(listeners) = events.get_mut(event) else {
            return false;
        };
        let before = listeners.len();
        listeners.retain(|l| l.callback != callback_id);
        let removed = listeners.len() != before;
        if listeners.is_empty() {
            events.remove(event);
        }
        if events.is_empty() {
            self.by_target.remove(&target);
        }
        if removed {
            self.links.unlink(callback_id);
        }
        removed
    }

    /// Drops every registration on `node_id` and returns the callback ids
    /// that were there, in no particular order.
    pub(crate) fn release(&mut self, node_id: NodeId) -> Vec<u32> {
        self.release_target(EventTargetId::Node(node_id))
    }

    /// Drops every registration on each of `nodes` and returns the callback ids that were there.
    pub(crate) fn release_nodes(&mut self, nodes: &[NodeId]) -> Vec<u32> {
        nodes.iter().flat_map(|&id| self.release(id)).collect()
    }

    /// Drops every registration on `target` and returns the callback ids that were there, in no
    /// particular order.
    pub(crate) fn release_target(&mut self, target: EventTargetId) -> Vec<u32> {
        let ids: Vec<u32> = self
            .by_target
            .remove(&target)
            .map(|events| events.into_values().flatten().map(|l| l.callback).collect())
            .unwrap_or_default();
        for &id in &ids {
            self.links.unlink(id);
        }
        ids
    }

    /// The links from each signal to the registrations its abort removes.
    #[must_use]
    pub fn links(&self) -> &Links {
        &self.links
    }

    /// Every registration as `(target, event, callback id)`.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn registrations(&self) -> Vec<(EventTargetId, String, u32)> {
        self.by_target
            .iter()
            .flat_map(|(&target, events)| {
                events.iter().flat_map(move |(event, listeners)| {
                    listeners
                        .iter()
                        .map(move |l| (target, event.clone(), l.callback))
                })
            })
            .collect()
    }

    /// Whether `target` has a registration for `event`.
    #[must_use]
    pub fn has_listener(&self, target: EventTargetId, event: &str) -> bool {
        !self.listeners_for(target, event).is_empty()
    }

    /// Whether nothing is registered anywhere.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_target.is_empty()
    }

    /// The registrations of `(target, event)` in registration order.
    #[must_use]
    pub fn listeners_for(&self, target: EventTargetId, event: &str) -> &[Listener] {
        self.by_target
            .get(&target)
            .and_then(|events| events.get(event))
            .map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn callback_ids(listeners: &EventListeners, target: EventTargetId, event: &str) -> Vec<u32> {
        listeners
            .listeners_for(target, event)
            .iter()
            .map(|l| l.callback)
            .collect()
    }

    #[test]
    fn a_registration_keeps_its_phase_and_passivity() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "wheel", 7, true, true);
        listeners.insert(EventTargetId::Node(1), "wheel", 8, false, false);

        assert_eq!(
            listeners.listeners_for(EventTargetId::Node(1), "wheel"),
            &[
                Listener {
                    callback: 7,
                    capture: true,
                    passive: true
                },
                Listener {
                    callback: 8,
                    capture: false,
                    passive: false
                },
            ]
        );
    }

    #[test]
    fn remove_reports_whether_it_removed_anything() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "click", 7, false, false);

        assert!(listeners.remove(EventTargetId::Node(1), "click", 7));
        assert!(callback_ids(&listeners, EventTargetId::Node(1), "click").is_empty());
        assert!(!listeners.remove(EventTargetId::Node(1), "click", 7));
        assert!(!listeners.remove(EventTargetId::Node(99), "click", 7));
    }

    #[test]
    fn release_returns_every_callback_on_a_node() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "click", 7, false, false);
        listeners.insert(EventTargetId::Node(1), "focus", 8, false, false);
        listeners.insert(EventTargetId::Node(2), "click", 9, false, false);

        let mut released = listeners.release(1);
        released.sort_unstable();

        assert_eq!(released, vec![7, 8]);
        assert!(callback_ids(&listeners, EventTargetId::Node(1), "click").is_empty());
        assert_eq!(
            callback_ids(&listeners, EventTargetId::Node(2), "click"),
            &[9],
            "another node's registrations must survive"
        );
    }

    #[test]
    fn releasing_a_list_of_nodes_gives_the_callback_ids_of_each() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "click", 7, false, false);
        listeners.insert(EventTargetId::Node(2), "click", 8, false, false);
        listeners.insert(EventTargetId::Node(3), "click", 9, false, false);

        let mut released = listeners.release_nodes(&[1, 3]);
        released.sort_unstable();

        assert_eq!(released, [7, 9]);
        assert_eq!(
            callback_ids(&listeners, EventTargetId::Node(2), "click"),
            &[8]
        );
    }

    #[test]
    fn releasing_a_node_leaves_the_signals_alone() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Node(1), "click", 7, false, false);
        listeners.insert(EventTargetId::Signal(2), "abort", 8, false, false);

        assert_eq!(listeners.release(1), [7]);
        assert_eq!(
            callback_ids(&listeners, EventTargetId::Signal(2), "abort"),
            [8]
        );
    }

    #[test]
    fn releasing_a_target_gives_its_callback_ids() {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Signal(2), "abort", 8, false, false);

        assert_eq!(listeners.release_target(EventTargetId::Signal(2)), [8]);
        assert!(listeners.is_empty());
    }

    #[rstest]
    #[case::same(EventTargetId::Signal(2), "abort", true)]
    #[case::other_event(EventTargetId::Signal(2), "click", false)]
    #[case::other_target(EventTargetId::Signal(3), "abort", false)]
    fn a_listener_is_found_by_target_and_event(
        #[case] target: EventTargetId,
        #[case] event: &str,
        #[case] found: bool,
    ) {
        let mut listeners = EventListeners::default();
        listeners.insert(EventTargetId::Signal(2), "abort", 8, false, false);

        assert_eq!(listeners.has_listener(target, event), found);
    }
}
