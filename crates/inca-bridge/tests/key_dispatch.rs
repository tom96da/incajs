// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for `"keydown"`/`"keyup"`: routing from GPUI's own key
//! dispatch, through a real `FocusRegistry`, to the JS callbacks registered
//! for it; see `KeyableRoot::render` below.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyWindowHandle, Context, DispatchEventResult, KeyBinding, KeyDownEvent, KeyUpEvent, Keystroke,
    PlatformInput, Render, TestAppContext, Window, prelude::*,
};
use inca_bridge::{EventDispatcher, EventListeners, Host};
use inca_gpui::{NodeId, render_tree_with_events};
use inca_jsenv::Engine;

struct KeyableRoot {
    host: Rc<RefCell<Host>>,
    node: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for KeyableRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dispatcher.begin_frame(window, cx);
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher).unwrap()
    }
}

/// A focusable parent wrapping a focusable child, both listening for
/// `"keydown"`/`"keyup"` via the same callback id.
fn build_key_pair() -> (Rc<RefCell<Host>>, NodeId, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let parent = host_mut.tree.create_node("div").unwrap();
    let child = host_mut.tree.create_node("div").unwrap();
    host_mut.tree.append_child(parent, child).unwrap();
    for node in [parent, child] {
        host_mut.listeners.register(node, "keydown", 0);
        host_mut.listeners.register(node, "keyup", 0);
        host_mut.tree.set_attribute(node, "tabindex", "0").unwrap();
        host_mut.focus.mark_tab_dirty(node);
    }
    drop(host_mut);
    (host, parent, child)
}

fn install_key_recorder(engine: &Engine) {
    engine
        .eval::<()>(
            r"
            globalThis.seen = [];
            globalThis.__inca_callbacks__ = {
               0: (event) => { globalThis.seen.push(`${event.type}:${event.currentTarget}:${event.key}`); }
            };
            ",
        )
        .unwrap();
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
    let (host, parent, _child) = build_key_pair();
    let root = host.borrow().root;
    host.borrow_mut().tree.append_child(root, parent).unwrap();
    for event in ["keydown", "keyup"] {
        host.borrow_mut().listeners.register(root, event, 1);
    }
    let engine = Rc::new(Engine::new().unwrap());
    install_key_recorder(&engine);
    engine
        .eval::<()>(
            r"
            globalThis.__inca_callbacks__[1] = (e) => { globalThis.seen.push(
            `${e.type}:${e.currentTarget}:${e.target}:${e.eventPhase}:${e.key}`); };
            ",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| KeyableRoot {
        host: Rc::clone(&host),
        node: root,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

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

/// A window of the `build_key_pair` tree with its child focused. Callback 0
/// is `js`; the parent and the child listen for `keydown` and `keyup`.
type Pair = (AnyWindowHandle, Rc<Engine>, Rc<RefCell<Host>>, [NodeId; 2]);

fn focused_pair(cx: &mut TestAppContext, js: &str) -> Pair {
    let (host, parent, child) = build_key_pair();
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(&format!(
            "globalThis.seen = []; globalThis.__inca_callbacks__ = {{ 0: (e) => {{ {js} }} }};"
        ))
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| KeyableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();
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

#[gpui::test]
fn a_canceled_key_event_is_reported_to_the_platform_as_handled(cx: &mut TestAppContext) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type); e.preventDefault();");

    for up in [false, true] {
        let result = platform_result(cx, window, up, "a");
        assert!(!result.propagate, "up: {up}");
        assert!(result.default_prevented, "up: {up}");
    }
    assert_eq!(
        seen(&engine),
        r#"["keydown","keydown","keyup","keyup"]"#,
        "the listeners of both nodes ran before the key was reported handled"
    );
}

#[gpui::test]
fn a_stopped_key_event_is_reported_to_the_platform_as_unhandled(cx: &mut TestAppContext) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type); e.stopPropagation();");

    for up in [false, true] {
        let result = platform_result(cx, window, up, "a");
        assert!(result.propagate && !result.default_prevented, "up: {up}");
    }
    assert_eq!(seen(&engine), r#"["keydown","keyup"]"#);
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

#[gpui::test]
fn a_passive_key_listener_leaves_the_platform_result_untouched(cx: &mut TestAppContext) {
    let (window, engine, host, [parent, child]) =
        focused_pair(cx, "seen.push(e.type); e.preventDefault();");
    host.borrow_mut().listeners = EventListeners::default();
    for node in [parent, child] {
        for event in ["keydown", "keyup"] {
            host.borrow_mut()
                .listeners
                .register_with(node, event, 0, false, true);
        }
    }

    for up in [false, true] {
        let result = platform_result(cx, window, up, "a");
        assert!(result.propagate && !result.default_prevented, "up: {up}");
    }
    assert_eq!(seen(&engine), r#"["keydown","keydown","keyup","keyup"]"#);
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
