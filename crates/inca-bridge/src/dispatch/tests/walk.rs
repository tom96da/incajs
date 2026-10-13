// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn one_event_object_reaches_every_node(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    for node in [child, parent, outer] {
        listen(
            &dispatcher,
            node,
            "click",
            r"
            (e) => { globalThis.first ??= e; e.seen = (e.seen ?? 0) + 1;
            out.push([e === first, e.seen]); }
            ",
        );
    }

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([[true, 1], [true, 2], [true, 3]]));
}

#[gpui::test]
fn after_dispatch_current_target_is_null_phase_0_and_stop_flags_clear(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    listen(
        &dispatcher,
        child,
        "click",
        r"
        (e) => {
            globalThis.kept = e; e.stopPropagation(); e.preventDefault();
            out.push([e.currentTarget, e.eventPhase, e.cancelBubble]);
        }
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
        listen(
            &dispatcher,
            node,
            "click",
            &format!(
                r"
                (e) => {{
                    out.push([e.currentTarget, e.composedPath()]);
                    if (e.currentTarget === {child}) __inca_native__.removeChild({parent}, {child});
                }}
                "
            ),
        );
    }

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
    // Callback ids follow the registration order: 0, 1 and 2.
    listen(
        &dispatcher,
        child,
        "click",
        &format!(
            r"
            () => {{
                out.push(0);
                __inca_native__.removeEventListener({child}, 'click', 1);
                __inca_native__.removeEventListener({parent}, 'click', 2);
            }}
            "
        ),
    );
    listen(&dispatcher, child, "click", "() => out.push(1)");
    listen(&dispatcher, parent, "click", "() => out.push(2)");

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([0]));
}

#[gpui::test]
fn a_listener_added_to_an_ancestor_mid_dispatch_runs_and_one_added_to_the_node_waits(
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
fn each_dispatch_takes_the_next_event_id_and_an_unheard_one_takes_none(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    let record = "(e) => out.push([e.type, e.eventId])";
    listen(&dispatcher, parent, "click", record);
    listen(&dispatcher, parent, "focusin", record);
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

    dispatcher
        .engine
        .eval::<()>("out.length = 0; globalThis.immediate = true;")
        .unwrap();
    let immediate = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
    assert_eq!(immediate, serde_json::json!([0]));
}

/// A listener that pushes `[name, currentTarget, eventPhase]`.
fn tagged(name: &str) -> String {
    format!("(e) => out.push(['{name}', e.currentTarget, e.eventPhase])")
}

#[gpui::test]
fn capture_listeners_run_root_down_before_the_bubble_listeners_run_target_up(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    // Bubble listeners register first, so the walk order differs from the registration order.
    for (name, node, capture) in [
        ("child", child, false),
        ("parent", parent, false),
        ("outer", outer, false),
        ("child-capture", child, true),
        ("parent-capture", parent, true),
        ("outer-capture", outer, true),
    ] {
        listen_with(&dispatcher, node, "click", (capture, false), &tagged(name));
    }

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
    for (name, capture) in [("b0", false), ("c1", true), ("b2", false), ("c3", true)] {
        listen_with(&dispatcher, child, "click", (capture, false), &tagged(name));
    }

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
    for (name, node, capture) in [
        ("outer", outer, true),
        ("outer-bubble", outer, false),
        ("parent", parent, true),
        ("parent-bubble", parent, false),
        ("child", child, true),
        ("child-bubble", child, false),
    ] {
        listen_with(&dispatcher, node, "focus", (capture, false), &tagged(name));
    }

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
    listen_with(
        &dispatcher,
        child,
        "click",
        (false, true),
        "(e) => { e.preventDefault(); }",
    );
    let cx = cx.add_empty_window();

    let canceled =
        cx.update(|window, cx| dispatcher.fire(child, "click", &EventPayload::None, window, cx));

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
fn a_capture_listener_that_removes_a_later_listener_keeps_it_from_running(cx: &mut TestAppContext) {
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

#[gpui::test]
fn the_cut_nodes_are_skipped_in_both_passes(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    for (name, node, capture) in [
        ("outer-capture", outer, true),
        ("parent-capture", parent, true),
        ("child-capture", child, true),
        ("child", child, false),
        ("parent", parent, false),
        ("outer", outer, false),
    ] {
        listen_with(&dispatcher, node, "click", (capture, false), &tagged(name));
    }
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
