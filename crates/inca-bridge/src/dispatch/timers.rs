// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs the timers a script started when they fall due and its frame callbacks in the next frame.

use std::rc::{Rc, Weak};
use std::time::Duration;

use gpui::{AnyWindowHandle, App, FutureExt as _, Window};

use super::{EventDispatcher, State, drain_jobs_and_refresh, observe_focus};
use crate::bindings::Host;
use crate::script::Script;

impl EventDispatcher {
    /// Starts the task that runs the timers of this dispatcher's engine in
    /// `window` when they fall due, and asks `window` for a frame while a
    /// frame callback is pending. Call it once the window exists.
    pub fn start_timers(&self, window: AnyWindowHandle, cx: &mut App) {
        self.state.frame_requested.set(false);
        let script = Rc::downgrade(&self.script);
        let host = Rc::downgrade(&self.host);
        let state = Rc::downgrade(&self.state);
        let wake = self.host.window.borrow().wake.clone();
        // A foreground task on the main thread. It sleeps on the wake channel and one executor
        // timer until the earliest timer is due.
        let task = cx.spawn(async move |cx| {
            loop {
                let (wait, wants_frame) = match (host.upgrade(), state.upgrade()) {
                    (Some(host), Some(state)) => {
                        let host = host.js.borrow();
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
                            request_frame(window, &script, &host, &state);
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
                                let (Some(script), Some(host)) = (script.upgrade(), host.upgrade())
                                else {
                                    return;
                                };
                                fire_timers(&*script, &host, window, cx);
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
        run_frame_callbacks(&*self.script, &self.host, &self.state, window, cx);
    }

    /// Ends the task [`start_timers`](Self::start_timers) started, so no timer
    /// of this dispatcher's engine runs again.
    pub fn stop_timers(&self) {
        self.state.timer_pump.borrow_mut().take();
    }

    /// Runs the callbacks that are due now, for a session that has no window
    /// yet, and reports their failures.
    pub fn run_timers_idle(&self) {
        self.script.run_timers();
        self.report_pending();
    }

    /// The time left until the earliest timer is due, or `None` with no timer.
    #[must_use]
    pub fn next_timer_wait(&self) -> Option<Duration> {
        self.host.js.borrow().timers.wait()
    }
}

/// Queues the next-frame callback that runs the frame callbacks of `host`. The callback holds
/// weak references, so a dropped session leaves it nothing to run.
fn request_frame(
    window: &Window,
    script: &Weak<dyn Script>,
    host: &Weak<Host>,
    state: &Weak<State>,
) {
    let Some(strong) = state.upgrade() else {
        return;
    };
    strong.frame_requested.set(true);
    let (script, host, state) = (Weak::clone(script), Weak::clone(host), Weak::clone(state));
    window.on_next_frame(move |window, cx| {
        let (Some(script), Some(host), Some(state)) =
            (script.upgrade(), host.upgrade(), state.upgrade())
        else {
            return;
        };
        run_frame_callbacks(&*script, &host, &state, window, cx);
    });
}

/// One frame: the callbacks pending now, then the redraw and the failures they caused. A frame
/// with no pending callback changes nothing.
fn run_frame_callbacks(
    script: &dyn Script,
    host: &Host,
    state: &State,
    window: &mut Window,
    cx: &App,
) {
    state.frame_requested.set(false);
    observe_focus(host, window, cx);
    if script.run_frames() == 0 {
        return;
    }
    drain_jobs_and_refresh(script, window);
    script.report_pending();
}

/// One turn: the due callbacks, then the redraw and the failures they caused.
/// A turn with no due callback changes nothing.
fn fire_timers(script: &dyn Script, host: &Host, window: &mut Window, cx: &App) {
    observe_focus(host, window, cx);
    if script.run_timers() == 0 {
        return;
    }
    drain_jobs_and_refresh(script, window);
    script.report_pending();
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::{
        Reported, TestRoot, capture_reporter, engine_for, engine_on_clock, host_on_executor_clock,
        mount_with_timers,
    };
    use crate::tests::{FakeScript, fake_dispatcher_on};
    use crate::{EventDispatcher, Host};
    use gpui::AppContext as _;
    use gpui::{TestAppContext, WindowHandle};

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    struct Scene {
        window: WindowHandle<TestRoot>,
        host: Rc<Host>,
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

    /// A window over a host on the executor clock whose script is a [`FakeScript`]. A turn of the
    /// fake runs the timers that are due and the frame callbacks that were requested.
    struct PumpScene {
        window: WindowHandle<TestRoot>,
        host: Rc<Host>,
        fake: Rc<FakeScript>,
        started: Rc<RefCell<Vec<(u32, std::time::Instant)>>>,
        requested: Rc<RefCell<Vec<u32>>>,
    }

    fn pump_scene(cx: &mut TestAppContext, start: bool) -> PumpScene {
        let host = host_on_executor_clock(cx);
        let (dispatcher, fake) = fake_dispatcher_on(&host);
        let started: Rc<RefCell<Vec<(u32, std::time::Instant)>>> = Rc::default();
        let requested: Rc<RefCell<Vec<u32>>> = Rc::default();
        {
            let (host, started) = (Rc::clone(&host), Rc::clone(&started));
            let executor = cx.executor();
            fake.on_timers(move || {
                let now = executor.now();
                let due: Vec<u32> = started
                    .borrow()
                    .iter()
                    .filter(|(_, at)| *at <= now)
                    .map(|(id, _)| *id)
                    .collect();
                started.borrow_mut().retain(|(_, at)| *at > now);
                for id in &due {
                    host.js.borrow().timers.stop(*id);
                }
                due.len()
            });
        }
        {
            let (host, requested) = (Rc::clone(&host), Rc::clone(&requested));
            fake.on_frames(move || {
                let handles = std::mem::take(&mut *requested.borrow_mut());
                for handle in &handles {
                    host.js.borrow().frames.cancel(*handle);
                }
                handles.len()
            });
        }
        let node = host.document.borrow().root();
        let window = cx.add_window(|_, _| TestRoot {
            host: Rc::clone(&host),
            node,
            dispatcher,
        });
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        if start {
            window
                .update(cx, |root, window, cx| {
                    root.dispatcher.start_timers(window.window_handle(), cx);
                })
                .unwrap();
        }
        PumpScene {
            window,
            host,
            fake,
            started,
            requested,
        }
    }

    impl PumpScene {
        /// Queues the timer `id` as a script's `setTimeout(.., ms)` does.
        fn set_timeout(&self, cx: &TestAppContext, id: u32, ms: u32) {
            let due = cx.executor().now() + Duration::from_millis(u64::from(ms));
            self.started.borrow_mut().push((id, due));
            self.host.js.borrow().timers.start(id, ms, false);
        }

        /// Queues a frame callback as a script's `requestAnimationFrame` does.
        fn request_frame(&self, id: u32) {
            let handle = self.host.js.borrow().frames.request(id).unwrap();
            self.requested.borrow_mut().push(handle);
        }

        fn timer_turns(&self) -> usize {
            self.fake.turns().0
        }
    }

    #[gpui::test]
    fn a_timer_started_while_the_pump_waits_with_an_empty_queue_runs_a_turn(
        cx: &mut TestAppContext,
    ) {
        let scene = pump_scene(cx, true);
        cx.run_until_parked();

        scene.set_timeout(cx, 1, 100);
        advance(cx, 99);
        assert_eq!(scene.timer_turns(), 0);
        advance(cx, 1);
        assert_eq!(scene.timer_turns(), 1);
    }

    #[gpui::test]
    fn a_shorter_timer_started_while_waiting_runs_its_turn_first(cx: &mut TestAppContext) {
        let scene = pump_scene(cx, true);
        scene.set_timeout(cx, 1, 10_000);
        cx.run_until_parked();

        scene.set_timeout(cx, 2, 10);
        advance(cx, 10);
        assert_eq!(scene.timer_turns(), 1);
        assert!(scene.host.js.borrow().timers.next_due().is_some());
        advance(cx, 9990);
        assert_eq!(scene.timer_turns(), 2);
        assert!(scene.host.js.borrow().timers.next_due().is_none());
    }

    #[gpui::test]
    fn closing_the_window_ends_the_pump(cx: &mut TestAppContext) {
        let scene = pump_scene(cx, true);
        scene.set_timeout(cx, 1, 10);
        cx.run_until_parked();

        let keep = scene
            .window
            .update(cx, |root, _, _| root.dispatcher.clone())
            .unwrap();
        cx.update_window(scene.window.into(), |_, window, _| window.remove_window())
            .unwrap();
        advance(cx, 10);

        assert_eq!(scene.timer_turns(), 0);
        assert_eq!(scene.host.window.borrow().wake.receiver_count(), 1);
        drop(keep);
    }

    #[gpui::test]
    fn stop_timers_ends_the_pump(cx: &mut TestAppContext) {
        let scene = pump_scene(cx, true);
        scene.set_timeout(cx, 1, 10);
        cx.run_until_parked();

        scene
            .window
            .update(cx, |root, _, _| root.dispatcher.stop_timers())
            .unwrap();
        advance(cx, 10);

        assert_eq!(scene.timer_turns(), 0);
    }

    #[gpui::test]
    fn without_start_timers_a_timer_never_runs_a_turn(cx: &mut TestAppContext) {
        let scene = pump_scene(cx, false);
        scene.set_timeout(cx, 1, 10);

        advance(cx, 10);

        assert_eq!(scene.timer_turns(), 0);
        assert!(scene.host.js.borrow().timers.next_due().is_some());
    }

    #[gpui::test]
    fn a_timer_callback_sees_the_focus_the_window_applied(cx: &mut TestAppContext) {
        let host = host_on_executor_clock(cx);
        let engine = engine_for(&host);
        let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
        let node = {
            let mut host = host.document.borrow_mut();
            let node = host.tree.create_node("div").unwrap();
            let root = host.root();
            host.tree.append_child(root, node).unwrap();
            host.tree
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
                let mut host = host.document.borrow_mut();
                let host = &mut *host;
                host.focus.mark_tab_dirty(node);
                let _ = host
                    .focus
                    .apply_pending(&host.tree, &host.style, window, cx);
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
        let scene = pump_scene(cx, true);
        let frame = |cx: &mut TestAppContext| {
            cx.run_until_parked();
            scene
                .window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap()
        };
        assert_eq!(frame(cx), 0);
        assert_eq!(scene.fake.turns().1, 0);
        scene.fake.queue_job(|| {});
        assert_eq!(frame(cx), 0);
        assert_eq!(scene.fake.turns().1, 0);
        scene.request_frame(1);
        cx.run_until_parked();

        assert_eq!(frame(cx), 1);
        assert_eq!(scene.fake.turns().1, 1);
        assert_eq!(frame(cx), 0);
        assert_eq!(scene.fake.turns().1, 1);
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
        assert!(
            scene
                .host
                .window
                .borrow()
                .layout
                .borrow()
                .contains_key(&node)
        );
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
