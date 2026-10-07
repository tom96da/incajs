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
    Context, Modifiers, MouseButton, MouseExitEvent, Render, ScrollDelta, ScrollWheelEvent,
    TestAppContext, VisualTestContext, Window, point, prelude::*, px,
};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{EventSink, NodeId, render_tree_with_events};
use inca_jsenv::{Engine, EngineError};

fn build_clickable_tree() -> (Rc<RefCell<Host>>, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let node = host_mut.tree.create_node("div").unwrap();
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

/// The `eventId` of the event `install_click_counter` saw last, 0 if none.
fn last_event_id(engine: &Engine) -> u64 {
    engine
        .eval::<u64>("globalThis.lastEvent?.eventId ?? 0")
        .unwrap()
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
            .eval::<String>("JSON.stringify({ ...globalThis.lastEvent, timeStamp: undefined })")
            .unwrap(),
        format!(
            concat!(
                r#"{{"type":"click","target":{node},"currentTarget":{node},"#,
                r#""bubbles":true,"cancelable":true,"composed":true,"defaultPrevented":false,"eventPhase":2,"isTrusted":true,"eventId":1}}"#
            ),
            node = node
        )
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
        let node = host.tree.create_node("div").unwrap();
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

/// Clicks the node once with `body` as its callback and returns what the
/// dispatcher reported.
fn failures_from_click(cx: &mut TestAppContext, body: &str) -> Vec<EngineError> {
    let (host, node) = build_clickable_tree();
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(&format!("globalThis.__inca_callbacks__ = {{ 0: {body} }};"))
        .unwrap();
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&reported);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host))
        .with_reporter(Rc::new(move |err| sink.borrow_mut().push(err.clone())));

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

    reported.take()
}

#[gpui::test]
fn an_async_callback_that_throws_is_reported_like_a_sync_one(cx: &mut TestAppContext) {
    let sync = failures_from_click(cx, "() => { throw new Error('boom'); }");
    let asynchronous = failures_from_click(cx, "async () => { throw new Error('boom'); }");

    assert_eq!(sync.len(), 1);
    assert_eq!(asynchronous.len(), 1);
    assert_eq!(asynchronous[0].message(), sync[0].message());
    assert!(asynchronous[0].stack().is_some());
}

#[gpui::test]
fn a_failing_microtask_is_reported_once(cx: &mut TestAppContext) {
    let failures = failures_from_click(
        cx,
        "() => { queueMicrotask(() => { throw new Error('later'); }); }",
    );

    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].message(), "Error: later");
}

#[gpui::test]
fn a_rejection_handled_in_time_is_not_reported(cx: &mut TestAppContext) {
    let failures = failures_from_click(cx, "async () => { throw new Error('caught'); }");
    assert_eq!(failures.len(), 1);

    let failures = failures_from_click(
        cx,
        "() => { (async () => { throw new Error('caught'); })().catch(() => {}); }",
    );
    assert!(failures.is_empty(), "{failures:?}");
}

