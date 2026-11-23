// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a script sees of the timers a window runs.

#![allow(clippy::unwrap_used, unused_imports)]

use std::rc::Rc;

use crate::support::Rig;
use crate::support::{Reported, capture_reporter};
use gpui::WindowHandle;
use gpui::{AppContext as _, TestAppContext};
use inca_test_macros::gpui_cases;
use inca_window::test_support::TestRoot;
use std::time::Duration;

use super::support::*;

struct Scene {
    window: WindowHandle<TestRoot>,
    host: Rig,
    engine: Rc<inca_jsenv::Engine>,
    reported: Reported,
}

fn scene(cx: &mut TestAppContext) -> Scene {
    let host = host_on_executor_clock(cx);
    let executor = cx.executor();
    let engine = engine_on_clock(&host, Rc::new(move || executor.now()));
    let (report, reported) = capture_reporter();
    let window = mount_with_timers(cx, &host, &engine, report);
    engine.eval::<()>("globalThis.log = [];").unwrap();
    Scene {
        window,
        host,
        engine,
        reported,
    }
}

fn advance(cx: &mut TestAppContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn log(scene: &Scene) -> String {
    scene.engine.eval::<String>("log.join(',')").unwrap()
}

#[gpui::test]
fn a_throwing_timer_callback_reaches_the_reporter(cx: &mut TestAppContext) {
    let scene = scene(cx);
    scene
        .engine
        .eval::<()>(
            r"
                setTimeout(() => {
                    throw new Error('boom');
                }, 10);
                ",
        )
        .unwrap();

    advance(cx, 10);

    let messages: Vec<String> = scene
        .reported
        .borrow()
        .iter()
        .map(|err| err.message().to_owned())
        .collect();
    assert_eq!(messages, ["Error: boom"]);
}

#[gpui::test]
fn a_listener_error_raised_by_a_callback_is_reported_after_its_error(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);
    let (report, reported) = capture_reporter();
    let dispatcher = host.dispatcher(&engine, report);
    engine
        .eval::<()>(
            r"
                const n = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), n);
                __inca_test_listen__(n, 'x', () => {
                    throw new Error('listener');
                });
                setTimeout(() => {
                    __inca_native__.dispatchEvent(n, new Event('x'));
                    throw new Error('callback');
                }, 10);
                ",
        )
        .unwrap();
    let window = cx.add_empty_window();
    window.update(|window, cx| dispatcher.start_timers(window.window_handle(), cx));

    window.executor().advance_clock(Duration::from_millis(10));
    window.run_until_parked();

    let messages: Vec<String> = reported
        .borrow()
        .iter()
        .map(|err| err.message().to_owned())
        .collect();
    assert_eq!(messages, ["Error: callback", "Error: listener"]);
}

#[gpui::test]
fn a_timer_queued_before_start_timers_fires_on_the_first_turn(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);
    engine
        .eval::<()>(
            r"
                globalThis.log = [];
                setTimeout(() => log.push('a'), 50);
                ",
        )
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(50));
    let (report, _) = capture_reporter();

    let _window = mount_with_timers(cx, &host, &engine, report);
    cx.run_until_parked();

    assert_eq!(engine.eval::<String>("log.join(',')").unwrap(), "a");
}

#[gpui::test]
fn a_timer_callback_sees_the_focus_the_window_applied(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);
    let dispatcher = host.dispatcher(&engine, inca_jsenv::stderr_reporter());
    let node = {
        let mut document = host.document.borrow_mut();
        let node = document.tree.create_node("div").unwrap();
        let root = document.root();
        document.tree.append_child(root, node).unwrap();
        document
            .tree
            .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
            .unwrap();
        node
    };
    engine
        .eval::<()>(
            r"
                globalThis.log = [];
                setTimeout(() => log.push(__inca_native__.focusedNode()), 10);
                ",
        )
        .unwrap();
    let window = cx.add_empty_window();
    window.update(|window, cx| {
        {
            host.cells.document.borrow_mut().focus.mark_tab_dirty(node);
            let _ = apply_focus(&host.cells, window, cx);
            host.cells.focus_handle(node).unwrap().focus(window, cx);
        }
        dispatcher.start_timers(window.window_handle(), cx);
    });

    window.executor().advance_clock(Duration::from_millis(10));
    window.run_until_parked();

    assert_eq!(
        engine.eval::<String>("log.join(',')").unwrap(),
        node.to_string()
    );
}

