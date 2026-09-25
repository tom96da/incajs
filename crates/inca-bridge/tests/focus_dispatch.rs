// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for `inca_bridge::focus`: `focusNode`/`blurNode`
//! (`crate::bindings`) through to the `"focus"`/`"blur"` dispatched into
//! JS, via a real `FocusRegistry::apply_pending` running once per frame —
//! see `FocusableRoot::render` below.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Context, Render, TestAppContext, Window, prelude::*};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{NodeId, render_tree_with_events};
use inca_jsenv::Engine;

struct FocusableRoot {
    host: Rc<RefCell<Host>>,
    node: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for FocusableRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let transition = self.host.borrow_mut().focus.apply_pending(window, cx);
        transition.dispatch(&self.dispatcher, window, cx);
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher).unwrap()
    }
}

/// Two sibling nodes, both listening for `"focus"`/`"blur"` via the same
/// callback id, wrapped in a common parent — the root `FocusableRoot`
/// renders.
fn build_focusable_pair() -> (Rc<RefCell<Host>>, NodeId, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let parent = host_mut.tree.create_node("div");
    let a = host_mut.tree.create_node("div");
    let b = host_mut.tree.create_node("div");
    host_mut.tree.append_child(parent, a).unwrap();
    host_mut.tree.append_child(parent, b).unwrap();
    for node in [a, b] {
        host_mut.listeners.register(node, "focus", 0);
        host_mut.listeners.register(node, "blur", 0);
    }
    drop(host_mut);
    (host, parent, a)
}

fn install_focus_recorder(engine: &Engine) {
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { globalThis.seen.push(`${event.type}:${event.target}`); } \
             };",
        )
        .unwrap();
}

#[gpui::test]
fn focus_node_dispatches_focus_next_frame(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]",
        "mounting must never touch the JS engine"
    );

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["focus:{a}"]"#)
    );
}

#[gpui::test]
fn moving_focus_to_another_node_blurs_the_first(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let b = {
        let host = host.borrow();
        host.tree.get(parent).unwrap().children()[1]
    };
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(b);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["blur:{a}","focus:{b}"]"#)
    );
}

#[gpui::test]
fn blur_node_blurs_whatever_is_focused(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_blur();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["blur:{a}"]"#)
    );
}

/// `blurNode` when nothing is focused must not fire a spurious `"blur"`.
#[gpui::test]
fn blur_node_with_nothing_focused_reports_nothing(cx: &mut TestAppContext) {
    let (host, parent, _a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_blur();
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]"
    );
}

/// Destroying a focused node reports no `"blur"` — its listeners are
/// already gone by the time `destroyNode` returns, the same as any other
/// event kind's registrations on a destroyed node.
#[gpui::test]
fn destroying_a_focused_node_reports_no_blur(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().tree.destroy_node(a);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]"
    );
}

/// `focusNode` holds one pending request, not a queue — two calls before
/// the next frame leave only the later one to apply.
#[gpui::test]
fn a_second_focus_node_call_before_the_next_frame_wins(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let b = {
        let host = host.borrow();
        host.tree.get(parent).unwrap().children()[1]
    };
    let engine = Rc::new(Engine::new().unwrap());
    install_focus_recorder(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();

    cx.update_window(window.into(), |_root, window, cx| {
        host.borrow_mut().focus.request_focus(a);
        host.borrow_mut().focus.request_focus(b);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["focus:{b}"]"#),
        "only the second request should have taken effect"
    );
}