fn build_tree_listening_for(event: &str) -> (Rc<RefCell<Host>>, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let node = host_mut.tree.create_node("div").unwrap();
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
            .eval::<String>("JSON.stringify({ ...globalThis.lastEvent, timeStamp: undefined })")
            .unwrap(),
        format!(
            concat!(
                r#"{{"type":"mousedown","target":{node},"currentTarget":{node},"#,
                r#""bubbles":true,"cancelable":true,"composed":true,"defaultPrevented":false,"#,
                r#""eventPhase":2,"isTrusted":true,"#,
                r#""clientX":10,"clientY":20,"pageX":10,"pageY":20,"#,
                r#""movementX":0,"movementY":0,"button":0,"buttons":1,"detail":1,"#,
                r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"#,
                r#""eventId":1}}"#
            ),
            node = node
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
            .eval::<String>("JSON.stringify({ ...globalThis.lastEvent, timeStamp: undefined })")
            .unwrap(),
        format!(
            concat!(
                r#"{{"type":"wheel","target":{node},"currentTarget":{node},"#,
                r#""bubbles":true,"cancelable":true,"composed":true,"defaultPrevented":false,"#,
                r#""eventPhase":2,"isTrusted":true,"#,
                r#""clientX":10,"clientY":10,"pageX":10,"pageY":10,"#,
                r#""movementX":0,"movementY":0,"button":0,"buttons":0,"detail":0,"#,
                r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"#,
                r#""deltaX":0,"deltaY":5,"deltaZ":0,"deltaMode":0,"#,
                r#""eventId":1}}"#
            ),
            node = node
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
    assert_eq!(buttons, 0b0_0100);
}

/// A `wheel` on a node nothing listens for must not call into JS — same
/// guarantee `no_listener_registered_reports_nothing` gives `click`.
#[gpui::test]
fn wheel_with_no_listener_reports_nothing(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
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

/// `click` (which needs a `gpui` element id) and `mousedown` (which doesn't)
/// wired on the same node both still fire — the id-requiring branch must
/// still wire the stateless kinds, not just the id-requiring one.
#[gpui::test]
fn click_and_mousedown_wired_together_fire_independently(cx: &mut TestAppContext) {
    let (host, node) = build_clickable_tree();
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

    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        r#"["mousedown","click"]"#
    );
}

/// `on_hover` covers both `mouseenter` and `mouseleave` with one GPUI
/// registration — moving the pointer in, then out, must dispatch each
/// exactly once, in order.
#[gpui::test]
fn hover_dispatches_enter_then_leave(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    host.borrow_mut().listeners.register(node, "mouseenter", 0);
    host.borrow_mut().listeners.register(node, "mouseleave", 0);
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

    // The window starts with the mouse at its default position, which lands
    // inside this 100x100 node, so mounting alone already fired a real
    // "mouseenter" — assert that before discarding it, so this test proves
    // the reset below is throwing away a real event, not a no-op.
    assert_ne!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        "[]",
        "mounting already hovered must have dispatched mouseenter by itself"
    );
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        r#"["mouseenter","mouseleave"]"#
    );
}

/// A `mouseenter` carries the same DOM-`MouseEvent`-shaped fields as
/// `mousedown`/etc, built from the pointer's current position rather than
/// a native GPUI event (`on_hover` hands back only a `bool`).
#[gpui::test]
fn mouseenter_carries_dom_shaped_fields(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    host.borrow_mut().listeners.register(node, "mouseenter", 0);
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

    // Settle the implicit initial hover (the window's default mouse
    // position lands inside this 100x100 node) before the transition the
    // assertions below are about.
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    let previous = last_event_id(&engine);
    engine.eval::<()>("globalThis.clicks = 0;").unwrap();

    cx.simulate_mouse_move(
        point(px(10.0), px(20.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    assert_eq!(clicks(&engine), 1.0);
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify({ ...globalThis.lastEvent, eventId: undefined, timeStamp: undefined })")
            .unwrap(),
        format!(
            concat!(
                r#"{{"type":"mouseenter","target":{node},"currentTarget":{node},"#,
                r#""bubbles":false,"cancelable":false,"composed":false,"defaultPrevented":false,"#,
                r#""eventPhase":2,"isTrusted":true,"#,
                r#""clientX":10,"clientY":20,"pageX":10,"pageY":20,"#,
                r#""movementX":-190,"movementY":-180,"button":0,"buttons":0,"detail":0,"#,
                r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false}}"#
            ),
            node = node
        )
    );
    assert!(last_event_id(&engine) > previous);
}

/// A node listening only for `mouseleave` doesn't spuriously fire it on
/// mount just because the pointer already sits over it — `was_hovered`
/// starts `false`, so the mount-time check can only ever produce an enter
/// transition, never a leave one, regardless of which names JS registered.
#[gpui::test]
fn mounting_already_hovered_does_not_fire_a_spurious_leave(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mouseleave");
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
    let _cx = VisualTestContext::from_window(window.into(), cx);

    assert_eq!(
        clicks(&engine),
        0.0,
        "a mouseleave-only listener must not fire just from mounting already hovered"
    );
}

