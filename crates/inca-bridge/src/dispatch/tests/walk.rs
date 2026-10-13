// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn one_event_object_reaches_every_node(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    for node in [child, parent, outer] {
        listen(&host, node, "click");
    }
    callbacks(
        &dispatcher,
        r"
        0: (e) => { globalThis.first ??= e; e.seen = (e.seen ?? 0) + 1;
        out.push([e === first, e.seen]); }
        ",
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([[true, 1], [true, 2], [true, 3]]));
}

#[gpui::test]
fn after_dispatch_current_target_is_null_phase_0_and_stop_flags_clear(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    listen(&host, child, "click");
    callbacks(
        &dispatcher,
        r"
        0: (e) => { globalThis.kept = e; e.stopPropagation(); e.preventDefault();
        out.push([e.currentTarget, e.eventPhase, e.cancelBubble]); }
        ",
    );
    let during = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
    assert_eq!(during, serde_json::json!([[child, 2, true]]));

    let after = dispatcher
        .engine
        .eval::<String>(
            r"
            JSON.stringify([kept.currentTarget, kept.eventPhase, kept.cancelBubble,
            kept.defaultPrevented, kept.composedPath().length])
            ",
        )
        .unwrap();
    assert_eq!(after, "[null,0,false,true,0]");
}

#[gpui::test]
fn the_path_is_fixed_when_dispatch_starts(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    for node in [child, parent, outer] {
        listen(&host, node, "click");
    }
    callbacks(
        &dispatcher,
        &format!(
            r"
            0: (e) => {{ out.push([e.currentTarget, e.composedPath()]);
            if (e.currentTarget === {child})
                __inca_native__.removeChild({parent}, {child}); }}
            "
        ),
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    let path = serde_json::json!([child, parent, outer]);
    assert_eq!(
        out,
        serde_json::json!([[child, path], [parent, path], [outer, path]])
    );
}

#[gpui::test]
fn a_listener_removed_mid_dispatch_does_not_run(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    for (node, id) in [(child, 0), (child, 1), (parent, 2)] {
        host.borrow_mut().listeners.register(node, "click", id);
    }
    callbacks(
        &dispatcher,
        &format!(
            r"
            0: () => {{ out.push(0);
               __inca_native__.removeEventListener({child}, 'click', 1);
               __inca_native__.removeEventListener({parent}, 'click', 2); }},
            1: () => out.push(1),
            2: () => out.push(2)
            "
        ),
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0]));
}

#[gpui::test]
fn a_listener_added_to_an_ancestor_mid_dispatch_runs_and_one_added_to_the_node_waits(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    add_in(&host, child, false, false);
    add_in(&host, parent, false, false);
    callbacks(
        &dispatcher,
        &format!(
            r"
            0: () => {{ out.push(0);
               __inca_native__.addEventListener({child}, 'click');
               __inca_native__.addEventListener({parent}, 'click'); }},
            1: () => out.push(1), 2: () => out.push(2), 3: () => out.push(3)
            "
        ),
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0, 1, 3]));
}

