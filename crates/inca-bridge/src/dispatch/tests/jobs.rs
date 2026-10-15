// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui::test]
fn a_stale_callback_id_is_skipped_rather_than_reported(cx: &mut TestAppContext) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node_id = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_native__.addEventListener({node_id}, 'click');"
        ))
        .unwrap();

    // The listener has no function stored under its id.
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(node_id, "click", &EventPayload::None, window, cx);
    });

    assert!(
        reported.borrow().is_empty(),
        "a registration outliving its function is not a fault to report"
    );
}

#[gpui::test]
fn drain_jobs_and_refresh_runs_what_a_microtask_only_scheduled(cx: &mut TestAppContext) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    dispatcher
        .engine
        .eval::<()>("globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);")
        .unwrap();
    assert!(
        !dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap(),
        "the microtask must still be pending before the drain"
    );

    let cx = cx.add_empty_window();
    cx.update(|window, cx| dispatcher.drain_jobs_and_refresh(window, cx));

    assert!(dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap());
}

#[gpui::test]
fn a_pending_job_waits_when_no_listener_ran(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node_id = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>("globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);")
        .unwrap();

    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(node_id, "click", &EventPayload::None, window, cx);
    });

    assert!(
        !dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap(),
        "an event nobody hears neither enters the engine nor drains it"
    );
}

#[gpui::test]
fn a_throwing_callback_is_reported(cx: &mut TestAppContext) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node_id = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node_id,
        "click",
        "() => { throw new Error('boom'); }",
    );

    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(node_id, "click", &EventPayload::None, window, cx);
    });

    let reported = reported.borrow();
    assert_eq!(reported.len(), 1);
    assert_eq!(reported[0].message(), "Error: boom");
    assert!(reported[0].stack().is_some());
}

#[gpui::test]
fn one_throwing_callback_does_not_stop_the_others(cx: &mut TestAppContext) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node_id = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>("globalThis.ran = false;")
        .unwrap();
    listen(
        &dispatcher,
        node_id,
        "click",
        "() => { throw new Error('boom'); }",
    );
    listen(
        &dispatcher,
        node_id,
        "click",
        "() => { globalThis.ran = true; }",
    );

    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(node_id, "click", &EventPayload::None, window, cx);
    });

    assert_eq!(reported.borrow().len(), 1);
    assert!(dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap());
}

#[gpui::test]
fn listener_errors_of_a_dispatch_in_a_job_are_reported_when_the_jobs_drain(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "ping",
        "() => { throw new Error('boom'); }",
    );
    dispatcher
        .engine
        .eval::<()>(&format!(
            r"
            Promise.resolve().then(() =>
                __inca_native__.dispatchEvent({node}, new Event('ping'))
            );
            "
        ))
        .unwrap();
    let cx = cx.add_empty_window();

    cx.update(|window, cx| dispatcher.drain_jobs_and_refresh(window, cx));

    assert_eq!(reported.borrow().len(), 1);
    assert_eq!(reported.borrow()[0].message(), "Error: boom");
}

#[gpui::test]
fn drain_jobs_and_refresh_shows_js_the_focus_the_window_applied(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    let root = host.borrow().root;
    host.borrow_mut().tree.append_child(root, node).unwrap();
    host.borrow_mut()
        .tree
        .set_attribute(node, "tabindex", inca_gpui::AttributeValue::from("0"))
        .unwrap();
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        let mut host = host.borrow_mut();
        let host = &mut *host;
        host.focus.mark_tab_dirty(node);
        let _ = host.focus.apply_pending(&host.tree, window, cx);
        host.focus.handle(node).unwrap().focus(window, cx);
    });
    dispatcher
        .engine
        .eval::<()>(
            "var seen; Promise.resolve().then(() => { seen = __inca_native__.focusedNode(); });",
        )
        .unwrap();

    cx.update(|window, cx| dispatcher.drain_jobs_and_refresh(window, cx));

    assert_eq!(
        dispatcher.engine.eval::<u64>("seen").unwrap(),
        u64::from(node)
    );
}
