// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Calls the listeners of one target and leaves the event idle.

use std::cell::RefCell;
use std::rc::Rc;

use rquickjs::{Class, Ctx, Function, Object};

use super::{IncaEvent, Walk, bubble_targets, is_cut, note};
use crate::EventTargetId;
use crate::bindings::{Host, Listener};

/// Calls the listeners along `path` with `event`, then leaves the event
/// idle. Returns whether it ended canceled.
pub(super) fn deliver<'js>(
    ctx: &Ctx<'js>,
    host: &Rc<RefCell<Host>>,
    walk: &Walk<'_>,
    path: &[EventTargetId],
    callbacks: &Object<'js>,
    event: &Class<'js, IncaEvent<'js>>,
) -> bool {
    let record = event.borrow();
    let state = &record.state;
    let down = (0..path.len()).rev().map(|index| (true, index));
    let up = (0..bubble_targets(walk.flags, path).len()).map(|index| (false, index));
    for (capture, index) in down.chain(up) {
        if state.stop.get() {
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
        state.current.set(Some(index));
        state.phase.set(phase);
        for listener in listeners {
            if state.immediate.get() {
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
            let Ok(callback) = callbacks.get::<_, Function>(listener.callback) else {
                continue;
            };
            state.passive.set(listener.passive);
            let called = callback.call::<_, ()>((event.clone(),));
            note(ctx, host, called);
        }
    }
    let canceled = state.canceled.get();
    end_dispatch(&record);
    canceled
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
    use crate::tests::{
        dispatcher_with_engine, fire_and_read, listen, listen_with, tagged, tree_of_three,
    };
    use gpui::TestAppContext;
    use inca_gpui::EventPayload;

    #[gpui::test]
    fn a_listener_added_mid_dispatch_runs_on_an_ancestor_and_waits_on_the_node(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            child,
            "click",
            &format!(
                r"
                () => {{
                    out.push(0);
                    __inca_test_listen__({child}, 'click', () => out.push(2));
                    __inca_test_listen__({parent}, 'click', () => out.push(3));
                }}
                "
            ),
        );
        listen(&dispatcher, parent, "click", "() => out.push(1)");

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([0, 1, 3]));
    }

    #[gpui::test]
    fn stop_propagation_keeps_the_nodes_other_listeners(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            child,
            "click",
            r"
            (e) => {
                out.push(0);
                globalThis.immediate ? e.stopImmediatePropagation() : e.stopPropagation();
            }
            ",
        );
        listen(&dispatcher, child, "click", "() => out.push(1)");
        listen(&dispatcher, parent, "click", "() => out.push(2)");
        let stopped = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
        assert_eq!(stopped, serde_json::json!([0, 1]));
    }

    #[gpui::test]
    fn stop_immediate_propagation_skips_the_nodes_other_listeners(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            child,
            "click",
            r"
            (e) => {
                out.push(0);
                globalThis.immediate ? e.stopImmediatePropagation() : e.stopPropagation();
            }
            ",
        );
        listen(&dispatcher, child, "click", "() => out.push(1)");
        listen(&dispatcher, parent, "click", "() => out.push(2)");
        dispatcher
            .engine
            .eval::<()>("globalThis.immediate = true;")
            .unwrap();
        let immediate = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
        assert_eq!(immediate, serde_json::json!([0]));
    }

    #[gpui::test]
    fn stop_propagation_in_a_capture_listener_ends_the_walk_after_that_node(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        let stopper =
            "(e) => { out.push(['stopper', e.currentTarget, e.eventPhase]); e.stopPropagation(); }";
        let listeners = [
            (outer, true, tagged("outer")),
            (parent, true, stopper.to_owned()),
            (parent, true, tagged("parent-second")),
            (child, true, "() => out.push('child')".to_owned()),
            (child, false, "() => out.push('child-bubble')".to_owned()),
            (parent, false, "() => out.push('parent-bubble')".to_owned()),
        ];
        for (node, capture, f) in listeners {
            listen_with(&dispatcher, node, "click", (capture, false), &f);
        }

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(
            out,
            serde_json::json!([
                ["outer", outer, 1],
                ["stopper", parent, 1],
                ["parent-second", parent, 1],
            ])
        );
    }

    #[gpui::test]
    fn stop_propagation_in_a_target_capture_listener_skips_the_target_bubble_listeners(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        let stop = "(e) => { out.push(0); e.stopPropagation(); }";
        listen_with(&dispatcher, child, "click", (true, false), stop);
        listen_with(
            &dispatcher,
            child,
            "click",
            (true, false),
            "() => out.push(1)",
        );
        listen(&dispatcher, child, "click", "() => out.push(2)");
        listen(&dispatcher, parent, "click", "() => out.push(3)");

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([0, 1]));
    }

    #[gpui::test]
    fn stop_immediate_propagation_in_a_capture_listener_skips_the_rest_of_the_walk(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, _, child] = tree_of_three(&host);
        let stop = "(e) => { out.push(0); e.stopImmediatePropagation(); }";
        listen_with(&dispatcher, outer, "click", (true, false), stop);
        listen_with(
            &dispatcher,
            outer,
            "click",
            (true, false),
            "() => out.push(1)",
        );
        listen(&dispatcher, child, "click", "() => out.push(2)");

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([0]));
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

    #[gpui::test]
    fn a_passive_listener_can_still_stop_propagation(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen_with(
            &dispatcher,
            child,
            "click",
            (false, true),
            "(e) => { out.push(0); e.stopPropagation(); }",
        );
        listen(&dispatcher, parent, "click", "() => out.push(1)");

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([0]));
    }

    #[gpui::test]
    fn a_capture_listener_that_removes_a_later_listener_keeps_it_from_running(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        // Callback ids follow the registration order: 0 to 3, then 4 for the one added mid-walk.
        listen_with(
            &dispatcher,
            outer,
            "click",
            (true, false),
            &format!(
                r"
                () => {{
                    out.push(0);
                   __inca_native__.removeEventListener({parent}, 'click', 1);
                   __inca_native__.removeEventListener({child}, 'click', 2);
                   __inca_test_listen__({parent}, 'click', () => out.push(4), true);
                }}
                "
            ),
        );
        listen_with(
            &dispatcher,
            parent,
            "click",
            (true, false),
            "() => out.push(1)",
        );
        listen(&dispatcher, child, "click", "() => out.push(2)");
        listen(&dispatcher, parent, "click", "() => out.push(3)");

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([0, 4, 3]));
    }

    #[gpui::test]
    fn a_listener_re_added_with_other_flags_mid_walk_runs_once_in_its_new_pass(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        // Callback ids follow the registration order: 0 and 1.
        listen_with(
            &dispatcher,
            outer,
            "click",
            (true, false),
            &format!(
                r"
                () => {{
                    out.push(0);
                    __inca_native__.removeEventListener({parent}, 'click', 1);
                    __inca_test_listen__({parent}, 'click', again, true);
                }}
                "
            ),
        );
        dispatcher
            .engine
            .eval::<()>("globalThis.again = (e) => out.push([1, e.eventPhase]);")
            .unwrap();
        listen(&dispatcher, parent, "click", "again");

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([0, [1, 1]]));
    }

    #[gpui::test]
    fn a_listener_re_registered_with_other_flags_in_its_own_pass_skips_that_pass(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        // Callback ids follow the registration order: 0 and 1.
        listen_with(
            &dispatcher,
            child,
            "click",
            (true, false),
            &format!(
                r"
                () => {{
                    out.push(0);
                    __inca_native__.removeEventListener({child}, 'click', 1);
                    __inca_test_listen__({child}, 'click', again, true, true);
                }}
                "
            ),
        );
        dispatcher
            .engine
            .eval::<()>("globalThis.again = (e) => { out.push(1); e.preventDefault(); };")
            .unwrap();
        listen_with(&dispatcher, child, "click", (true, false), "again");
        let cx = cx.add_empty_window();

        let canceled = cx
            .update(|window, cx| dispatcher.fire(child, "click", &EventPayload::None, window, cx));

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(out, "[0]");
        assert!(!canceled);
    }

    #[gpui::test]
    fn a_listener_added_to_the_target_in_its_capture_pass_joins_only_the_bubble_pass(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        listen_with(
            &dispatcher,
            child,
            "click",
            (true, false),
            &format!(
                r"
                () => {{
                    out.push(0);
                   __inca_test_listen__({child}, 'click', () => out.push(1));
                   __inca_test_listen__({child}, 'click', () => out.push(2), true);
                }}
                "
            ),
        );

        let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([0, 1]));
    }
}
