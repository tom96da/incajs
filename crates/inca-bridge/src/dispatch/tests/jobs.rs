// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn record_layout_stores_and_clear_layout_empties() {
    let (dispatcher, host, _) = dispatcher_with_engine();
    let root = host.borrow().root;
    let layout = inca_gpui::NodeLayout {
        bounds: gpui::Bounds::default(),
        border: px(2.0),
        scroll: gpui::Point::default(),
    };

    dispatcher.record_layout(root, layout);
    assert_eq!(host.borrow().layout.borrow().get(&root), Some(&layout));

    dispatcher.clear_layout();
    assert!(host.borrow().layout.borrow().is_empty());
}

/// `inca-gpui` only ever produces a `button` index in `0..=4`, but
/// `update_held_buttons` takes a plain `u8`. A value outside that
/// range must not panic, only fail to set any bit.
#[test]
fn an_out_of_range_button_does_not_panic() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    assert_eq!(dispatcher.update_held_buttons("mousedown", 200), 0);
    assert_eq!(dispatcher.held_buttons.get(), 0);
}

#[test]
fn a_right_mousedown_sets_the_right_buttons_bit() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    assert_eq!(dispatcher.update_held_buttons("mousedown", 2), 0b0_0010);
}

#[test]
fn a_middle_mousedown_sets_the_middle_buttons_bit() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    assert_eq!(dispatcher.update_held_buttons("mousedown", 1), 0b0_0100);
}

#[gpui::test]
fn no_listener_registered_reports_nothing(cx: &mut TestAppContext) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node_id = host.borrow_mut().tree.create_node("div").unwrap();

    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(node_id, "click", &EventPayload::None, window, cx);
    });

    assert!(reported.borrow().is_empty());
}

#[gpui::test]
fn a_stale_callback_id_is_skipped_rather_than_reported(cx: &mut TestAppContext) {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node_id = host.borrow_mut().tree.create_node("div").unwrap();
    host.borrow_mut().listeners.register(node_id, "click", 0);

    // No `__inca_callbacks__` global defined at all.
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
    host.borrow_mut().listeners.register(node_id, "click", 0);
    dispatcher
        .engine
        .eval::<()>("globalThis.__inca_callbacks__ = { 0: () => { throw new Error('boom'); } };")
        .unwrap();

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
    host.borrow_mut().listeners.register(node_id, "click", 0);
    host.borrow_mut().listeners.register(node_id, "click", 1);
    dispatcher
        .engine
        .eval::<()>(
            r"
            globalThis.ran = false;
            globalThis.__inca_callbacks__ = {
               0: () => { throw new Error('boom'); },
               1: () => { globalThis.ran = true; }
            };
            ",
        )
        .unwrap();

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
    listen(&host, node, "ping");
    callbacks(&dispatcher, "0: () => { throw new Error('boom'); }");
    dispatcher
        .engine
        .eval::<()>(&format!(
            r"
            Promise.resolve().then(() =>
            __inca_native__.dispatchEvent({node}, new Event('ping')));
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
