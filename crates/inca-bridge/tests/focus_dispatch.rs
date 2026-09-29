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
        let transitions = self.host.borrow_mut().focus.apply_pending(window, cx);
        for transition in &transitions {
            transition.dispatch(&self.dispatcher, window, cx);
        }
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
        host.borrow_mut().focus.request_blur(a);
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
        host.borrow_mut().focus.request_blur(a);
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

/// `focusNode` queues its requests — two calls before the next frame both
/// take effect, in order, the same as the DOM's synchronous `.focus()`.
#[gpui::test]
fn two_focus_node_calls_before_the_next_frame_both_take_effect(cx: &mut TestAppContext) {
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
        format!(r#"["focus:{a}","blur:{a}","focus:{b}"]"#),
        "both requests must take effect, in order, like the DOM's synchronous focus()"
    );
}

/// Runs `script` against the host's bindings, then draws one frame.
fn run_then_draw(
    cx: &mut TestAppContext,
    engine: &Engine,
    host: &Rc<RefCell<Host>>,
    parent: NodeId,
    script: &str,
) {
    engine
        .with(|ctx| inca_bridge::bindings::install(&ctx, host))
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::new(Engine::new().unwrap()), Rc::clone(host));
    let window = cx.add_window(|_, _| FocusableRoot {
        host: Rc::clone(host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    engine.eval::<()>(script).unwrap();
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    cx.run_until_parked();
}

#[gpui::test]
fn focus_then_destroy_in_one_tick_leaves_no_handle(cx: &mut TestAppContext) {
    let (host, parent, a) = build_focusable_pair();
    let engine = Engine::new().unwrap();

    let script = format!("__inca_native__.focusNode({a}); __inca_native__.destroyNode({a});");
    run_then_draw(cx, &engine, &host, parent, &script);

    assert!(host.borrow().focus.handle(a).is_none());
}

#[gpui::test]
fn focus_on_an_unknown_node_leaves_no_handle(cx: &mut TestAppContext) {
    let (host, parent, _a) = build_focusable_pair();
    let engine = Engine::new().unwrap();

    run_then_draw(
        cx,
        &engine,
        &host,
        parent,
        "__inca_native__.focusNode(999);",
    );

    assert!(host.borrow().focus.handle(999).is_none());
}

/// `blurNode(id)` blurs only when `id` is the focused node: another node's
/// id, or an unknown one, leaves focus alone.
#[gpui::test]
fn blur_node_of_a_node_that_is_not_focused_is_a_no_op(cx: &mut TestAppContext) {
    for other in [Some(1), None] {
        let (host, parent, a) = build_focusable_pair();
        let other_id = other.map_or(999, |i| {
            host.borrow().tree.get(parent).unwrap().children()[i]
        });
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
            host.borrow_mut().focus.request_blur(other_id);
            window.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();

        let seen = engine.eval::<String>("JSON.stringify(seen)").unwrap();
        assert_eq!(seen, "[]", "blur of {other_id} must not blur {a}");
        let still_focused = cx
            .update_window(window.into(), |_, window, cx| {
                host.borrow().focus.handle(a).unwrap().is_focused(window)
                    && window.focused(cx).is_some()
            })
            .unwrap();
        assert!(still_focused);
    }
}
