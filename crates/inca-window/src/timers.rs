// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs the timers a script started when they fall due and its frame callbacks in the next frame.

use std::rc::{Rc, Weak};
use std::time::Duration;

use gpui::{AnyWindowHandle, App, FutureExt as _, Window};

use crate::dispatch::{EventDispatcher, State, drain_jobs_and_refresh};
use crate::script::Script;
use crate::view::Cells;

impl EventDispatcher {
    /// Starts the task that runs the timers of this dispatcher's script in
    /// `window` when they fall due, and asks `window` for a frame while a
    /// frame callback is pending. Call it once the window exists.
    pub fn start_timers(&self, window: AnyWindowHandle, cx: &mut App) {
        self.state.frame_requested.set(false);
        let script = Rc::downgrade(&self.script);
        let cells = Rc::downgrade(&self.cells);
        let state = Rc::downgrade(&self.state);
        let wake = self.cells.window.borrow().wake.clone();
        // A foreground task on the main thread. It sleeps on the wake channel and one executor
        // timer until the earliest timer is due.
        let task = cx.spawn(async move |cx| {
            loop {
                let (wait, wants_frame) = match (script.upgrade(), state.upgrade()) {
                    (Some(script), Some(state)) => (
                        script.next_timer_wait(),
                        script.frames_pending() && !state.frame_requested.get(),
                    ),
                    _ => break,
                };
                if wants_frame {
                    let live = cx.update(|cx| {
                        window.update(cx, |_, window, _| {
                            request_frame(window, &script, &cells, &state);
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
                    // The sender lives in the timers and frames of the script side. It is gone
                    // when they are.
                    Some(Err(_)) => break,
                    None => {
                        let live = cx.update(|cx| {
                            window.update(cx, |_, window, cx| {
                                let (Some(script), Some(cells), Some(state)) =
                                    (script.upgrade(), cells.upgrade(), state.upgrade())
                                else {
                                    return;
                                };
                                EventDispatcher {
                                    script,
                                    cells,
                                    state,
                                }
                                .fire_timers(window, cx);
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
    pub fn run_frame_callbacks(&self, window: &mut Window, cx: &mut App) {
        self.state.frame_requested.set(false);
        self.turn(window, cx, |window, _| {
            if self.script.run_frames() == 0 {
                return;
            }
            drain_jobs_and_refresh(&*self.script, window);
            self.script.report_pending();
        });
    }

    /// One turn: the due callbacks, then the redraw and the failures they caused.
    /// A turn with no due callback changes nothing.
    fn fire_timers(&self, window: &mut Window, cx: &mut App) {
        self.turn(window, cx, |window, _| {
            if self.script.run_timers() == 0 {
                return;
            }
            drain_jobs_and_refresh(&*self.script, window);
            self.script.report_pending();
        });
    }

    /// Ends the task [`start_timers`](Self::start_timers) started, so no timer
    /// of this dispatcher's script runs again.
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
        self.script.next_timer_wait()
    }
}

/// Queues the next-frame callback that runs the frame callbacks of `script`. The callback holds
/// weak references, so a dropped session leaves it nothing to run.
fn request_frame(
    window: &Window,
    script: &Weak<dyn Script>,
    cells: &Weak<Cells>,
    state: &Weak<State>,
) {
    let Some(strong) = state.upgrade() else {
        return;
    };
    strong.frame_requested.set(true);
    let (script, cells, state) = (Weak::clone(script), Weak::clone(cells), Weak::clone(state));
    window.on_next_frame(move |window, cx| {
        let (Some(script), Some(cells), Some(state)) =
            (script.upgrade(), cells.upgrade(), state.upgrade())
        else {
            return;
        };
        EventDispatcher {
            script,
            cells,
            state,
        }
        .run_frame_callbacks(window, cx);
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::TestRoot;
    use crate::tests::{FakeScript, fake_dispatcher_on};
    use crate::{Cells, test_support};
    use gpui::AppContext as _;
    use gpui::{TestAppContext, WindowHandle};

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    fn advance(cx: &mut TestAppContext, ms: u64) {
        cx.executor().advance_clock(Duration::from_millis(ms));
        cx.run_until_parked();
    }

    /// A window over cells on a script that is a [`FakeScript`]. A turn of the fake runs the
    /// timers that are due on the executor clock and the frame callbacks that were requested.
    struct PumpScene {
        window: WindowHandle<TestRoot>,
        cells: Rc<Cells>,
        fake: Rc<FakeScript>,
        started: Rc<RefCell<Vec<Instant>>>,
        requested: Rc<RefCell<Vec<u32>>>,
    }

    fn pump_scene(cx: &mut TestAppContext, start: bool) -> PumpScene {
        let (cells, wake) = test_support::cells();
        let (dispatcher, fake) = fake_dispatcher_on(&cells);
        fake.set_wake(wake);
        let started: Rc<RefCell<Vec<Instant>>> = Rc::default();
        let requested: Rc<RefCell<Vec<u32>>> = Rc::default();
        {
            let (started, executor) = (Rc::clone(&started), cx.executor());
            fake.on_timers(move || {
                let now = executor.now();
                let before = started.borrow().len();
                started.borrow_mut().retain(|at| *at > now);
                before - started.borrow().len()
            });
        }
        {
            let (started, executor) = (Rc::clone(&started), cx.executor());
            fake.on_timer_wait(move || {
                let now = executor.now();
                let next = started.borrow().iter().min().copied()?;
                Some(next.saturating_duration_since(now))
            });
        }
        {
            let requested = Rc::clone(&requested);
            fake.on_frames(move || std::mem::take(&mut *requested.borrow_mut()).len());
        }
        {
            let requested = Rc::clone(&requested);
            fake.on_frames_pending(move || !requested.borrow().is_empty());
        }
        let node = cells.document.borrow().root();
        let window = cx.add_window(|_, _| TestRoot {
            cells: Rc::clone(&cells),
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
            cells,
            fake,
            started,
            requested,
        }
    }

    impl PumpScene {
        /// Queues a timer due `ms` from now, as a script's `setTimeout(.., ms)` does.
        fn set_timeout(&self, cx: &TestAppContext, ms: u32) {
            let due = cx.executor().now() + Duration::from_millis(u64::from(ms));
            self.started.borrow_mut().push(due);
            self.fake.wake();
        }

        /// Queues a frame callback as a script's `requestAnimationFrame` does.
        fn request_frame(&self, id: u32) {
            self.requested.borrow_mut().push(id);
            self.fake.wake();
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

        scene.set_timeout(cx, 100);
        advance(cx, 99);
        assert_eq!(scene.timer_turns(), 0);
        advance(cx, 1);
        assert_eq!(scene.timer_turns(), 1);
    }

    #[gpui::test]
    fn a_shorter_timer_started_while_waiting_runs_its_turn_first(cx: &mut TestAppContext) {
        let scene = pump_scene(cx, true);
        scene.set_timeout(cx, 10_000);
        cx.run_until_parked();

        scene.set_timeout(cx, 10);
        advance(cx, 10);
        assert_eq!(scene.timer_turns(), 1);
        assert!(!scene.started.borrow().is_empty());
        advance(cx, 9990);
        assert_eq!(scene.timer_turns(), 2);
        assert!(scene.started.borrow().is_empty());
    }

    #[gpui::test]
    fn closing_the_window_ends_the_pump(cx: &mut TestAppContext) {
        let scene = pump_scene(cx, true);
        scene.set_timeout(cx, 10);
        cx.run_until_parked();

        let keep = scene
            .window
            .update(cx, |root, _, _| root.dispatcher.clone())
            .unwrap();
        cx.update_window(scene.window.into(), |_, window, _| window.remove_window())
            .unwrap();
        advance(cx, 10);

        assert_eq!(scene.timer_turns(), 0);
        assert_eq!(scene.cells.window.borrow().wake.receiver_count(), 1);
        drop(keep);
    }

    #[gpui::test]
    fn stop_timers_ends_the_pump(cx: &mut TestAppContext) {
        let scene = pump_scene(cx, true);
        scene.set_timeout(cx, 10);
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
        scene.set_timeout(cx, 10);

        advance(cx, 10);

        assert_eq!(scene.timer_turns(), 0);
        assert!(!scene.started.borrow().is_empty());
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
}