#[gpui::test]
fn each_dispatch_takes_the_next_event_id_and_an_unheard_one_takes_none(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen(&host, parent, "click");
    listen(&host, parent, "focusin");
    callbacks(&dispatcher, "0: (e) => out.push([e.type, e.eventId])");
    let cx = cx.add_empty_window();

    cx.update(|window, cx| {
        let none = EventPayload::None;
        dispatcher.fire(child, "click", &none, window, cx);
        dispatcher.fire(child, "keydown", &none, window, cx);
        dispatcher.fire(child, "click", &none, window, cx);
        dispatcher.fire(child, "focusin", &none, window, cx);
    });

    let out = dispatcher
        .engine
        .eval::<String>("JSON.stringify(out)")
        .unwrap();
    assert_eq!(out, r#"[["click",1],["click",2],["focusin",3]]"#);
}

#[gpui::test]
fn stop_propagation_keeps_the_nodes_other_listeners_and_stop_immediate_does_not(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    for (node, id) in [(child, 0), (child, 1), (parent, 2)] {
        host.borrow_mut().listeners.register(node, "click", id);
    }
    callbacks(
        &dispatcher,
        r"
        0: (e) => { out.push(0); globalThis.immediate ?
               e.stopImmediatePropagation() : e.stopPropagation(); },
        1: () => out.push(1), 2: () => out.push(2)
        ",
    );
    let stopped = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
    assert_eq!(stopped, serde_json::json!([0, 1]));

    dispatcher
        .engine
        .eval::<()>("out.length = 0; globalThis.immediate = true;")
        .unwrap();
    let immediate = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
    assert_eq!(immediate, serde_json::json!([0]));
}

/// Registers a `click` listener under the next host-allocated callback id.
fn add_in(host: &Rc<RefCell<Host>>, node: NodeId, capture: bool, passive: bool) -> u32 {
    host.borrow_mut()
        .listeners
        .add(node, "click", capture, passive)
        .unwrap()
}

fn listen_in(host: &Rc<RefCell<Host>>, node: NodeId, id: u32, capture: bool, passive: bool) {
    host.borrow_mut()
        .listeners
        .register_with(node, "click", id, capture, passive);
}

/// Callbacks `0..names.len()` that push `[name, currentTarget, eventPhase]`.
fn tagged(names: &[&str]) -> String {
    names
        .iter()
        .enumerate()
        .map(|(id, name)| {
            format!("{id}: (e) => out.push(['{name}', e.currentTarget, e.eventPhase])")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[gpui::test]
fn capture_listeners_run_root_down_before_the_bubble_listeners_run_target_up(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    // Bubble listeners register first, so the walk order differs from the registration order.
    for (id, node, capture) in [
        (0, child, false),
        (1, parent, false),
        (2, outer, false),
        (3, child, true),
        (4, parent, true),
        (5, outer, true),
    ] {
        listen_in(&host, node, id, capture, false);
    }
    callbacks(
        &dispatcher,
        &tagged(&[
            "child",
            "parent",
            "outer",
            "child-capture",
            "parent-capture",
            "outer-capture",
        ]),
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(
        out,
        serde_json::json!([
            ["outer-capture", outer, 1],
            ["parent-capture", parent, 1],
            ["child-capture", child, 2],
            ["child", child, 2],
            ["parent", parent, 3],
            ["outer", outer, 3],
        ])
    );
}

#[gpui::test]
fn capture_listeners_of_one_node_run_apart_from_its_bubble_listeners(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    for (id, capture) in [(0, false), (1, true), (2, false), (3, true)] {
        listen_in(&host, child, id, capture, false);
    }
    callbacks(&dispatcher, &tagged(&["b0", "c1", "b2", "c3"]));

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    let names: Vec<_> = out
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[0].as_str().unwrap())
        .collect();
    assert_eq!(names, ["c1", "c3", "b0", "b2"]);
}

#[gpui::test]
fn an_event_that_does_not_bubble_still_captures_through_its_ancestors(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    for (id, node, capture) in [
        (0, outer, true),
        (1, outer, false),
        (2, parent, true),
        (3, parent, false),
        (4, child, true),
        (5, child, false),
    ] {
        host.borrow_mut()
            .listeners
            .register_with(node, "focus", id, capture, false);
    }
    callbacks(
        &dispatcher,
        &tagged(&[
            "outer",
            "outer-bubble",
            "parent",
            "parent-bubble",
            "child",
            "child-bubble",
        ]),
    );

    let out = fire_and_read(cx, &dispatcher, child, "focus", &EventPayload::None);

    assert_eq!(
        out,
        serde_json::json!([
            ["outer", outer, 1],
            ["parent", parent, 1],
            ["child", child, 2],
            ["child-bubble", child, 2],
        ])
    );
}

#[gpui::test]
fn stop_propagation_in_a_capture_listener_ends_the_walk_after_that_node(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    for (id, node, capture) in [
        (0, outer, true),
        (1, parent, true),
        (2, parent, true),
        (3, child, true),
        (4, child, false),
        (5, parent, false),
    ] {
        listen_in(&host, node, id, capture, false);
    }
    callbacks(
        &dispatcher,
        r"
        0: (e) => out.push(['outer', e.currentTarget, e.eventPhase]),
        1: (e) => { out.push(['stopper', e.currentTarget, e.eventPhase]); e.stopPropagation(); },
        2: (e) => out.push(['parent-second', e.currentTarget, e.eventPhase]),
        3: () => out.push('child'), 4: () => out.push('child-bubble'),
        5: () => out.push('parent-bubble')
        ",
    );

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
    listen_in(&host, child, 0, true, false);
    listen_in(&host, child, 1, true, false);
    listen_in(&host, child, 2, false, false);
    listen_in(&host, parent, 3, false, false);
    callbacks(
        &dispatcher,
        r"
        0: (e) => { out.push(0); e.stopPropagation(); }, 1: () => out.push(1),
        2: () => out.push(2), 3: () => out.push(3)
        ",
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0, 1]));
}

#[gpui::test]
fn stop_immediate_propagation_in_a_capture_listener_skips_the_rest_of_the_walk(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, _, child] = tree_of_three(&host);
    listen_in(&host, outer, 0, true, false);
    listen_in(&host, outer, 1, true, false);
    listen_in(&host, child, 2, false, false);
    callbacks(
        &dispatcher,
        r"
        0: (e) => { out.push(0); e.stopImmediatePropagation(); }, 1: () => out.push(1),
        2: () => out.push(2)
        ",
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0]));
}