/// Hover (needs a `gpui` element id) and a stateless kind wired on the same
/// node both still fire — mirrors `click_and_mousedown_wired_together_
/// fire_independently` for the hover branch chained onto `wire_stateless`.
#[gpui::test]
fn hover_and_mousemove_wired_together_fire_independently(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    host.borrow_mut().listeners.register(node, "mouseenter", 0);
    host.borrow_mut().listeners.register(node, "mousemove", 0);
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

    // Settle the implicit initial hover before the transition below.
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    // Membership only, not order — which of the two GPUI dispatches first
    // is its own internal detail, not a contract this crate makes.
    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen.slice().sort())")
            .unwrap(),
        r#"["mouseenter","mousemove"]"#
    );
}

/// One pointer move produces both `mousemove` and `mouseenter`, and both
/// report that move's delta.
#[gpui::test]
fn one_move_gives_mousemove_and_mouseenter_the_same_movement(cx: &mut TestAppContext) {
    let host = Rc::new(RefCell::new(Host::default()));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        host.tree.set_style(node, "width", 100.0).unwrap();
        host.tree.set_style(node, "height", 100.0).unwrap();
        node
    };
    host.borrow_mut().listeners.register(node, "mouseenter", 0);
    host.borrow_mut().listeners.register(node, "mousemove", 0);
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { \
                0: (event) => { \
                    globalThis.seen.push([event.type, event.movementX, event.movementY]); \
                } \
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

    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.simulate_mouse_move(
        point(px(10.0), px(20.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    let seen: serde_json::Value = serde_json::from_str(
        &engine
            .eval::<String>("JSON.stringify(globalThis.seen.slice().sort())")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        seen,
        serde_json::json!([["mouseenter", -190, -180], ["mousemove", -190, -180]])
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
    assert_eq!(buttons(), 0b0_0001, "the just-pressed button alone");

    cx.simulate_mouse_down(point, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(buttons(), 0b0_0011, "both buttons held at once");

    cx.simulate_mouse_move(point, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        buttons(),
        0b0_0011,
        "a move reports every button still held"
    );

    cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        buttons(),
        0b0_0010,
        "releasing one button clears only its bit"
    );
}

/// `movementX`/`movementY` are a delta from the last pointer move, measured
/// from the pointer's own previous position.
#[gpui::test]
fn movement_tracks_the_delta_from_the_last_pointer_move(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousemove");
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

    let movement = || -> (f32, f32) {
        (
            engine.eval("globalThis.lastEvent.movementX").unwrap(),
            engine.eval("globalThis.lastEvent.movementY").unwrap(),
        )
    };

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(movement(), (0.0, 0.0), "nothing to take a delta from yet");

    cx.simulate_mouse_move(
        point(px(30.0), px(5.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(movement(), (20.0, -5.0));
}

/// One `mousemove` bubbling from a child to its listening parent reports
/// the same `movementX`/`movementY` to both.
#[gpui::test]
fn bubbled_mousemove_gives_every_listener_the_same_movement(cx: &mut TestAppContext) {
    let (host, parent) = build_tree_listening_for("mousemove");
    let child = {
        let mut host = host.borrow_mut();
        let child = host.tree.create_node("div").unwrap();
        host.tree.set_style(child, "width", 100.0).unwrap();
        host.tree.set_style(child, "height", 100.0).unwrap();
        host.tree.append_child(parent, child).unwrap();
        host.listeners.register(child, "mousemove", 0);
        child
    };
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { 0: (e) => { \
                 globalThis.seen.push([e.target, e.movementX, e.movementY]); } };",
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

    for (x, y) in [(10.0, 10.0), (30.0, 5.0)] {
        cx.simulate_mouse_move(point(px(x), px(y)), None::<MouseButton>, Modifiers::none());
    }
    cx.run_until_parked();

    let seen: Vec<(u32, f32, f32)> =
        serde_json::from_str(&engine.eval::<String>("JSON.stringify(seen)").unwrap()).unwrap();
    assert_eq!(seen.len(), 4, "each move reaches the child then the parent");
    assert!(seen.iter().any(|&(target, ..)| target == child));
    assert_eq!((seen[0].1, seen[0].2), (seen[1].1, seen[1].2));
    assert_eq!((seen[2].1, seen[2].2), (20.0, -5.0));
    assert_eq!((seen[3].1, seen[3].2), (20.0, -5.0));
}

/// An inert child never becomes the target, so the listening parent does.
#[gpui::test]
fn an_inert_child_is_never_the_event_target(cx: &mut TestAppContext) {
    let (host, parent) = build_tree_listening_for("mousemove");
    {
        let mut host = host.borrow_mut();
        let child = host.tree.create_node("div").unwrap();
        host.tree.set_style(child, "width", 100.0).unwrap();
        host.tree.set_style(child, "height", 100.0).unwrap();
        host.tree.set_attribute(child, "inert", true).unwrap();
        host.tree.append_child(parent, child).unwrap();
        host.listeners.register(child, "mousemove", 0);
    }
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { 0: (e) => { \
                 globalThis.seen.push(e.target); } };",
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

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let seen: Vec<u32> =
        serde_json::from_str(&engine.eval::<String>("JSON.stringify(seen)").unwrap()).unwrap();
    assert_eq!(seen, vec![parent]);
}

/// A pointer move from one sibling into the next gives the first sibling's
/// `mouseleave` and the second's `mousemove` the same movement.
#[gpui::test]
fn leave_and_mousemove_on_different_nodes_share_one_movement(cx: &mut TestAppContext) {
    let (host, root) = build_tree_listening_for("click");
    {
        let mut host = host.borrow_mut();
        for (event, id) in [("mouseleave", 0), ("mousemove", 1)] {
            let child = host.tree.create_node("div").unwrap();
            host.tree.set_style(child, "width", 100.0).unwrap();
            host.tree.set_style(child, "height", 100.0).unwrap();
            host.tree.append_child(root, child).unwrap();
            host.listeners.register(child, event, id);
            if id == 0 {
                host.listeners.register(child, "mousemove", id);
            }
        }
    }
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             const record = (e) => { \
                 globalThis.seen.push([e.type, e.movementX, e.movementY]); }; \
             globalThis.__inca_callbacks__ = { 0: record, 1: record };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&host),
        node: root,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    engine.eval::<()>("globalThis.seen = [];").unwrap();

    cx.simulate_mouse_move(
        point(px(30.0), px(150.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    cx.run_until_parked();

    let seen: serde_json::Value = serde_json::from_str(
        &engine
            .eval::<String>("JSON.stringify(globalThis.seen.slice().sort())")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        seen,
        serde_json::json!([["mouseleave", 20, 140], ["mousemove", 20, 140]])
    );
}

/// A second genuine `mousemove` at the same position reports no movement,
/// even though the first one at that position did.
#[gpui::test]
fn repeated_mousemove_at_one_position_reports_zero_movement(cx: &mut TestAppContext) {
    let (host, node) = build_tree_listening_for("mousemove");
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

    let mut movements = Vec::new();
    for x in [5.0, 10.0, 10.0] {
        cx.simulate_mouse_move(
            point(px(x), px(10.0)),
            None::<MouseButton>,
            Modifiers::none(),
        );
        cx.run_until_parked();
        movements.push(
            engine
                .eval::<f32>("globalThis.lastEvent.movementX")
                .unwrap(),
        );
    }

    assert_eq!(movements, [0.0, 5.0, 0.0]);
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
        let parent = host_mut.tree.create_node("div").unwrap();
        host_mut.tree.set_style(parent, "width", 100.0).unwrap();
        host_mut.tree.set_style(parent, "height", 100.0).unwrap();
        let child = host_mut.tree.create_node("div").unwrap();
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

/// A row 300 wide holding pane A (0..100) and pane B (100..200), each 100x100.
/// Callback 0 records `[type, movementX, movementY]` into `globalThis.seen`.
/// `a_events` are registered on A and `root_events` on the row; B has no
/// listener.
struct Panes {
    host: Rc<RefCell<Host>>,
    root: NodeId,
    a: NodeId,
    b: NodeId,
    engine: Rc<Engine>,
    dispatcher: EventDispatcher,
}

fn build_panes(a_events: &[&str], root_events: &[&str]) -> Panes {
    let host = Rc::new(RefCell::new(Host::default()));
    let (root, a, b) = {
        let mut host = host.borrow_mut();
        let root = host.tree.create_node("div").unwrap();
        host.tree.set_style(root, "display", "flex").unwrap();
        host.tree.set_style(root, "flex_direction", "row").unwrap();
        host.tree.set_style(root, "width", 300.0).unwrap();
        host.tree.set_style(root, "height", 100.0).unwrap();
        let mut pane = || {
            let pane = host.tree.create_node("div").unwrap();
            host.tree.set_style(pane, "width", 100.0).unwrap();
            host.tree.set_style(pane, "height", 100.0).unwrap();
            host.tree.append_child(root, pane).unwrap();
            pane
        };
        let (a, b) = (pane(), pane());
        for event in a_events {
            host.listeners.register(a, *event, 0);
        }
        for event in root_events {
            host.listeners.register(root, *event, 0);
        }
        (root, a, b)
    };
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { 0: (e) => { \
                 globalThis.seen.push([e.type, e.movementX, e.movementY]); } };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    Panes {
        host,
        root,
        a,
        b,
        engine,
        dispatcher,
    }
}

/// Mounts `panes` and draws the first frame.
fn mount_panes(cx: &mut TestAppContext, panes: &Panes) -> VisualTestContext {
    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&panes.host),
        node: panes.root,
        dispatcher: panes.dispatcher.clone(),
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    VisualTestContext::from_window(window.into(), cx)
}

/// What the callbacks recorded since the last [`take_seen`], in order.
fn take_seen(engine: &Engine) -> Vec<(String, f32, f32)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

fn seen_event(name: &str, x: f32, y: f32) -> (String, f32, f32) {
    (name.to_owned(), x, y)
}

fn move_to(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_mouse_move(point(px(x), px(y)), None::<MouseButton>, Modifiers::none());
}

/// Redraws with the pointer still, so a changed layout is hit-tested again.
fn relayout(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn widen_a(panes: &Panes) {
    panes
        .host
        .borrow_mut()
        .tree
        .set_style(panes.a, "width", 200.0)
        .unwrap();
}

/// A click at one position and a move over an unlistened area leave the next
/// hover with nothing to measure from the click.
#[gpui::test]
fn a_hover_after_a_click_elsewhere_reports_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &["mousedown"]);
    let mut cx = mount_panes(cx, &panes);

    cx.simulate_mouse_down(
        point(px(160.0), px(60.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    move_to(&mut cx, 170.0, 60.0);
    take_seen(&panes.engine);

    widen_a(&panes);
    relayout(&mut cx);

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseenter", 0.0, 0.0)]
    );
}

#[gpui::test]
fn a_layout_change_under_a_still_pointer_gives_mouseenter_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &[]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 20.0, 20.0);
    move_to(&mut cx, 150.0, 20.0);
    take_seen(&panes.engine);

    widen_a(&panes);
    relayout(&mut cx);

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseenter", 0.0, 0.0)]
    );
}

/// An element already under the pointer when it mounts is entered with no
/// movement, whatever an earlier move left behind.
#[gpui::test]
fn a_mount_time_hover_reports_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &[]);
    // The window's pointer starts at (0,0). The seed is a nonzero baseline.
    cx.update(|app| {
        panes
            .dispatcher
            .pointer_moved(point(px(40.0), px(70.0)), app);
    });

    let _cx = mount_panes(cx, &panes);

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseenter", 0.0, 0.0)]
    );
}

/// Moves over a node nothing listens on still advance the baseline.
#[gpui::test]
fn moves_over_an_unlistened_node_advance_the_movement_baseline(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &[]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    move_to(&mut cx, 150.0, 10.0);
    move_to(&mut cx, 160.0, 10.0);
    move_to(&mut cx, 10.0, 10.0);

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseenter", -150.0, 0.0)]
    );
}

/// A drag move is a pointer move like any other.
#[gpui::test]
fn a_drag_move_over_an_unlistened_area_advances_the_baseline(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &[]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    cx.simulate_mouse_move(
        point(px(150.0), px(10.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    move_to(&mut cx, 10.0, 10.0);

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseenter", -140.0, 0.0)]
    );
}

/// A `mousedown` between the two moves leaves the baseline in place.
#[gpui::test]
fn mouseleave_from_a_move_reports_that_moves_mousemove_delta(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseleave"], &["mousemove", "mousedown"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    cx.simulate_mouse_down(
        point(px(60.0), px(60.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    take_seen(&panes.engine);

    move_to(&mut cx, 200.0, 10.0);

    let mut seen = take_seen(&panes.engine);
    seen.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        seen,
        [
            seen_event("mouseleave", 190.0, 0.0),
            seen_event("mousemove", 190.0, 0.0)
        ]
    );
}

/// After a window exit, a layout change under the pointer enters with no
/// movement.
#[gpui::test]
fn a_hover_after_a_window_exit_reports_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &[]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    move_to(&mut cx, 150.0, 20.0);
    cx.simulate_event(MouseExitEvent {
        position: point(px(150.0), px(20.0)),
        ..Default::default()
    });
    take_seen(&panes.engine);

    widen_a(&panes);
    relayout(&mut cx);

    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseenter", 0.0, 0.0)]
    );
}

fn wheel_at(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Lines(point(0.0, 1.0)),
        ..Default::default()
    });
}

/// With no pointer move yet, a wheel has nothing to measure from.
#[gpui::test]
fn a_wheel_before_any_move_reports_zero_movement(cx: &mut TestAppContext) {
    let panes = build_panes(&[], &["wheel"]);
    let mut cx = mount_panes(cx, &panes);
    take_seen(&panes.engine);

    wheel_at(&mut cx, 30.0, 40.0);

    assert_eq!(take_seen(&panes.engine), [seen_event("wheel", 0.0, 0.0)]);
}

/// A wheel measures from the last move, and from nothing after a window exit.
#[gpui::test]
fn a_wheel_measures_from_the_last_move_and_from_nothing_after_an_exit(cx: &mut TestAppContext) {
    let panes = build_panes(&[], &["wheel"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    wheel_at(&mut cx, 30.0, 40.0);
    assert_eq!(take_seen(&panes.engine), [seen_event("wheel", 20.0, 30.0)]);

    cx.simulate_event(MouseExitEvent {
        position: point(px(10.0), px(10.0)),
        ..Default::default()
    });
    wheel_at(&mut cx, 30.0, 40.0);
    assert_eq!(take_seen(&panes.engine), [seen_event("wheel", 0.0, 0.0)]);
}

/// A window exit's `mouseleave` reports 0, and so does the first move after
/// the pointer comes back.
#[gpui::test]
fn a_window_exit_gives_mouseleave_zero_and_resets_the_next_move(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseleave"], &["mousemove"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);

    cx.simulate_event(MouseExitEvent {
        position: point(px(10.0), px(10.0)),
        ..Default::default()
    });
    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mouseleave", 0.0, 0.0)]
    );

    move_to(&mut cx, 50.0, 80.0);
    assert_eq!(
        take_seen(&panes.engine),
        [seen_event("mousemove", 0.0, 0.0)]
    );
}

/// `mousedown`, `mouseup` and `wheel` measure from the last pointer move and
/// leave it where it was, so the move that follows reports the same distance.
#[gpui::test]
fn button_and_wheel_events_measure_from_the_last_move_and_leave_it_in_place(
    cx: &mut TestAppContext,
) {
    let panes = build_panes(&[], &["mousedown", "mouseup", "wheel", "mousemove"]);
    let mut cx = mount_panes(cx, &panes);
    move_to(&mut cx, 10.0, 10.0);
    take_seen(&panes.engine);
    let at = point(px(30.0), px(40.0));

    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    wheel_at(&mut cx, 30.0, 40.0);
    move_to(&mut cx, 30.0, 40.0);

    assert_eq!(
        take_seen(&panes.engine),
        [
            seen_event("mousedown", 20.0, 30.0),
            seen_event("mouseup", 20.0, 30.0),
            seen_event("wheel", 20.0, 30.0),
            seen_event("mousemove", 20.0, 30.0),
        ]
    );
}

/// The pointer tracker is a zero-size child: the root and its panes keep
/// their bounds.
#[gpui::test]
fn the_pointer_tracker_leaves_the_layout_unchanged(cx: &mut TestAppContext) {
    let panes = build_panes(&["mouseenter"], &[]);
    let mut cx = mount_panes(cx, &panes);

    let root = cx.debug_bounds("node-1").unwrap();
    assert_eq!(root.origin, point(px(0.0), px(0.0)));
    assert_eq!(root.size, gpui::size(px(300.0), px(100.0)));
    let a = cx.debug_bounds("node-2").unwrap();
    let b = cx.debug_bounds("node-3").unwrap();
    assert_eq!(a.origin, root.origin);
    assert_eq!(b.origin, point(px(100.0), px(0.0)));
    assert_eq!((panes.root, panes.a, panes.b), (1, 2, 3));
}

/// A parent over a child, both listening for `events`; callback 0 records
/// `[type, currentTarget, eventId]` into `globalThis.seen`.
fn mount_recording_pair(
    cx: &mut TestAppContext,
    events: &[&str],
) -> (VisualTestContext, Rc<Engine>, NodeId, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let (parent, child) = {
        let mut host = host.borrow_mut();
        let parent = host.tree.create_node("div").unwrap();
        host.tree.set_style(parent, "width", 100.0).unwrap();
        host.tree.set_style(parent, "height", 100.0).unwrap();
        let child = host.tree.create_node("div").unwrap();
        host.tree.set_style(child, "width", 100.0).unwrap();
        host.tree.set_style(child, "height", 100.0).unwrap();
        host.tree.append_child(parent, child).unwrap();
        for event in events {
            host.listeners.register(parent, *event, 0);
            host.listeners.register(child, *event, 0);
        }
        (parent, child)
    };
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            "globalThis.seen = []; \
             globalThis.__inca_callbacks__ = { 0: (e) => { \
                 globalThis.seen.push([e.type, e.currentTarget, e.eventId]); } };",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| ClickableRoot {
        host,
        node: parent,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    (
        VisualTestContext::from_window(window.into(), cx),
        engine,
        parent,
        child,
    )
}

fn take_ids(engine: &Engine) -> Vec<(String, NodeId, u64)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

fn mouse_down(cx: &mut VisualTestContext) {
    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
}

#[gpui::test]
fn every_node_on_a_bubble_path_sees_the_same_event_id(cx: &mut TestAppContext) {
    let (mut cx, engine, parent, child) = mount_recording_pair(cx, &["mousedown"]);

    mouse_down(&mut cx);

    assert_eq!(
        take_ids(&engine),
        [
            ("mousedown".to_owned(), child, 1),
            ("mousedown".to_owned(), parent, 1)
        ]
    );
}

#[gpui::test]
fn each_input_event_gets_the_next_event_id(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["mousedown"]);

    mouse_down(&mut cx);
    mouse_down(&mut cx);
    mouse_down(&mut cx);

    let ids: Vec<u64> = take_ids(&engine).iter().map(|seen| seen.2).collect();
    assert_eq!(ids, [1, 1, 2, 2, 3, 3]);
}

#[gpui::test]
fn a_click_has_its_own_event_id_after_its_mousedown(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, child) = mount_recording_pair(cx, &["mousedown", "click"]);

    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());

    let seen = take_ids(&engine);
    let of = |name: &str, node: NodeId| {
        seen.iter()
            .find(|entry| entry.0 == name && entry.1 == node)
            .unwrap()
            .2
    };
    assert!(of("click", child) > of("mousedown", child));
}

#[gpui::test]
fn wheel_and_mouseenter_carry_an_event_id(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["wheel", "mouseenter"]);
    let entered = take_ids(&engine);
    assert!(!entered.is_empty() && entered.iter().all(|seen| seen.2 >= 1));
    let before = entered.iter().map(|seen| seen.2).max().unwrap();

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.0), px(10.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-5.0))),
        ..Default::default()
    });

    let wheel = take_ids(&engine);
    assert_eq!(wheel.len(), 2);
    assert!(
        wheel
            .iter()
            .all(|seen| seen.0 == "wheel" && seen.2 > before)
    );
    assert_eq!(wheel[0].2, wheel[1].2);
}

