// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The path of an event and the order its listeners run in.

use inca_node::{NodeId, VirtualTree};

use super::{EventListeners, EventPayload, EventTargetId, Flags, Listener};

/// What one walk needs to know.
pub struct Walk<'a> {
    pub name: &'a str,
    pub flags: Flags,
    pub target: EventTargetId,
    /// The nodes whose listeners do not run. A cut silences window too.
    pub cut: &'a [NodeId],
    /// The payload of the event the walk dispatches. `deliver` does not read it.
    pub payload: &'a EventPayload,
    /// The value of `isTrusted` on the event the walk creates. `deliver` does not read it.
    pub trusted: bool,
}

/// How the walk reaches one event and its listeners. The walk sets the position, reads the
/// registrations, calls each listener through this seam and reads the stop flags the listeners
/// left.
pub trait Invoke {
    /// Whether `stopPropagation()` ran.
    fn stopped(&self) -> bool;
    /// Whether `stopImmediatePropagation()` ran.
    fn immediately_stopped(&self) -> bool;
    /// The registrations of `(target, name)` at the time of the call, in registration order.
    fn listeners(&self, target: EventTargetId, name: &str) -> Vec<Listener>;
    /// Whether `listener` is still registered for `(target, name)`. The full value is compared.
    fn is_registered(&self, target: EventTargetId, name: &str, listener: &Listener) -> bool;
    /// Sets `currentTarget` to the path entry `index` and `eventPhase` to `phase`.
    fn at(&self, index: usize, phase: u8);
    /// Calls `listener`. Listener errors are filed by the implementation.
    fn call(&self, listener: Listener);
    /// Leaves the event idle. Returns whether it ended canceled.
    fn end(&self) -> bool;
}

/// The targets an event fired at `target` travels through, nearest first: the
/// nodes from `target` up to the document, then window. A node outside the
/// document never reaches window. A signal is a path of its own.
#[must_use]
pub fn path_from(tree: &VirtualTree, target: EventTargetId) -> Vec<EventTargetId> {
    if let EventTargetId::Signal(_) = target {
        return vec![target];
    }
    let mut path = Vec::new();
    if let EventTargetId::Node(node) = target {
        let ancestors: Vec<_> = tree.ancestors(node).collect();
        let connected = ancestors.last().copied() == tree.document();
        path.extend(ancestors.into_iter().map(EventTargetId::Node));
        if !connected {
            return path;
        }
    }
    path.push(EventTargetId::Window);
    path
}

/// The targets whose bubble listeners the event reaches.
fn bubble_targets(flags: Flags, path: &[EventTargetId]) -> &[EventTargetId] {
    if flags.bubbles {
        path
    } else {
        &path[..path.len().min(1)]
    }
}

/// Whether `target` stays silent: a cut node, or window while any node is cut.
fn is_cut(target: EventTargetId, cut: &[NodeId]) -> bool {
    match target {
        EventTargetId::Node(node) => cut.contains(&node),
        EventTargetId::Window => !cut.is_empty(),
        EventTargetId::Signal(_) => false,
    }
}

/// Whether a listener of `name` exists that an event with `flags` fired at
/// `target` reaches. Nodes in `cut` stay silent.
#[must_use]
pub fn listened(
    tree: &VirtualTree,
    listeners: &EventListeners,
    name: &str,
    flags: Flags,
    target: EventTargetId,
    cut: &[NodeId],
) -> bool {
    path_from(tree, target)
        .into_iter()
        .enumerate()
        .any(|(depth, on_path)| {
            !is_cut(on_path, cut)
                && listeners
                    .listeners_for(on_path, name)
                    .iter()
                    .any(|l| l.capture || flags.bubbles || depth == 0)
        })
}

/// Takes the next number after `last` as an `eventId`.
pub fn next_event_id(last: &mut u64) -> u64 {
    *last += 1;
    *last
}