#[gpui::test]
fn a_passive_listener_cannot_cancel_and_an_active_one_after_it_can(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen_in(&host, parent, 0, true, true);
    listen_in(&host, child, 1, false, true);
    listen_in(&host, child, 2, false, true);
    listen_in(&host, child, 3, false, false);
    callbacks(
        &dispatcher,
        r"
        0: (e) => { e.preventDefault(); out.push(e.defaultPrevented); },
        1: (e) => { e.returnValue = false; out.push(e.defaultPrevented); },
        2: (e) => { e.stopPropagation(); e.preventDefault(); out.push(e.defaultPrevented); },
        3: (e) => { out.push(e.defaultPrevented); e.preventDefault();
           out.push(e.defaultPrevented); }
        ",
    );
    let cx = cx.add_empty_window();

    let canceled =
        cx.update(|window, cx| dispatcher.fire(child, "click", &EventPayload::None, window, cx));

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
    listen_in(&host, child, 0, false, true);
    callbacks(&dispatcher, "0: (e) => { e.preventDefault(); }");
    let cx = cx.add_empty_window();

    let canceled =
        cx.update(|window, cx| dispatcher.fire(child, "click", &EventPayload::None, window, cx));

    assert!(!canceled);
}

#[gpui::test]
fn a_passive_listener_can_still_stop_propagation(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen_in(&host, child, 0, false, true);
    listen_in(&host, parent, 1, false, false);
    callbacks(
        &dispatcher,
        "0: (e) => { out.push(0); e.stopPropagation(); }, 1: () => out.push(1)",
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0]));
}

#[gpui::test]
fn a_capture_listener_that_removes_a_later_listener_keeps_it_from_running(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    add_in(&host, outer, true, false);
    add_in(&host, parent, true, false);
    add_in(&host, child, false, false);
    add_in(&host, parent, false, false);
    callbacks(
        &dispatcher,
        &format!(
            r"
            0: () => {{ out.push(0);
               __inca_native__.removeEventListener({parent}, 'click', 1);
               __inca_native__.removeEventListener({child}, 'click', 2);
               __inca_native__.addEventListener({parent}, 'click', true); }},
            1: () => out.push(1), 2: () => out.push(2), 3: () => out.push(3),
            4: () => out.push(4)
            "
        ),
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0, 4, 3]));
}

#[gpui::test]
fn a_listener_re_added_with_other_flags_mid_walk_runs_once_in_its_new_pass(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    add_in(&host, outer, true, false);
    add_in(&host, parent, false, false);
    callbacks(
        &dispatcher,
        &format!(
            r"
            0: () => {{ out.push(0);
               __inca_native__.removeEventListener({parent}, 'click', 1);
               const id = __inca_native__.addEventListener({parent}, 'click', true);
               __inca_callbacks__[id] = __inca_callbacks__[1]; }},
            1: (e) => out.push([1, e.eventPhase])
            "
        ),
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0, [1, 1]]));
}

#[gpui::test]
fn a_listener_re_registered_with_other_flags_in_its_own_pass_skips_that_pass(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    add_in(&host, child, true, false);
    add_in(&host, child, true, false);
    callbacks(
        &dispatcher,
        &format!(
            r"
            0: () => {{ out.push(0);
               __inca_native__.removeEventListener({child}, 'click', 1);
               const id = __inca_native__.addEventListener({child}, 'click', true, true);
               __inca_callbacks__[id] = __inca_callbacks__[1]; }},
            1: (e) => {{ out.push(1); e.preventDefault(); }}
            "
        ),
    );
    let cx = cx.add_empty_window();

    let canceled =
        cx.update(|window, cx| dispatcher.fire(child, "click", &EventPayload::None, window, cx));

    let out = dispatcher
        .engine
        .eval::<String>("JSON.stringify(out)")
        .unwrap();
    assert_eq!(out, "[0]");
    assert!(!canceled);
}

#[gpui::test]
fn a_listener_added_to_the_target_during_its_capture_pass_joins_the_bubble_pass_only(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    add_in(&host, child, true, false);
    callbacks(
        &dispatcher,
        &format!(
            r"
            0: () => {{ out.push(0);
               __inca_native__.addEventListener({child}, 'click');
               __inca_native__.addEventListener({child}, 'click', true); }},
            1: () => out.push(1), 2: () => out.push(2)
            "
        ),
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0, 1]));
}

#[gpui::test]
fn the_cut_nodes_are_skipped_in_both_passes(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    for (id, node, capture) in [
        (0, outer, true),
        (1, parent, true),
        (2, child, true),
        (3, child, false),
        (4, parent, false),
        (5, outer, false),
    ] {
        listen_in(&host, node, id, capture, false);
    }
    callbacks(
        &dispatcher,
        &tagged(&[
            "outer-capture",
            "parent-capture",
            "child-capture",
            "child",
            "parent",
            "outer",
        ]),
    );
    let cx = cx.add_empty_window();
    let path = dispatcher.path_from(child);

    cx.update(|_, _| {
        event::walk(
            &dispatcher.engine,
            &dispatcher.host,
            &Walk {
                name: "click",
                flags: Flags::ALL,
                target: child,
                path: &path,
                cut: &[parent, outer],
                payload: &EventPayload::None,
                trusted: true,
            },
        );
    });

    let out = dispatcher
        .engine
        .eval::<String>("JSON.stringify(out)")
        .unwrap();
    assert_eq!(
        out,
        serde_json::json!([["child-capture", child, 2], ["child", child, 2]]).to_string()
    );
}
