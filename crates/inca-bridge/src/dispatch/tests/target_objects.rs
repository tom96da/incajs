// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use rstest::rstest;

/// Gives every node one object `{ node }` and counts the calls.
const HOOK: &str = r"
    globalThis.calls = 0;
    globalThis.objs = {};
    globalThis.nodeOf = (o) => (o === null ? null : o.node);
    globalThis.__inca_targets__ = (ids) => {
        globalThis.calls += 1;
        return ids.map((id) => (globalThis.objs[id] ??= { node: id }));
    };
    ";

const THROWS: &str = "globalThis.__inca_targets__ = () => { throw new Error('no targets'); };";

#[gpui::test]
fn event_fields_hold_the_objects_the_hook_returns(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    let note = r"(e) => {
        out.push([nodeOf(e.target), nodeOf(e.currentTarget), e.srcElement === e.target,
            e.composedPath().map(nodeOf)]);
        globalThis.kept = e;
    }";
    listen_with(&dispatcher, outer, "click", (true, false), note);
    listen(&dispatcher, parent, "click", note);

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    let path = serde_json::json!([child, parent, outer]);
    assert_eq!(
        out,
        serde_json::json!([[child, outer, true, path], [child, parent, true, path]])
    );
    let after = eval_json(
        &dispatcher,
        &format!(
            "[kept.target === objs[{child}], kept.currentTarget, kept.composedPath().length, calls]"
        ),
    );
    assert_eq!(after, serde_json::json!([true, null, 0, 1]));
}

#[gpui::test]
fn the_hook_runs_once_per_heard_walk_and_never_for_an_unheard_one(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen(&dispatcher, outer, "click", "() => {}");
    listen(&dispatcher, parent, "click", "() => {}");
    let cx = cx.add_empty_window();

    cx.update(|window, cx| {
        dispatcher.fire(child, "click", &EventPayload::None, window, cx);
        dispatcher.fire(child, "keydown", &EventPayload::None, window, cx);
    });

    assert_eq!(eval_json(&dispatcher, "calls"), serde_json::json!(1));
}

#[gpui::test]
fn related_target_is_the_object_of_the_related_node(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    for event in ["mouseover", "focus"] {
        listen(
            &dispatcher,
            outer,
            event,
            "(e) => out.push([e.type, nodeOf(e.relatedTarget)])",
        );
    }
    let over = EventPayload::Mouse(MousePayload {
        related_target: Some(parent),
        ..MousePayload::at(at(0.0, 0.0), gpui::Modifiers::none())
    });
    let cx = cx.add_empty_window();

    cx.update(|window, cx| {
        dispatcher.fire(outer, "mouseover", &over, window, cx);
        for related_target in [Some(child), None] {
            let payload = EventPayload::Focus { related_target };
            dispatcher.fire(outer, "focus", &payload, window, cx);
        }
    });

    assert_eq!(
        read_out(&dispatcher),
        serde_json::json!([["mouseover", parent], ["focus", child], ["focus", null]])
    );
}

#[test]
fn dispatch_event_gives_an_unheard_event_its_target_object() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{ const e = new Event('ping');
            const r = __inca_native__.dispatchEvent({node}, e);
            return [r, e.target === objs[{node}], calls]; }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!([true, true, 1]));
}

#[gpui::test]
fn a_throwing_hook_runs_no_listener(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher.engine.eval::<()>(THROWS).unwrap();
    listen(&dispatcher, node, "click", "() => out.push('ran')");

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([]));
}

#[gpui::test]
fn a_throwing_hook_is_reported(cx: &mut TestAppContext) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher.engine.eval::<()>(THROWS).unwrap();
    listen(&dispatcher, node, "click", "() => out.push('ran')");

    let _ = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    assert_eq!(reported.borrow().len(), 1);
    assert_eq!(reported.borrow()[0].message(), "Error: no targets");
}

#[gpui::test]
fn a_hook_that_returns_no_array_runs_no_listener(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>("globalThis.__inca_targets__ = () => 5;")
        .unwrap();
    listen(&dispatcher, node, "click", "() => out.push('ran')");

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([]));
}