/// Calls the listeners along `path`, then leaves the event idle. Returns whether it ended
/// canceled. Capture listeners run from the end of `path` down to the target, then bubble
/// listeners from the target up, the latter only while the event bubbles. At the target, capture
/// listeners run before bubble listeners. A node's listeners are those registered when the walk
/// reaches it, minus any removed while an earlier listener ran.
pub fn deliver(walk: &Walk<'_>, path: &[EventTargetId], invoke: &impl Invoke) -> bool {
    let down = (0..path.len()).rev().map(|index| (true, index));
    let up = (0..bubble_targets(walk.flags, path).len()).map(|index| (false, index));
    for (capture, index) in down.chain(up) {
        if invoke.stopped() {
            break;
        }
        let target = path[index];
        if is_cut(target, walk.cut) {
            continue;
        }
        let mut listeners = invoke.listeners(target, walk.name);
        listeners.retain(|l| l.capture == capture);
        if listeners.is_empty() {
            continue;
        }
        let phase = match (index == 0, capture) {
            (true, _) => 2,
            (false, true) => 1,
            (false, false) => 3,
        };
        invoke.at(index, phase);
        for listener in listeners {
            if invoke.immediately_stopped() {
                break;
            }
            // The full value is compared. Flags change only through remove, then add.
            if !invoke.is_registered(target, walk.name, &listener) {
                continue;
            }
            invoke.call(listener);
        }
    }
    invoke.end()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    use rstest::rstest;

    use super::*;
    use crate::new_document_tree;

    type Action = Box<dyn Fn(&Fake)>;

    /// An `Invoke` that records each call as (callback id, path index, phase, passive) and
    /// runs the action registered for that callback id.
    struct Fake {
        listeners: RefCell<EventListeners>,
        path: [EventTargetId; 3],
        position: Cell<(usize, u8)>,
        log: RefCell<Vec<(u32, usize, u8, bool)>>,
        actions: HashMap<u32, Action>,
        stop: Cell<bool>,
        immediate: Cell<bool>,
        canceled: Cell<bool>,
    }

    impl Invoke for Fake {
        fn stopped(&self) -> bool {
            self.stop.get()
        }
        fn immediately_stopped(&self) -> bool {
            self.immediate.get()
        }
        fn listeners(&self, target: EventTargetId, name: &str) -> Vec<Listener> {
            self.listeners.borrow().listeners_for(target, name).to_vec()
        }
        fn is_registered(&self, target: EventTargetId, name: &str, listener: &Listener) -> bool {
            self.listeners
                .borrow()
                .listeners_for(target, name)
                .contains(listener)
        }
        fn at(&self, index: usize, phase: u8) {
            self.position.set((index, phase));
        }
        fn call(&self, listener: Listener) {
            let (index, phase) = self.position.get();
            self.log
                .borrow_mut()
                .push((listener.callback, index, phase, listener.passive));
            if let Some(action) = self.actions.get(&listener.callback) {
                action(self);
            }
        }
        fn end(&self) -> bool {
            self.canceled.get()
        }
    }

    fn stop(fake: &Fake) {
        fake.stop.set(true);
    }

    fn stop_immediately(fake: &Fake) {
        fake.stop.set(true);
        fake.immediate.set(true);
    }

    /// Registers `(node index, id, capture, passive)` listeners of `click` on the three nodes
    /// (0 child, 1 parent, 2 outer) and delivers one event from the child.
    fn run(
        listeners: &[(usize, u32, bool, bool)],
        flags: Flags,
        actions: Vec<(u32, Action)>,
    ) -> (Vec<(u32, usize, u8, bool)>, bool) {
        let path = [2, 1, 0].map(EventTargetId::Node);
        run_on(path, &[], listeners, flags, actions)
    }

    /// Like [`run`] over `path`, with the nodes in `cut` silenced.
    fn run_on(
        path: [EventTargetId; 3],
        cut: &[NodeId],
        listeners: &[(usize, u32, bool, bool)],
        flags: Flags,
        actions: Vec<(u32, Action)>,
    ) -> (Vec<(u32, usize, u8, bool)>, bool) {
        let mut registry = EventListeners::default();
        for &(index, id, capture, passive) in listeners {
            registry.insert(path[index], "click", id, capture, passive);
        }
        let fake = Fake {
            listeners: RefCell::new(registry),
            path,
            position: Cell::new((0, 0)),
            log: RefCell::default(),
            actions: actions.into_iter().collect(),
            stop: Cell::new(false),
            immediate: Cell::new(false),
            canceled: Cell::new(false),
        };
        let walk = Walk {
            name: "click",
            flags,
            target: path[0],
            cut,
            payload: &EventPayload::None,
            trusted: true,
        };
        let canceled = deliver(&walk, &path, &fake);
        (fake.log.take(), canceled)
    }

    fn act(f: impl Fn(&Fake) + 'static) -> Action {
        Box::new(f)
    }

    fn node(fake: &Fake, index: usize) -> EventTargetId {
        fake.path[index]
    }

    #[test]
    fn capture_runs_outermost_first_then_the_target_then_bubble_outwards() {
        let listeners = [
            (0, 1, false, false),
            (0, 2, true, false),
            (1, 3, false, false),
            (1, 4, true, false),
            (2, 5, false, false),
            (2, 6, true, false),
        ];

        let (log, _) = run(&listeners, Flags::ALL, vec![]);

        assert_eq!(
            log,
            [
                (6, 2, 1, false),
                (4, 1, 1, false),
                (2, 0, 2, false),
                (1, 0, 2, false),
                (3, 1, 3, false),
                (5, 2, 3, false),
            ]
        );
    }

    #[test]
    fn an_event_that_does_not_bubble_skips_the_bubble_listeners_of_ancestors() {
        let listeners = [
            (0, 1, false, false),
            (1, 2, false, false),
            (1, 3, true, false),
        ];

        let (log, _) = run(&listeners, Flags::NONE, vec![]);

        assert_eq!(log, [(3, 1, 1, false), (1, 0, 2, false)]);
    }

    #[test]
    fn a_cut_node_runs_no_listener_in_either_pass() {
        let path = [2, 1, 0].map(EventTargetId::Node);
        let listeners = [
            (0, 1, true, false),
            (0, 2, false, false),
            (1, 3, true, false),
            (1, 4, false, false),
            (2, 5, true, false),
            (2, 6, false, false),
        ];

        let (log, _) = run_on(path, &[1, 0], &listeners, Flags::ALL, vec![]);

        assert_eq!(log, [(1, 0, 2, false), (2, 0, 2, false)]);
    }

    #[test]
    fn a_cut_node_silences_window() {
        let path = [
            EventTargetId::Node(2),
            EventTargetId::Node(1),
            EventTargetId::Window,
        ];
        let listeners = [
            (0, 1, false, false),
            (2, 2, true, false),
            (2, 3, false, false),
        ];

        let (log, _) = run_on(path, &[1], &listeners, Flags::ALL, vec![]);

        assert_eq!(log, [(1, 0, 2, false)]);
    }

    #[test]
    fn window_listens_when_no_node_is_cut() {
        let path = [
            EventTargetId::Node(2),
            EventTargetId::Node(1),
            EventTargetId::Window,
        ];

        let (log, _) = run_on(path, &[], &[(2, 1, false, false)], Flags::ALL, vec![]);

        assert_eq!(log, [(1, 2, 3, false)]);
    }

    #[test]
    fn the_passive_flag_reaches_the_call() {
        let (log, _) = run(&[(0, 1, false, true)], Flags::ALL, vec![]);

        assert_eq!(log, [(1, 0, 2, true)]);
    }

    #[test]
    fn the_end_result_is_returned() {
        let (_, canceled) = run(
            &[(0, 1, false, false)],
            Flags::ALL,
            vec![(1, act(|f| f.canceled.set(true)))],
        );

        assert!(canceled);
    }

    #[test]
    fn a_passive_listener_can_still_stop_propagation() {
        let listeners = [(0, 1, false, true), (1, 2, false, false)];

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, act(stop))]);

        assert_eq!(log, [(1, 0, 2, true)]);
    }

    #[test]
    fn stop_propagation_keeps_the_nodes_other_listeners() {
        let listeners = [
            (0, 1, false, false),
            (0, 2, false, false),
            (1, 3, false, false),
        ];

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, act(stop))]);

        assert_eq!(log, [(1, 0, 2, false), (2, 0, 2, false)]);
    }

    #[test]
    fn stop_immediate_propagation_skips_the_nodes_other_listeners() {
        let listeners = [
            (0, 1, false, false),
            (0, 2, false, false),
            (1, 3, false, false),
        ];

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, act(stop_immediately))]);

        assert_eq!(log, [(1, 0, 2, false)]);
    }

    #[test]
    fn stop_propagation_in_a_capture_listener_ends_the_walk_after_that_node() {
        let listeners = [
            (2, 1, true, false),
            (1, 2, true, false),
            (1, 3, true, false),
            (0, 4, true, false),
            (0, 5, false, false),
            (1, 6, false, false),
        ];

        let (log, _) = run(&listeners, Flags::ALL, vec![(2, act(stop))]);

        assert_eq!(log, [(1, 2, 1, false), (2, 1, 1, false), (3, 1, 1, false)]);
    }

    #[test]
    fn stop_propagation_in_a_target_capture_listener_skips_the_target_bubble_listeners() {
        let listeners = [
            (0, 1, true, false),
            (0, 2, true, false),
            (0, 3, false, false),
            (1, 4, false, false),
        ];

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, act(stop))]);

        assert_eq!(log, [(1, 0, 2, false), (2, 0, 2, false)]);
    }

    #[test]
    fn stop_immediate_propagation_in_a_capture_listener_skips_the_rest_of_the_walk() {
        let listeners = [
            (2, 1, true, false),
            (2, 2, true, false),
            (0, 3, false, false),
        ];

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, act(stop_immediately))]);

        assert_eq!(log, [(1, 2, 1, false)]);
    }

    #[test]
    fn a_listener_added_mid_dispatch_runs_on_an_ancestor_and_waits_on_the_node() {
        let listeners = [(0, 1, false, false), (1, 2, false, false)];
        let add = act(|f| {
            let mut registry = f.listeners.borrow_mut();
            registry.insert(node(f, 0), "click", 3, false, false);
            registry.insert(node(f, 1), "click", 4, false, false);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, add)]);

        assert_eq!(log, [(1, 0, 2, false), (2, 1, 3, false), (4, 1, 3, false)]);
    }

    #[test]
    fn a_capture_listener_that_removes_a_later_listener_keeps_it_from_running() {
        let listeners = [
            (2, 1, true, false),
            (1, 2, true, false),
            (0, 3, false, false),
            (1, 4, false, false),
        ];
        let remove = act(|f| {
            let mut registry = f.listeners.borrow_mut();
            registry.remove(node(f, 1), "click", 2);
            registry.remove(node(f, 0), "click", 3);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, remove)]);

        assert_eq!(log, [(1, 2, 1, false), (4, 1, 3, false)]);
    }

    #[test]
    fn a_listener_re_added_with_other_flags_mid_walk_runs_once_in_its_new_pass() {
        let listeners = [(2, 1, true, false), (1, 2, false, false)];
        let readd = act(|f| {
            let mut registry = f.listeners.borrow_mut();
            registry.remove(node(f, 1), "click", 2);
            registry.insert(node(f, 1), "click", 2, true, false);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, readd)]);

        assert_eq!(log, [(1, 2, 1, false), (2, 1, 1, false)]);
    }

    #[test]
    fn a_listener_re_registered_with_other_flags_in_its_own_pass_skips_that_pass() {
        let listeners = [(0, 1, true, false), (0, 2, true, false)];
        let readd = act(|f| {
            let mut registry = f.listeners.borrow_mut();
            registry.remove(node(f, 0), "click", 2);
            registry.insert(node(f, 0), "click", 2, true, true);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, readd)]);

        assert_eq!(log, [(1, 0, 2, false)]);
    }

    #[test]
    fn a_listener_added_to_the_target_in_its_capture_pass_joins_only_the_bubble_pass() {
        let listeners = [(0, 1, true, false)];
        let add = act(|f| {
            let mut registry = f.listeners.borrow_mut();
            registry.insert(node(f, 0), "click", 2, false, false);
            registry.insert(node(f, 0), "click", 3, true, false);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, add)]);

        assert_eq!(log, [(1, 0, 2, false), (2, 0, 2, false)]);
    }

    #[rstest]
    #[case::bubbling(true)]
    #[case::not_bubbling(false)]
    fn the_path_of_a_signal_is_the_signal_alone(#[case] bubbles: bool) {
        let tree = new_document_tree();
        let flags = Flags {
            bubbles,
            cancelable: false,
            composed: false,
        };

        let path = path_from(&tree, EventTargetId::Signal(5));

        assert_eq!(path, [EventTargetId::Signal(5)]);
        assert_eq!(bubble_targets(flags, &path), [EventTargetId::Signal(5)]);
    }

    #[test]
    fn a_signal_is_never_cut() {
        assert!(!is_cut(EventTargetId::Signal(5), &[1, 2]));
    }
}
