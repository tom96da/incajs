// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

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

/// Unlike `install_click_counter`, defers the actual mutation to a
/// microtask, the same way a real framework's reactivity scheduler
/// (batched via `Promise.resolve().then(...)`)
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
/// touch the JS engine by itself. Only an actual dispatched event may.
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

// Note: this doesn't assert on `ClickableRoot`'s render count. A click on
// any interactive element already makes GPUI redraw for its own hover/
// active bookkeeping, regardless of whether the walk also calls
// `window.refresh()`, so a render-count assertion cannot isolate the walk's
// own effect. The JS-side call count below is the signal.
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
                r#"{{"type":"click","target":{node},"currentTarget":null,"#,
                r#""bubbles":true,"cancelable":true,"composed":true,"eventPhase":0,"isTrusted":true,"#,
                r#""clientX":10,"clientY":10,"screenX":10,"screenY":10,"offsetX":10,"offsetY":10,"x":10,"y":10,"relatedTarget":null,"pageX":10,"pageY":10,"#,
                r#""movementX":0,"movementY":0,"button":0,"buttons":0,"detail":1,"#,
                r#""pointerId":1,"pointerType":"mouse","isPrimary":true,"width":1,"height":1,"pressure":0,"#,
                r#""ctrlKey":false,"shiftKey":false,"altKey":false,"metaKey":false,"eventId":1}}"#
            ),
            node = node
        )
    );
}

/// The containers track the pointer only while something listens or can take
/// focus, so a listener registered after the first frame has to survive the
/// re-render that follows.
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

/// The host holds a list of callbacks per `(node, event)` and dispatches to
/// all of it. `packages/core` registers one (a
/// framework adapter composes its own handlers first), so nothing above this
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
/// time the walk returns.
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
