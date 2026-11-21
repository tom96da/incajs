// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Calls the listeners of one target and leaves the event idle.

use std::cell::RefCell;
use std::rc::Rc;

use inca_jsenv::callbacks;
use rquickjs::{Class, Ctx, Value};

use super::{IncaEvent, Walk, bubble_targets, is_cut, note};
use crate::EventTargetId;
use crate::bindings::Host;
use inca_document::Listener;

/// How the walk reaches one event and its listeners. The walk sets the position, calls
/// each listener through this seam and reads the stop flags the listeners left.
pub(super) trait Invoke {
    /// Whether `stopPropagation()` ran.
    fn stopped(&self) -> bool;
    /// Whether `stopImmediatePropagation()` ran.
    fn immediately_stopped(&self) -> bool;
    /// Sets `currentTarget` to the path entry `index` and `eventPhase` to `phase`.
    fn at(&self, index: usize, phase: u8);
    /// Calls `listener`. Listener errors are filed by the implementation.
    fn call(&self, listener: Listener);
    /// Leaves the event idle. Returns whether it ended canceled.
    fn end(&self) -> bool;
}

/// Calls a listener of a JS event object in the engine context of the walk.
pub(super) struct JsInvoke<'a, 'js> {
    pub(super) ctx: &'a Ctx<'js>,
    pub(super) host: &'a Rc<RefCell<Host>>,
    pub(super) event: &'a Class<'js, IncaEvent<'js>>,
}

impl Invoke for JsInvoke<'_, '_> {
    fn stopped(&self) -> bool {
        self.event.borrow().state.stop.get()
    }

    fn immediately_stopped(&self) -> bool {
        self.event.borrow().state.immediate.get()
    }

    fn at(&self, index: usize, phase: u8) {
        let record = self.event.borrow();
        record.state.current.set(Some(index));
        record.state.phase.set(phase);
    }

    fn call(&self, listener: Listener) {
        let called = callbacks::get(self.ctx, listener.callback).and_then(|stored| {
            let Some(callback) = stored.and_then(Value::into_function) else {
                return Ok(());
            };
            self.event.borrow().state.passive.set(listener.passive);
            callback.call::<_, ()>((self.event.clone(),))
        });
        note(self.ctx, self.host, called);
    }

    fn end(&self) -> bool {
        let record = self.event.borrow();
        let canceled = record.state.canceled.get();
        end_dispatch(&record);
        canceled
    }
}

/// Calls the listeners along `path`, then leaves the event idle. Returns whether it ended
/// canceled.
pub(super) fn deliver(
    host: &Rc<RefCell<Host>>,
    walk: &Walk<'_>,
    path: &[EventTargetId],
    invoke: &impl Invoke,
) -> bool {
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
        let listeners: Vec<Listener> = host
            .borrow()
            .listeners
            .listeners_for(target, walk.name)
            .iter()
            .filter(|l| l.capture == capture)
            .copied()
            .collect();
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
            if !host
                .borrow()
                .listeners
                .listeners_for(target, walk.name)
                .contains(&listener)
            {
                continue;
            }
            invoke.call(listener);
        }
    }
    invoke.end()
}