#[gpui::test]
fn a_hook_that_returns_no_array_is_reported(cx: &mut TestAppContext) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>("globalThis.__inca_targets__ = () => 5;")
        .unwrap();
    listen(&dispatcher, node, "click", "() => out.push('ran')");

    let _ = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    assert_eq!(reported.borrow().len(), 1);
}

#[test]
fn dispatch_event_throws_what_the_hook_throws() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher.engine.eval::<()>(THROWS).unwrap();
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{ try {{ __inca_native__.dispatchEvent({node}, new Event('ping'));
            return 'returned'; }} catch (e) {{ return e.message; }} }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!("no targets"));
}

#[test]
fn dispatch_event_throws_when_the_hook_returns_no_array() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>("globalThis.__inca_targets__ = () => 5;")
        .unwrap();
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{ try {{ __inca_native__.dispatchEvent({node}, new Event('ping'));
            return 'returned'; }} catch {{ return 'threw'; }} }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!("threw"));
}

#[gpui::test]
fn a_target_a_listener_destroys_stays_in_the_path(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen_with(
        &dispatcher,
        outer,
        "click",
        (true, false),
        &format!("() => __inca_native__.destroyNode({child})"),
    );
    listen(
        &dispatcher,
        parent,
        "click",
        "(e) => out.push([nodeOf(e.target), e.composedPath().map(nodeOf), calls])",
    );

    let out = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([[child, [child, parent, outer], 1]]));
}

#[gpui::test]
fn a_kept_event_lets_its_path_objects_go_after_the_walk(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    dispatcher
        .engine
        .eval::<()>("globalThis.__inca_targets__ = (ids) => ids.map((id) => ({ node: id }));")
        .unwrap();
    listen(
        &dispatcher,
        child,
        "click",
        "(e) => { globalThis.kept = e; globalThis.weak = new WeakRef(e.composedPath()[1]); }",
    );
    fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    dispatcher.engine.with(|ctx| ctx.run_gc());

    let gone = dispatcher
        .engine
        .eval::<bool>(&format!(
            "kept.target.node === {child} && weak.deref() === undefined"
        ))
        .unwrap();
    assert!(gone);
}

#[test]
fn dispatch_event_gives_listeners_the_hook_objects() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen(
        &dispatcher,
        parent,
        "ping",
        "(e) => out.push([nodeOf(e.target), nodeOf(e.currentTarget), e.composedPath().map(nodeOf), calls])",
    );
    dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_native__.dispatchEvent({child}, new Event('ping', {{ bubbles: true }}));"
        ))
        .unwrap();

    assert_eq!(
        read_out(&dispatcher),
        serde_json::json!([[child, parent, [child, parent, outer], 1]])
    );
}

#[gpui::test]
fn a_wheel_payload_gives_its_related_node_to_the_hook(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, _] = tree_of_three(&host);
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen(
        &dispatcher,
        outer,
        "wheel",
        "(e) => out.push(nodeOf(e.relatedTarget))",
    );
    let wheel = EventPayload::Wheel(WheelPayload {
        mouse: MousePayload {
            related_target: Some(parent),
            ..MousePayload::at(at(0.0, 0.0), gpui::Modifiers::none())
        },
        delta_x: 0.0,
        delta_y: 0.0,
        delta_z: 0.0,
        delta_mode: 0,
    });

    let out = fire_and_read(cx, &dispatcher, outer, "wheel", &wheel);

    assert_eq!(out, serde_json::json!([parent]));
}

/// Records `[label, target, currentTarget, eventPhase, path]` of the event it hears.
fn note_walk(label: &str) -> String {
    format!(
        r"
        (e) => out.push([
            '{label}',
            nodeOf(e.target),
            nodeOf(e.currentTarget),
            e.eventPhase,
            e.composedPath().map(nodeOf),
        ])
        "
    )
}