#[gpui::test]
fn mouseup_and_click_each_keep_one_event_id_across_a_bubble_path(cx: &mut TestAppContext) {
    let (mut cx, engine, parent, child) = mount_recording_pair(cx, &["mouseup", "click"]);

    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());

    let seen = take_ids(&engine);
    let id_of = |name: &str, node: NodeId| {
        seen.iter()
            .find(|entry| entry.0 == name && entry.1 == node)
            .map(|entry| entry.2)
            .unwrap()
    };
    assert_eq!(seen.len(), 4);
    assert_eq!(id_of("mouseup", child), id_of("mouseup", parent));
    assert_eq!(id_of("click", child), id_of("click", parent));
    assert_ne!(id_of("mouseup", child), id_of("click", child));
}

#[gpui::test]
fn two_wheel_events_get_different_event_ids(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["wheel"]);

    for _ in 0..2 {
        cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.0), px(10.0)),
            delta: ScrollDelta::Pixels(point(px(0.0), px(-5.0))),
            ..Default::default()
        });
    }

    let ids: Vec<u64> = take_ids(&engine).iter().map(|seen| seen.2).collect();
    assert_eq!(ids.len(), 4);
    assert_eq!(ids[0], ids[1]);
    assert_eq!(ids[2], ids[3]);
    assert!(ids[2] > ids[0]);
}