/// Leaves `event` as a dispatch leaves it: no walk state, no stop flags,
/// no path.
pub(super) fn end_dispatch(event: &IncaEvent<'_>) {
    let state = &event.state;
    for flag in [
        &state.dispatching,
        &state.passive,
        &state.stop,
        &state.immediate,
    ] {
        flag.set(false);
    }
    state.current.set(None);
    state.phase.set(0);
    // A kept event keeps only target, relatedTarget and detail alive.
    drop(event.held.path.take());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    use super::{Invoke, deliver};
    use crate::EventTargetId;
    use crate::bindings::Host;
    use crate::event::Flags;
    use crate::event::walk::Walk;
    use crate::tests::{dispatcher_with_engine, listen, listen_with, tree_of_three};
    use gpui::TestAppContext;
    use inca_document::{EventPayload, Listener};

    type Action = Box<dyn Fn(&Fake)>;

    /// An `Invoke` that records each call as (callback id, path index, phase, passive) and
    /// runs the action registered for that callback id.
    struct Fake {
        host: Rc<RefCell<Host>>,
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
        let host = Rc::new(RefCell::new(Host::default()));
        let [outer, parent, child] = tree_of_three(&host);
        let path = [child, parent, outer].map(EventTargetId::Node);
        for &(index, id, capture, passive) in listeners {
            host.borrow_mut()
                .listeners
                .insert(path[index], "click", id, capture, passive);
        }
        let fake = Fake {
            host: Rc::clone(&host),
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
            cut: &[],
            payload: &EventPayload::None,
            trusted: true,
        };
        let canceled = deliver(&host, &walk, &path, &fake);
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
    fn stop_propagation_keeps_the_nodes_other_listeners_and_skips_the_ancestors() {
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
    fn a_listener_added_mid_walk_runs_on_an_ancestor_and_waits_on_the_node() {
        let listeners = [(0, 1, false, false), (1, 2, false, false)];
        let add = act(|f| {
            let mut host = f.host.borrow_mut();
            host.listeners.insert(node(f, 0), "click", 3, false, false);
            host.listeners.insert(node(f, 1), "click", 4, false, false);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, add)]);

        assert_eq!(log, [(1, 0, 2, false), (2, 1, 3, false), (4, 1, 3, false)]);
    }

    #[test]
    fn a_listener_removed_mid_walk_does_not_run() {
        let listeners = [
            (2, 1, true, false),
            (1, 2, true, false),
            (0, 3, false, false),
            (1, 4, false, false),
        ];
        let remove = act(|f| {
            let mut host = f.host.borrow_mut();
            host.listeners.remove(node(f, 1), "click", 2);
            host.listeners.remove(node(f, 0), "click", 3);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, remove)]);

        assert_eq!(log, [(1, 2, 1, false), (4, 1, 3, false)]);
    }

    #[test]
    fn a_listener_re_added_with_other_flags_mid_walk_runs_once_in_its_new_pass() {
        let listeners = [(2, 1, true, false), (1, 2, false, false)];
        let readd = act(|f| {
            let mut host = f.host.borrow_mut();
            host.listeners.remove(node(f, 1), "click", 2);
            host.listeners.insert(node(f, 1), "click", 2, true, false);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, readd)]);

        assert_eq!(log, [(1, 2, 1, false), (2, 1, 1, false)]);
    }

    #[test]
    fn a_listener_re_registered_with_other_flags_in_its_own_pass_skips_that_pass() {
        let listeners = [(0, 1, true, false), (0, 2, true, false)];
        let readd = act(|f| {
            let mut host = f.host.borrow_mut();
            host.listeners.remove(node(f, 0), "click", 2);
            host.listeners.insert(node(f, 0), "click", 2, true, true);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, readd)]);

        assert_eq!(log, [(1, 0, 2, false)]);
    }

    #[test]
    fn a_listener_added_to_the_target_in_its_capture_pass_joins_only_the_bubble_pass() {
        let listeners = [(0, 1, true, false)];
        let add = act(|f| {
            let mut host = f.host.borrow_mut();
            host.listeners.insert(node(f, 0), "click", 2, false, false);
            host.listeners.insert(node(f, 0), "click", 3, true, false);
        });

        let (log, _) = run(&listeners, Flags::ALL, vec![(1, add)]);

        assert_eq!(log, [(1, 0, 2, false), (2, 0, 2, false)]);
    }

    #[gpui::test]
    fn a_passive_listener_cannot_cancel_and_an_active_one_after_it_can(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        let passive = (false, true);
        listen_with(
            &dispatcher,
            parent,
            "click",
            (true, true),
            "(e) => { e.preventDefault(); out.push(e.defaultPrevented); }",
        );
        listen_with(
            &dispatcher,
            child,
            "click",
            passive,
            "(e) => { e.returnValue = false; out.push(e.defaultPrevented); }",
        );
        listen_with(
            &dispatcher,
            child,
            "click",
            passive,
            r"
            (e) => { e.stopPropagation(); e.preventDefault(); out.push(e.defaultPrevented); }
            ",
        );
        listen(
            &dispatcher,
            child,
            "click",
            r"
            (e) => {
                out.push(e.defaultPrevented); e.preventDefault();
                out.push(e.defaultPrevented);
            }
            ",
        );
        let cx = cx.add_empty_window();

        let canceled = cx
            .update(|window, cx| dispatcher.fire(child, "click", &EventPayload::None, window, cx));

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(out, "[false,false,false,false,true]");
        assert!(canceled);
    }

    #[gpui::test]
    fn an_event_only_passive_listeners_heard_stays_uncanceled(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        listen_with(
            &dispatcher,
            child,
            "click",
            (false, true),
            "(e) => { e.preventDefault(); }",
        );
        let cx = cx.add_empty_window();

        let canceled = cx
            .update(|window, cx| dispatcher.fire(child, "click", &EventPayload::None, window, cx));

        assert!(!canceled);
    }
}
