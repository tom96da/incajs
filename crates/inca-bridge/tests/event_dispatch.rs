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
    Context, Modifiers, MouseButton, Render, ScrollDelta, ScrollWheelEvent, TestAppContext,
    VisualTestContext, Window, point, prelude::*, px,
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
        format!(r#"{{"type":"click","target":{node},"currentTarget":{node}}}"#)
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

fn build_tree_listening_for(event: &str) -> (Rc<RefCell<Host>>, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let node = host_mut.tree.create_node("div");
    host_mut.tree.set_style(node, "width", 100.0).unwrap();
    host_mut.tree.set_style(node, "height", 100.0).unwrap();
    host_mut.listeners.register(node, event, 0);
    drop(host_mut);
    (host, node)
}

/// A `mousedown`'s payload is DOM-`MouseEvent`-shaped: coordinates, which
/// button, and which buttons are held (just the one just pressed).
#[gpui::test]
fn mousedown_carries_dom_shaped_fields(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousedown");
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

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_mouse_down(
        point(px(10.0), px(20.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert_eq!(clicks(&engine), 1.0);
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.lastEvent)")
            .unwrap(),
        format!(
            r#"{{"type":"mousedown","target":{node},"currentTarget":{node},"clientX":10,"clientY":20,"button":0,"buttons":1,"detail":1,"ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false}}"#
        )
    );
}

/// `preventDefault()` reaches GPUI's own `Window::default_prevented` state.
#[gpui::test]
fn prevent_default_reaches_the_window(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousedown");
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.__inca_callbacks__ = { 0: (event) => { event.preventDefault(); } };",
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
    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let prevented = cx
        .update_window(window.into(), |_, window, _| window.default_prevented())
        .unwrap();
    assert!(prevented, "preventDefault() must reach the window");
}

/// A `wheel`'s payload carries DOM-`WheelEvent`-shaped delta fields plus
/// every field a `mouse` payload does — `WheelEvent` extends `MouseEvent`.
#[gpui::test]
fn wheel_carries_dom_shaped_delta_fields(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("wheel");
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

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-5.0))),
        ..Default::default()
    });

    assert_eq!(clicks(&engine), 1.0);
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.lastEvent)")
            .unwrap(),
        format!(
            r#"{{"type":"wheel","target":{node},"currentTarget":{node},"clientX":10,"clientY":10,"button":0,"buttons":0,"detail":0,"ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"deltaX":0,"deltaY":-5,"deltaZ":0,"deltaMode":0}}"#
        )
    );
}

/// A mouse button held while scrolling still shows up in a `wheel`'s
/// `buttons` — the `held_buttons` tracking `mousedown`/`mouseup` update is
/// shared with every payload kind, not copied per kind.
#[gpui::test]
fn wheel_reports_a_button_held_from_an_earlier_mousedown(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousedown");
    host.borrow_mut().listeners.register(node, "wheel", 0);
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
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Middle,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });

    let buttons: u8 = engine.eval("globalThis.lastEvent.buttons").unwrap();
    assert_eq!(buttons, 0b010);
}

/// A `wheel` on a node nothing listens for must not call into JS — same
/// guarantee `no_listener_registered_reports_nothing` gives `click`.
#[gpui::test]
fn wheel_with_no_listener_reports_nothing(cx: &mut TestAppContext) {
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
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });

    assert_eq!(clicks(&engine), 0.0);
}

/// Two kinds wired on the same node fire independently, at the values each
/// carries — wiring one kind doesn't steal or block another's dispatch.
#[gpui::test]
fn wheel_and_mousedown_wired_together_fire_independently(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("wheel");
    host.borrow_mut().listeners.register(node, "mousedown", 0);
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { globalThis.seen.push(event.type); } \
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

    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        r#"["mousedown","wheel"]"#
    );
}

/// DOM's `buttons` reports every button held, not just the one an event is
/// about: pressing a second button while the first is still down must union
/// its bit in, and releasing one button must clear only that bit.
#[gpui::test]
fn buttons_tracks_every_button_currently_held(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousedown");
    host.borrow_mut().listeners.register(node, "mouseup", 0);
    host.borrow_mut().listeners.register(node, "mousemove", 0);
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
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    let buttons = || -> u8 { engine.eval("globalThis.lastEvent.buttons").unwrap() };
    let point = point(px(10.0), px(10.0));

    cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(buttons(), 0b001, "the just-pressed button alone");

    cx.simulate_mouse_down(point, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(buttons(), 0b101, "both buttons held at once");

    cx.simulate_mouse_move(point, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(buttons(), 0b101, "a move reports every button still held");

    cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(buttons(), 0b100, "releasing one button clears only its bit");
}

/// `stopImmediatePropagation()` stops the remaining callbacks on the *same*
/// node — the second callback below must never run.
#[gpui::test]
fn stop_immediate_propagation_skips_the_rest_of_this_nodes_callbacks(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousedown");
    host.borrow_mut().listeners.register(node, "mousedown", 1);

    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.ran = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { globalThis.ran.push(0); event.stopImmediatePropagation(); }, \
                1: () => { globalThis.ran.push(1); } \
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
    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.ran)")
            .unwrap(),
        "[0]",
        "the second callback on the same node must not run"
    );
}

/// `stopPropagation()` reaches GPUI's own bubble: an ancestor's listener for
/// the same event must not fire once a descendant's callback calls it.
#[gpui::test]
fn stop_propagation_keeps_an_ancestors_listener_from_firing(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let parent = {
        let mut host_mut = host.borrow_mut();
        let parent = host_mut.tree.create_node("div");
        host_mut.tree.set_style(parent, "width", 100.0).unwrap();
        host_mut.tree.set_style(parent, "height", 100.0).unwrap();
        let child = host_mut.tree.create_node("div");
        host_mut.tree.set_style(child, "width", 100.0).unwrap();
        host_mut.tree.set_style(child, "height", 100.0).unwrap();
        host_mut.tree.append_child(parent, child).unwrap();
        host_mut.listeners.register(parent, "mousedown", 0);
        host_mut.listeners.register(child, "mousedown", 1);
        parent
    };

    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.ran = []; \
             globalThis.__inca_callbacks__ = { \
                0: () => { globalThis.ran.push('parent'); }, \
                1: (event) => { globalThis.ran.push('child'); event.stopPropagation(); } \
             };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));

    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.ran)")
            .unwrap(),
        r#"["child"]"#,
        "the parent's listener must not run once the child stopped propagation"
    );
}
