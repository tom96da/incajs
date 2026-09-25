// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for `"keydown"`/`"keyup"` (`crate::element`'s
//! `wire_stateless`): routing from GPUI's own key dispatch, through a real
//! `FocusRegistry`, to the JS callbacks registered for it — see
//! `KeyableRoot::render` below.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, KeyUpEvent, Keystroke, PlatformInput, Render, TestAppContext, Window, prelude::*,
};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{NodeId, render_tree_with_events};
use inca_jsenv::Engine;

struct KeyableRoot {
    host: Rc<RefCell<Host>>,
    node: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for KeyableRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transitions = self.host.borrow_mut().focus.apply_pending(window, cx);
        for transition in &transitions {
            transition.dispatch(&self.dispatcher, window, cx);
        }
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher).unwrap()
    }
}

/// A focusable parent wrapping a focusable child, both listening for
/// `"keydown"`/`"keyup"` via the same callback id.
fn build_key_pair() -> (Rc<RefCell<Host>>, NodeId, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let parent = host_mut.tree.create_node("div");
    let child = host_mut.tree.create_node("div");
    host_mut.tree.append_child(parent, child).unwrap();
    for node in [parent, child] {
        host_mut.listeners.register(node, "keydown", 0);
        host_mut.listeners.register(node, "keyup", 0);
    }
    drop(host_mut);
    (host, parent, child)
}

fn install_key_recorder(engine: &Engine) {
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { globalThis.seen.push(`${event.type}:${event.currentTarget}:${event.key}`); } \
             };",
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

#[gpui::test]
fn a_key_down_bubbles_from_the_focused_node_to_its_ancestor(cx: &mut TestAppContext) {
    let (host, parent, child) = build_key_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_key_recorder(&engine);
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
        host.borrow_mut().focus.request_focus(child);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.simulate_keystrokes(window.into(), "a");

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["keydown:{child}:a","keydown:{parent}:a"]"#),
        "keydown must bubble from the focused node up through its ancestors"
    );
}

/// With nothing focused, a key reaches no node in the tree — GPUI routes it
/// to the window's own root instead, which carries no `inca` listeners.
#[gpui::test]
fn a_key_down_with_nothing_focused_reaches_no_node(cx: &mut TestAppContext) {
    let (host, parent, _child) = build_key_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_key_recorder(&engine);
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

    cx.simulate_keystrokes(window.into(), "a");

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]"
    );
}

#[gpui::test]
fn a_key_up_also_bubbles_from_the_focused_node(cx: &mut TestAppContext) {
    let (host, parent, child) = build_key_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_key_recorder(&engine);
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
        host.borrow_mut().focus.request_focus(child);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

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
        format!(r#"["keyup:{child}:a","keyup:{parent}:a"]"#)
    );
}