#[gpui::test]
fn native_event_path_runs_through_the_root_then_document_then_window(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = node_under_root(&host);
    let root = host.borrow().root();
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen(&dispatcher, node, "click", &note_walk("node"));
    listen(&dispatcher, root, "click", &note_walk("root"));
    listen_global(
        &dispatcher,
        "$document",
        "click",
        false,
        &note_walk("document"),
    );
    listen_global(&dispatcher, "$window", "click", false, &note_walk("window"));

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    let path = serde_json::json!([node, root, "$document", "$window"]);
    assert_eq!(
        out,
        serde_json::json!([
            ["node", node, node, 2, path],
            ["root", node, root, 3, path],
            ["document", node, "$document", 3, path],
            ["window", node, "$window", 3, path],
        ])
    );
}

#[gpui::test]
fn global_targets_capture_window_first_and_bubble_window_last(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = node_under_root(&host);
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    for (tag, capture) in [
        ("$window", true),
        ("$document", true),
        ("$document", false),
        ("$window", false),
    ] {
        let label = format!("{tag} {}", if capture { "capture" } else { "bubble" });
        listen_global(&dispatcher, tag, "click", capture, &note_walk(&label));
    }
    listen_with(
        &dispatcher,
        node,
        "click",
        (true, false),
        &note_walk("node capture"),
    );
    listen(&dispatcher, node, "click", &note_walk("node bubble"));

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    let order: Vec<_> = out
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row[0].as_str().unwrap(), row[2].clone(), row[3].clone()))
        .collect();
    let (doc, win) = ("$document", "$window");
    assert_eq!(
        order,
        [
            (
                "$window capture",
                serde_json::json!(win),
                serde_json::json!(1)
            ),
            (
                "$document capture",
                serde_json::json!(doc),
                serde_json::json!(1)
            ),
            (
                "node capture",
                serde_json::json!(node),
                serde_json::json!(2)
            ),
            ("node bubble", serde_json::json!(node), serde_json::json!(2)),
            (
                "$document bubble",
                serde_json::json!(doc),
                serde_json::json!(3)
            ),
            (
                "$window bubble",
                serde_json::json!(win),
                serde_json::json!(3)
            ),
        ]
    );
}

#[gpui::test]
fn a_key_event_with_nothing_focused_starts_at_the_host_root(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let root = host.borrow().root();
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen(&dispatcher, root, "keydown", &note_walk("root"));
    listen_global(
        &dispatcher,
        "$document",
        "keydown",
        false,
        &note_walk("document"),
    );
    listen_global(
        &dispatcher,
        "$window",
        "keydown",
        false,
        &note_walk("window"),
    );
    let key = gpui::Keystroke::parse("a").unwrap();
    let cx = cx.add_empty_window();

    cx.update(|window, cx| {
        let sink: &dyn EventSink = &dispatcher;
        sink.key_input(&key, false, &EventPayload::None, window, cx);
    });

    let path = serde_json::json!([root, "$document", "$window"]);
    assert_eq!(
        read_out(&dispatcher),
        serde_json::json!([
            ["root", root, root, 2, path],
            ["document", root, "$document", 3, path],
            ["window", root, "$window", 3, path],
        ])
    );
}

#[gpui::test]
fn an_event_at_a_node_outside_the_root_never_reaches_document_or_window(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen(&dispatcher, node, "click", &note_walk("node"));
    listen_global(
        &dispatcher,
        "$document",
        "click",
        false,
        &note_walk("document"),
    );
    listen_global(&dispatcher, "$window", "click", true, &note_walk("window"));

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([["node", node, node, 2, [node]]]));
}

#[rstest]
#[case::add("__inca_native__.addEventListener('$nope', 'click', false, false)")]
#[case::remove("__inca_native__.removeEventListener('$nope', 'click', 0)")]
#[case::dispatch("__inca_native__.dispatchEvent('$nope', new Event('click'))")]
fn unknown_global_target_tag_throws_a_type_error(#[case] call: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                try {{
                    {call};
                    return 'returned';
                }} catch (e) {{
                    return e instanceof TypeError;
                }}
            }})()
            "
        ),
    );
    assert_eq!(got, serde_json::json!(true));
}