#[gpui::test]
fn run_timers_idle_runs_due_callbacks_without_a_window(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);
    let (report, reported) = capture_reporter();
    let dispatcher = host.dispatcher(&engine, report);
    engine
        .eval::<()>(
            r"
                globalThis.log = [];
                setTimeout(() => {
                    log.push('a');
                    throw new Error('boom');
                }, 10);
                ",
        )
        .unwrap();

    cx.executor().advance_clock(Duration::from_millis(10));
    dispatcher.run_timers_idle();

    assert_eq!(engine.eval::<String>("log.join(',')").unwrap(), "a");
    assert_eq!(reported.borrow().len(), 1);
}

#[gpui::test]
fn a_native_stop_removes_the_entry(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);

    engine
        .eval::<()>("clearTimeout(setTimeout(() => {}, 10));")
        .unwrap();

    assert!(host.js.borrow().timers.next_due().is_none());
}

#[gpui::test]
fn a_turn_with_no_due_timer_leaves_queued_jobs_alone(cx: &mut TestAppContext) {
    let scene = scene(cx);
    scene
        .engine
        .eval::<()>("globalThis.id = setTimeout(() => {}, 10);")
        .unwrap();
    cx.run_until_parked();
    scene
        .engine
        .eval::<()>(
            r"
                clearTimeout(id);
                Promise.resolve().then(() => log.push('job'));
                ",
        )
        .unwrap();

    advance(cx, 10);

    assert_eq!(log(&scene), "");
}

#[gpui::test]
fn a_due_timer_whose_handler_returns_reports_nothing(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);
    let (report, reported) = capture_reporter();
    let _window = mount_with_timers(cx, &host, &engine, report);
    engine.eval::<()>("setTimeout(() => {}, 10);").unwrap();

    advance(cx, 10);

    assert!(reported.borrow().is_empty());
}

#[gpui::test]
fn a_frame_callback_that_requests_another_gets_the_next_frame(cx: &mut TestAppContext) {
    let scene = scene(cx);
    scene
        .engine
        .eval::<()>(
            r"
                requestAnimationFrame(() => {
                    log.push('a');
                    requestAnimationFrame(() => log.push('b'));
                });
                ",
        )
        .unwrap();

    assert_eq!(next_frame(cx, &scene), 1);
    assert_eq!(log(&scene), "a");
    assert_eq!(next_frame(cx, &scene), 1);

    assert_eq!(log(&scene), "a,b");
}

#[gpui::test]
fn a_frame_callback_write_is_drawn_in_the_frame_that_ran_it(cx: &mut TestAppContext) {
    let scene = scene(cx);
    scene
        .engine
        .eval::<()>(
            r"
                requestAnimationFrame(() => {
                    const n = __inca_native__;
                    const box = n.createNode('div');
                    n.setStyle(box, 'width', 40);
                    n.setStyle(box, 'height', 20);
                    n.appendChild(n.rootNodeId(), box);
                    globalThis.box = box;
                });
                ",
        )
        .unwrap();

    next_frame(cx, &scene);

    let node = scene.engine.eval::<u32>("box").unwrap();
    assert!(scene.host.cells.window.borrow().layout_of(node).is_some());
}

#[gpui::test]
fn a_frame_callback_requested_before_the_window_exists_runs_in_the_first_frame(
    cx: &mut TestAppContext,
) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);
    engine
        .eval::<()>(
            r"
                globalThis.log = [];
                requestAnimationFrame(() => log.push('a'));
                ",
        )
        .unwrap();
    let (report, reported) = capture_reporter();
    let window = mount_with_timers(cx, &host, &engine, report);
    let scene = Scene {
        window,
        host,
        engine,
        reported,
    };

    assert_eq!(next_frame(cx, &scene), 1);

    assert_eq!(log(&scene), "a");
}

#[gpui::test]
fn dropping_the_session_frees_the_engine(cx: &mut TestAppContext) {
    let Scene {
        window,
        host,
        engine,
        reported,
    } = scene(cx);
    engine.eval::<()>("setTimeout(() => {}, 10);").unwrap();
    cx.run_until_parked();
    let weak = Rc::downgrade(&engine);

    cx.update_window(window.into(), |_, window, _| window.remove_window())
        .unwrap();
    drop((engine, host, reported));
    cx.run_until_parked();

    assert!(weak.upgrade().is_none());
}

/// Delivers the next frame to the window of `scene`: the callbacks GPUI queued. The test
/// platform draws a window the callbacks invalidated. Returns how many callbacks ran.
fn next_frame(cx: &mut TestAppContext, scene: &Scene) -> usize {
    cx.run_until_parked();
    scene
        .window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap()
}
