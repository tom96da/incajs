// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for `inca_bridge::dispatch`'s event dispatch: a real
//! `VirtualTree`, wired to a real `QuickJS` callback via the
//! `__inca_callbacks__` convention (see `EventDispatcher`'s doc comment),
//! rendered through `render_tree_with_events`.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, Modifiers, Render, TestAppContext, VisualTestContext, Window, point, prelude::*, px,
};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{NodeId, render_tree_with_events};
use inca_jsenv::Engine;

fn build_clickable_tree() -> (Rc<RefCell<Host>>, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let node = host_mut.tree.create_node("div");
    host_mut.tree.set_style(node, "width", 100.0).unwrap();
    host_mut.tree.set_style(node, "height", 100.0).unwrap();
    host_mut.listeners.register(node, "click", 0);
    drop(host_mut);
    (host, node)
}

fn install_click_counter(engine: &Engine) {
    engine
        .eval::<()>(
            "globalThis.__inca_callbacks__ = { \
                0: (event) => { \
                    globalThis.clicks = (globalThis.clicks || 0) + 1; \
                    globalThis.lastEvent = event; \
                } \
            };",
        )
        .unwrap();
}

fn clicks(engine: &Engine) -> f64 {
    engine.eval::<f64>("globalThis.clicks || 0").unwrap()
}

/// Unlike `install_click_counter`, defers the actual mutation to a
/// microtask — the same way a real framework's reactivity scheduler
/// (e.g. `@vue/runtime-core`'s, batched via `Promise.resolve().then(...)`)
/// doesn't apply an effect synchronously within the event callback itself.
fn install_deferred_click_counter(engine: &Engine) {
    engine
        .eval::<()>(
            "globalThis.__inca_callbacks__ = { \
                0: (event) => { \
                    Promise.resolve().then(() => { \
                        globalThis.clicks = (globalThis.clicks || 0) + 1; \
                        globalThis.lastEvent = event; \
                    }); \
                } \
            };",
        )
        .unwrap();
}

/// Building the element tree (what happens on every re-render) must never
/// touch the JS engine by itself — only an actual dispatched event may.
/// Plain `#[test]`: building an `AnyElement` needs no `gpui` App/Window.
#[test]
fn repeated_builds_with_no_event_never_touch_the_js_engine() {
    let (host, node) = build_clickable_tree();
    let engine = Rc::new(Engine::new().unwrap());
    install_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    for _ in 0..5 {
        let host = host.borrow();
        let _ = render_tree_with_events(&host.tree, node, &dispatcher).unwrap();
    }

    assert_eq!(
        clicks(&engine),
        0.0,
        "building the element tree must never call into JS on its own"
    );
}

struct ClickableRoot {
    host: Rc<RefCell<Host>>,
    node: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for ClickableRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher).unwrap()
    }
}

// Note: this doesn't assert on `ClickableRoot`'s render count. A click on
// any interactive element already makes GPUI redraw for its own hover/
// active bookkeeping, regardless of whether `dispatch()` also calls
// `window.refresh()` — confirmed empirically, not just assumed — so a
// render-count assertion here couldn't actually isolate `dispatch()`'s own
// effect. The JS-side call count below is the real, unambiguous signal.
#[gpui::test]
fn click_dispatches_to_js_exactly_once(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = Rc::new(Engine::new().unwrap());
    install_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });

    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    assert_eq!(
        clicks(&engine),
        0.0,
        "mounting must never touch the JS engine"
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(
        clicks(&engine),
        1.0,
        "exactly one click must dispatch exactly one JS call"
    );
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.lastEvent)")
            .unwrap(),
        format!(r#"{{"type":"click","target":{node}}}"#)
    );
}

/// A node is wired for input only while something listens on it, so a
/// listener registered after the first frame has to survive the re-render
/// that follows.
#[gpui::test]
fn a_listener_registered_after_the_first_render_still_fires(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div");
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    let engine = Rc::new(Engine::new().unwrap());
    install_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    // Nothing was listening for that first frame.
    host.borrow_mut().listeners.register(node, "click", 0);

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(clicks(&engine), 1.0);
}

/// The host holds a list of callbacks per `(node, event)`, the way the DOM
/// does, and dispatches to all of it. `packages/core` registers one — a
/// framework adapter composes its own handlers first — so nothing above this
/// layer exercises the list.
#[gpui::test]
fn a_click_reaches_every_callback_registered_for_it(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    host.borrow_mut().listeners.register(node, "click", 1);

    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.__inca_callbacks__ = { \
                0: () => { globalThis.clicks = (globalThis.clicks || 0) + 1; }, \
                1: () => { globalThis.clicks = (globalThis.clicks || 0) + 10; } \
            };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(clicks(&engine), 11.0, "both callbacks must run, once each");
}

/// A callback that only schedules its effect via a microtask (as any real
/// reactivity scheduler does) must still have that effect applied by the
/// time `dispatch` returns — not left pending until some later, unrelated
/// engine call happens to drain the job queue.
#[gpui::test]
fn click_drains_a_callback_that_defers_its_effect_via_a_microtask(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
    let engine = Rc::new(Engine::new().unwrap());
    install_deferred_click_counter(&engine);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(
        clicks(&engine),
        1.0,
        "a microtask-deferred effect must already be applied once dispatch returns"
    );
}
