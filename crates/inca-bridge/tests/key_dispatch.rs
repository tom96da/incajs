// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for `"keydown"`/`"keyup"`: routing from GPUI's own key
//! dispatch, through a real `FocusRegistry`, to the JS callbacks registered
//! for it; see `test_support::TestRoot`.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyWindowHandle, DispatchEventResult, KeyBinding, KeyDownEvent, KeyUpEvent, Keystroke,
    PlatformInput, TestAppContext, prelude::*,
};
use inca_bridge::{EventListeners, Host};
use inca_gpui::NodeId;
use inca_jsenv::Engine;
use inca_test_macros::gpui_cases;

use inca_bridge::test_support;

/// A focusable parent wrapping a focusable child.
fn build_key_pair() -> (Rc<RefCell<Host>>, NodeId, NodeId) {
    let (host, parent, children) = test_support::focusable_tree(1, true);
    (host, parent, children[0])
}

/// Registers `js`, the body of a listener taking `e`, for `"keydown"` and
/// `"keyup"` on each of `nodes`.
fn listen_for_keys(engine: &Engine, nodes: [NodeId; 2], js: &str, passive: bool) {
    let callback = format!("(e) => {{ {js} }}");
    test_support::listen_each(engine, &nodes, &["keydown", "keyup"], &callback, passive);
}

fn a_keystroke() -> Keystroke {
    Keystroke {
        modifiers: gpui::Modifiers::default(),
        key: "a".to_string(),
        key_char: Some("a".to_string()),
    }
}

/// With nothing focused, a key goes to the host root, which is the event's
/// `target`, and bubbles from there.
#[gpui::test]
fn a_key_event_with_nothing_focused_targets_the_root(cx: &mut TestAppContext) {
    let (host, parent, child) = build_key_pair();
    let root = host.borrow().root;
    host.borrow_mut().tree.append_child(root, parent).unwrap();
    let engine = test_support::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    listen_for_keys(
        &engine,
        [parent, child],
        "globalThis.seen.push(`${e.type}:${e.currentTarget}:${e.key}`);",
        false,
    );
    for event in ["keydown", "keyup"] {
        engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({root}, '{event}', (e) => {{
                    const {{ type, currentTarget, target, eventPhase, key }} = e;
                    globalThis.seen.push(
                        `${{type}}:${{currentTarget}}:${{target}}:${{eventPhase}}:${{key}}`);
                }});
                "
            ))
            .unwrap();
    }
    let window = test_support::mount_active(cx, &host, &engine, root);

    cx.simulate_keystrokes(window.into(), "a");
    cx.update_window(window.into(), |_root, window, cx| {
        window.dispatch_event(
            PlatformInput::KeyUp(KeyUpEvent {
                keystroke: a_keystroke(),
            }),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["keydown:{root}:{root}:2:a","keyup:{root}:{root}:2:a"]"#)
    );
}

gpui::actions!(key_dispatch_tests, [Bound]);

/// A window of the `build_key_pair` tree with its child focused. The parent
/// and the child run `js` for `keydown` and `keyup`.
type Pair = (AnyWindowHandle, Rc<Engine>, Rc<RefCell<Host>>, [NodeId; 2]);

fn focused_pair(cx: &mut TestAppContext, js: &str) -> Pair {
    let (host, parent, child) = build_key_pair();
    let engine = test_support::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    listen_for_keys(&engine, [parent, child], js, false);
    let window = test_support::mount_active(cx, &host, &engine, parent);
    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(child, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    (window.into(), engine, host, [parent, child])
}

/// What the platform is told about one key press or release.
fn platform_result(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    up: bool,
    key: &str,
) -> DispatchEventResult {
    let keystroke = Keystroke::parse(key).unwrap();
    let event = if up {
        PlatformInput::KeyUp(KeyUpEvent { keystroke })
    } else {
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        })
    };
    cx.update_window(window, |_, window, cx| window.dispatch_event(event, cx))
        .unwrap()
}

