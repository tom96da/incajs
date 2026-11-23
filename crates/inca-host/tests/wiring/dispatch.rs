// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The dispatch of an event into the engine and the jobs it leaves.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_document::{EventPayload, EventTargetId};
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::support::*;
use inca_document::{EVENTS, MousePayload, PointerSource};
use inca_gpui::{EventSink, InputLayout};
use inca_node::NodeId;

#[gpui::test]
fn a_stale_callback_id_is_skipped_rather_than_reported(cx: &mut TestAppContext) {
    let (dispatcher, cells, reported) = dispatcher_with_engine();
    let node_id = cells.document.borrow_mut().create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_native__.addEventListener({node_id}, 'click', () => {{}});"
        ))
        .unwrap();
    dispatcher
        .engine
        .with(|ctx| inca_jsenv::callbacks::release(&ctx, &[1]))
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
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node_id = cells.document.borrow_mut().create_node("div").unwrap();
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
    let (dispatcher, cells, reported) = dispatcher_with_engine();
    let node_id = cells.document.borrow_mut().create_node("div").unwrap();
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
    let (dispatcher, cells, reported) = dispatcher_with_engine();
    let node_id = cells.document.borrow_mut().create_node("div").unwrap();
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
    let (dispatcher, cells, reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
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
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
    let root = cells.document.borrow().root();
    cells
        .document
        .borrow_mut()
        .append_child(&*cells.window.borrow(), root, node)
        .unwrap();
    cells
        .document
        .borrow_mut()
        .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
        .unwrap();
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        cells.document.borrow_mut().mark_tab_dirty(node);
        let _ = apply_focus(&cells, window, cx);
        cells.focus_handle(node).unwrap().focus(window, cx);
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

/// Starts a turn of JS from one entry of the window, with a `read` that a listener or a callback
/// of that turn calls.
type Entry = fn(&crate::support::KeyScene, &mut gpui::Window, &mut gpui::App);

#[gpui_cases]
#[gpui::test]
#[case::pointer(
    "__inca_test_listen__(s.parent, 'mousedown', read);",
    |s, window, cx| {
        use inca_gpui::EventSink as _;
        let payload = crate::support::mouse_at(1.0, 1.0);
        s.dispatcher.dispatcher().pointer_input("mousedown", s.plain, &payload, window, cx);
    }
)]
#[case::key(
    "__inca_test_listen__(s.parent, 'keydown', read);",
    |s, window, cx| {
        use inca_gpui::EventSink as _;
        let keystroke = gpui::Keystroke::parse_with_os("a", cx.platform_info().os).unwrap();
        let payload = inca_document::EventPayload::Key(inca_document::KeyPayload {
            key: "a".into(),
            repeat: false,
            modifiers: inca_document::Modifiers::none(),
            is_composing: false,
        });
        s.dispatcher.dispatcher().key_input(&keystroke, false, &payload, window, cx);
    }
)]
#[case::frame(
    "requestAnimationFrame(read);",
    |s, window, cx| s.dispatcher.dispatcher().run_frame_callbacks(window, cx)
)]
#[case::activation(
    "__inca_test_listen__(s.second, 'focus', read);",
    |s, window, cx| s.dispatcher.dispatcher().window_activated(true, window, cx)
)]
fn a_turn_sees_the_focus_the_window_moved_to_since_the_last_frame(
    cx: &mut TestAppContext,
    install: &str,
    entry: Entry,
) {
    let s = crate::support::js_scene(cx);
    s.focus(cx, s.first);
    s.dispatcher
        .engine
        .eval::<()>(&format!(
            r"
            const s = {{ parent: {}, second: {} }};
            globalThis.seen = undefined;
            const read = () => {{ globalThis.seen = __inca_native__.focusedNode(); }};
            {install}
            ",
            s.parent, s.second
        ))
        .unwrap();

    s.in_window(cx, |s, window, cx| {
        // Focus moved outside the document, as a click does.
        s.cells.focus_handle(s.second).unwrap().focus(window, cx);
        entry(s, window, cx);
    });

    assert_eq!(
        s.dispatcher.engine.eval::<Option<NodeId>>("seen").unwrap(),
        Some(s.second)
    );
}
