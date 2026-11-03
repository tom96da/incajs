// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs the timers a script started when they fall due and its frame callbacks in the next frame.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gpui::{AnyWindowHandle, App, FutureExt as _, Window};
use inca_jsenv::{Engine, ErrorReporter, run_frames, run_timers};

use super::{EventDispatcher, State, drain_jobs_and_refresh, observe_focus, report_failures};
use crate::bindings::Host;

impl EventDispatcher {
    /// Starts the task that runs the timers of this dispatcher's engine in
    /// `window` when they fall due, and asks `window` for a frame while a
    /// frame callback is pending. Call it once the window exists.
    pub fn start_timers(&self, window: AnyWindowHandle, cx: &mut App) {
        self.state.frame_requested.set(false);
        let engine = Rc::downgrade(&self.engine);
        let host = Rc::downgrade(&self.host);
        let state = Rc::downgrade(&self.state);
        let reporter = Rc::clone(&self.reporter);
        let wake = self.host.borrow().wake.clone();
        let task = cx.spawn(async move |cx| {
            loop {
                let (wait, wants_frame) = match (host.upgrade(), state.upgrade()) {
                    (Some(host), Some(state)) => {
                        let host = host.borrow();
                        (
                            host.timers.wait(),
                            host.frames.is_pending() && !state.frame_requested.get(),
                        )
                    }
                    _ => break,
                };
                if wants_frame {
                    let live = cx.update(|cx| {
                        window.update(cx, |_, window, _| {
                            request_frame(window, &engine, &host, &reporter, &state);
                        })
                    });
                    if live.is_err() {
                        break;
                    }
                }
                let waited = match wait {
                    Some(wait) => wake
                        .recv()
                        .with_timeout(wait, cx.background_executor())
                        .await
                        .ok(),
                    None => Some(wake.recv().await),
                };
                match waited {
                    Some(Ok(())) => {}
                    // The sender is gone only with the host.
                    Some(Err(_)) => break,
                    None => {
                        let live = cx.update(|cx| {
                            window.update(cx, |_, window, cx| {
                                let (Some(engine), Some(host)) = (engine.upgrade(), host.upgrade())
                                else {
                                    return;
                                };
                                fire_timers(&engine, &host, &reporter, window, cx);
                            })
                        });
                        if live.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        *self.state.timer_pump.borrow_mut() = Some(task);
    }

    /// Runs the frame callbacks the script requested, as the next frame of
    /// `window` does before it draws, and redraws when one ran.
    pub fn run_frame_callbacks(&self, window: &mut Window, cx: &App) {
        run_frame_callbacks(
            &self.engine,
            &self.host,
            &self.reporter,
            &self.state,
            window,
            cx,
        );
    }

    /// Ends the task [`start_timers`](Self::start_timers) started, so no timer
    /// of this dispatcher's engine runs again.
    pub fn stop_timers(&self) {
        self.state.timer_pump.borrow_mut().take();
    }

    /// Runs the callbacks that are due now, for a session that has no window
    /// yet, and reports their failures.
    pub fn run_timers_idle(&self) {
        let timers = Rc::clone(&self.host.borrow().timers);
        run_timers(&self.engine, &timers, &self.reporter);
        self.report_pending();
    }

    /// The time left until the earliest timer is due, or `None` with no timer.
    #[must_use]
    pub fn next_timer_wait(&self) -> Option<Duration> {
        self.host.borrow().timers.wait()
    }
}

/// Queues the next-frame callback that runs the frame callbacks of `host`. The callback holds
/// weak references, so a dropped session leaves it nothing to run.
fn request_frame(
    window: &Window,
    engine: &Weak<Engine>,
    host: &Weak<RefCell<Host>>,
    reporter: &ErrorReporter,
    state: &Weak<State>,
) {
    let Some(strong) = state.upgrade() else {
        return;
    };
    strong.frame_requested.set(true);
    let (engine, host, reporter, state) = (
        Weak::clone(engine),
        Weak::clone(host),
        Rc::clone(reporter),
        Weak::clone(state),
    );
    window.on_next_frame(move |window, cx| {
        let (Some(engine), Some(host), Some(state)) =
            (engine.upgrade(), host.upgrade(), state.upgrade())
        else {
            return;
        };
        run_frame_callbacks(&engine, &host, &reporter, &state, window, cx);
    });
}

/// One frame: the callbacks pending now, then the redraw and the failures they caused. A frame
/// with no pending callback changes nothing.
fn run_frame_callbacks(
    engine: &Engine,
    host: &RefCell<Host>,
    reporter: &ErrorReporter,
    state: &State,
    window: &mut Window,
    cx: &App,
) {
    state.frame_requested.set(false);
    observe_focus(host, window, cx);
    let frames = Rc::clone(&host.borrow().frames);
    if run_frames(engine, &frames, reporter) == 0 {
        return;
    }
    drain_jobs_and_refresh(engine, reporter, window);
    report_failures(host, reporter);
}

/// One turn: the due callbacks, then the redraw and the failures they caused.
/// A turn with no due callback changes nothing.
fn fire_timers(
    engine: &Engine,
    host: &RefCell<Host>,
    reporter: &ErrorReporter,
    window: &mut Window,
    cx: &App,
) {
    observe_focus(host, window, cx);
    let timers = Rc::clone(&host.borrow().timers);
    if run_timers(engine, &timers, reporter) == 0 {
        return;
    }
    drain_jobs_and_refresh(engine, reporter, window);
    report_failures(host, reporter);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{
        Reported, TestRoot, capture_reporter, engine_for, engine_on_clock, host_on_executor_clock,
        mount_active, mount_with_timers,
    };
    use crate::{EventDispatcher, Host};
    use gpui::AppContext as _;
    use gpui::{TestAppContext, WindowHandle};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    struct Scene {
        window: WindowHandle<TestRoot>,
        host: Rc<RefCell<Host>>,
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
        let engine = engine_for(&host);
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
        assert_eq!(scene.host.borrow().wake.receiver_count(), 1);
        drop(keep);
    }

    #[gpui::test]
    fn a_timer_callback_sees_the_focus_the_window_applied(cx: &mut TestAppContext) {
        let host = host_on_executor_clock(cx);
        let engine = engine_for(&host);
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
        let engine = engine_for(&host);
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
        let engine = engine_for(&host);
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
        let engine = engine_for(&host);

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
    fn a_due_timer_whose_handler_returns_reports_nothing(cx: &mut TestAppContext) {
        let host = host_on_executor_clock(cx);
        let engine = engine_for(&host);
        let (report, reported) = capture_reporter();
        let _window = mount_with_timers(cx, &host, &engine, report);
        engine.eval::<()>("setTimeout(() => {}, 10);").unwrap();

        advance(cx, 10);

        assert!(reported.borrow().is_empty());
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

    #[gpui::test]
    fn a_frame_is_requested_only_while_a_callback_is_pending(cx: &mut TestAppContext) {
        let scene = scene(cx);
        assert_eq!(next_frame(cx, &scene), 0);
        scene
            .engine
            .eval::<()>("requestAnimationFrame(() => log.push('a'));")
            .unwrap();
        cx.run_until_parked();

        assert_eq!(next_frame(cx, &scene), 1);
        scene
            .engine
            .eval::<()>("Promise.resolve().then(() => log.push('job'));")
            .unwrap();

        assert_eq!(next_frame(cx, &scene), 0);
        assert_eq!(log(&scene), "a");
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
        assert!(scene.host.borrow().layout.borrow().contains_key(&node));
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
}