fn seen(engine: &Engine) -> String {
    engine
        .eval::<String>("JSON.stringify(globalThis.seen)")
        .unwrap()
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false, r#"["keydown","keydown"]"#)]
#[case::key_up(true, r#"["keyup","keyup"]"#)]
fn a_canceled_key_event_is_reported_to_the_platform_as_handled(
    cx: &mut TestAppContext,
    up: bool,
    expected_seen: &str,
) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type); e.preventDefault();");

    let result = platform_result(cx, window, up, "a");

    assert!(!result.propagate);
    assert!(result.default_prevented);
    assert_eq!(
        seen(&engine),
        expected_seen,
        "the listeners of both nodes ran before the key was reported handled"
    );
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false, r#"["keydown"]"#)]
#[case::key_up(true, r#"["keyup"]"#)]
fn a_stopped_key_event_is_reported_to_the_platform_as_unhandled(
    cx: &mut TestAppContext,
    up: bool,
    expected_seen: &str,
) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type); e.stopPropagation();");

    let result = platform_result(cx, window, up, "a");

    assert!(result.propagate && !result.default_prevented);
    assert_eq!(seen(&engine), expected_seen);
}

#[gpui::test]
fn a_key_event_nobody_listens_for_is_reported_to_the_platform_as_unhandled(
    cx: &mut TestAppContext,
) {
    let (window, engine, host, _) = focused_pair(cx, "seen.push(e.type); e.preventDefault();");
    host.borrow_mut().listeners = EventListeners::default();

    let result = platform_result(cx, window, false, "a");

    assert!(result.propagate && !result.default_prevented);
    assert_eq!(seen(&engine), "[]");
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false, r#"["keydown","keydown"]"#)]
#[case::key_up(true, r#"["keyup","keyup"]"#)]
fn a_passive_key_listener_leaves_the_platform_result_untouched(
    cx: &mut TestAppContext,
    up: bool,
    expected_seen: &str,
) {
    let js = "seen.push(e.type); e.preventDefault();";
    let (window, engine, host, nodes) = focused_pair(cx, js);
    host.borrow_mut().listeners = EventListeners::default();
    listen_for_keys(&engine, nodes, js, true);

    let result = platform_result(cx, window, up, "a");

    assert!(result.propagate && !result.default_prevented);
    assert_eq!(seen(&engine), expected_seen);
}

#[gpui::test]
fn a_bound_action_runs_and_no_listener_sees_its_key(cx: &mut TestAppContext) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type);");
    let acted = Rc::new(RefCell::new(0));
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("b", Bound, None)]);
        let acted = Rc::clone(&acted);
        cx.on_action(move |_: &Bound, _| *acted.borrow_mut() += 1);
    });

    let result = platform_result(cx, window, false, "b");

    assert_eq!(*acted.borrow(), 1);
    assert_eq!(seen(&engine), "[]");
    assert!(!result.propagate);
}

#[gpui::test]
fn a_key_event_bubbles_from_the_focused_node_to_its_ancestor(cx: &mut TestAppContext) {
    let js = "seen.push(`${e.type}:${e.currentTarget}:${e.key}`);";
    let (window, engine, _host, [parent, child]) = focused_pair(cx, js);

    cx.simulate_keystrokes(window, "a");
    platform_result(cx, window, true, "a");

    assert_eq!(
        seen(&engine),
        format!(
            r#"["keydown:{child}:a","keydown:{parent}:a","keyup:{child}:a","keyup:{parent}:a"]"#
        )
    );
}

#[gpui::test]
fn each_key_event_takes_its_own_event_id_shared_along_its_bubble(cx: &mut TestAppContext) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.eventId);");

    cx.simulate_keystrokes(window, "a a b");

    let ids: Vec<u64> = serde_json::from_str(&seen(&engine)).unwrap();
    assert_eq!(ids.len(), 6);
    for pair in ids.chunks(2) {
        assert_eq!(pair[0], pair[1]);
    }
    assert!(ids[0] >= 1 && ids[2] > ids[0] && ids[4] > ids[2], "{ids:?}");
}