#[gpui::test]
fn mouseleave_carries_an_event_id_shared_along_its_bubble_path(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["mouseleave"]);
    take_ids(&engine);

    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    let seen = take_ids(&engine);
    assert!(!seen.is_empty());
    assert!(
        seen.iter()
            .all(|entry| entry.0 == "mouseleave" && entry.2 >= 1)
    );
    assert!(seen.iter().all(|entry| entry.2 == seen[0].2));
}

#[gpui::test]
fn click_and_mouseup_alternate_down_the_bubble_path_with_one_id_per_name(cx: &mut TestAppContext) {
    let (mut cx, engine, parent, child) = mount_recording_pair(cx, &["click", "mouseup"]);

    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());

    let seen = take_ids(&engine);
    let order: Vec<(&str, NodeId)> = seen
        .iter()
        .map(|entry| (entry.0.as_str(), entry.1))
        .collect();
    assert_eq!(
        order,
        [
            ("click", child),
            ("mouseup", child),
            ("click", parent),
            ("mouseup", parent)
        ]
    );
    assert_eq!(seen[0].2, seen[2].2);
    assert_eq!(seen[1].2, seen[3].2);
    assert_ne!(seen[0].2, seen[1].2);
}

#[gpui::test]
fn a_moves_mousemove_gets_a_later_id_than_its_mouseenter(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["mousemove", "mouseenter"]);
    cx.simulate_mouse_move(
        point(px(200.0), px(200.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );
    take_ids(&engine);

    cx.simulate_mouse_move(
        point(px(10.0), px(10.0)),
        None::<MouseButton>,
        Modifiers::none(),
    );

    let seen = take_ids(&engine);
    let first_of = |name: &str| seen.iter().find(|entry| entry.0 == name).unwrap().2;
    assert!(first_of("mousemove") > first_of("mouseenter"));
}
