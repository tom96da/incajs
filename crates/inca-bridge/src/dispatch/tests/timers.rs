// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use std::time::Duration;

use super::*;
use crate::test_support::{TIMERS_JS, host_on_executor_clock, mount_with_timers};

struct Scene {
    window: WindowHandle<TestRoot>,
    host: Rc<RefCell<Host>>,
    engine: Rc<inca_jsenv::Engine>,
    reported: Reported,
}

fn scene(cx: &mut TestAppContext) -> Scene {
    let host = host_on_executor_clock(cx);
    let engine = timer_engine(&host);
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

/// [`engine_for`] with [`TIMERS_JS`] evaluated.
fn timer_engine(host: &Rc<RefCell<Host>>) -> Rc<inca_jsenv::Engine> {
    let engine = engine_for(host);
    engine.eval::<()>(TIMERS_JS).unwrap();
    engine
}

#[gpui::test]
fn a_timer_started_while_the_pump_waits_with_an_empty_queue_fires(cx: &mut TestAppContext) {
    let scene = scene(cx);
    cx.run_until_parked();

    scene
        .engine
        .eval::<()>("setTimeout(() => log.push('a'), 100);")
        .unwrap();
    advance(cx, 99);
    assert_eq!(log(&scene), "");
    advance(cx, 1);
    assert_eq!(log(&scene), "a");
}

#[gpui::test]
fn a_shorter_timer_started_while_waiting_fires_first(cx: &mut TestAppContext) {
    let scene = scene(cx);
    scene
        .engine
        .eval::<()>("setTimeout(() => log.push('long'), 10000);")
        .unwrap();
    cx.run_until_parked();

    scene
        .engine
        .eval::<()>("setTimeout(() => log.push('short'), 10);")
        .unwrap();
    advance(cx, 10);
    assert_eq!(log(&scene), "short");
    advance(cx, 9990);
    assert_eq!(log(&scene), "short,long");
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
    let engine = timer_engine(&host);
    let (report, reported) = capture_reporter();
    let dispatcher =
        EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host)).with_reporter(report);
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
    let engine = timer_engine(&host);
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
fn closing_the_window_ends_the_pump(cx: &mut TestAppContext) {
    let scene = scene(cx);
    scene
        .engine
        .eval::<()>("setTimeout(() => log.push('a'), 10);")
        .unwrap();
    cx.run_until_parked();

    let keep = scene
        .window
        .update(cx, |root, _, _| root.dispatcher.clone())
        .unwrap();
    cx.update_window(scene.window.into(), |_, window, _| window.remove_window())
        .unwrap();
    advance(cx, 10);

    assert_eq!(log(&scene), "");
    assert_eq!(scene.host.borrow().timer_wake.receiver_count(), 1);
    drop(keep);
}

#[gpui::test]
fn a_timer_callback_sees_the_focus_the_window_applied(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = timer_engine(&host);
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let node = {
        let mut host = host.borrow_mut();
        let node = host.tree.create_node("div").unwrap();
        let root = host.root();
        host.tree.append_child(root, node).unwrap();
        host.tree
            .set_attribute(node, "tabindex", inca_gpui::AttributeValue::from("0"))
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
            let mut host = host.borrow_mut();
            let host = &mut *host;
            host.focus.mark_tab_dirty(node);
            let _ = host.focus.apply_pending(&host.tree, window, cx);
            host.focus.handle(node).unwrap().focus(window, cx);
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

#[gpui::test]
fn stop_timers_ends_the_pump(cx: &mut TestAppContext) {
    let scene = scene(cx);
    scene
        .engine
        .eval::<()>("setTimeout(() => log.push('a'), 10);")
        .unwrap();
    cx.run_until_parked();

    scene
        .window
        .update(cx, |root, _, _| root.dispatcher.stop_timers())
        .unwrap();
    advance(cx, 10);

    assert_eq!(log(&scene), "");
}

#[gpui::test]
fn without_start_timers_a_timer_never_runs(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = timer_engine(&host);
    let root = host.borrow().root();
    let _window = mount_active(cx, &host, &engine, root);
    engine
        .eval::<()>(
            r"
            globalThis.log = [];
            setTimeout(() => log.push('a'), 10);
            ",
        )
        .unwrap();

    advance(cx, 10);

    assert_eq!(engine.eval::<String>("log.join(',')").unwrap(), "");
    assert!(host.borrow().timers.next_due().is_some());
}

#[gpui::test]
fn run_timers_idle_runs_due_callbacks_without_a_window(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = timer_engine(&host);
    let (report, reported) = capture_reporter();
    let dispatcher =
        EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host)).with_reporter(report);
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
    let engine = timer_engine(&host);

    engine
        .eval::<()>("clearTimeout(setTimeout(() => {}, 10));")
        .unwrap();

    assert!(host.borrow().timers.next_due().is_none());
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
fn a_due_timer_without_the_hook_is_skipped(cx: &mut TestAppContext) {
    let host = host_on_executor_clock(cx);
    let engine = engine_for(&host);
    let (report, reported) = capture_reporter();
    let _window = mount_with_timers(cx, &host, &engine, report);
    engine
        .eval::<()>("__inca_native__.startTimer(10, false);")
        .unwrap();

    advance(cx, 10);

    assert!(reported.borrow().is_empty());
}
