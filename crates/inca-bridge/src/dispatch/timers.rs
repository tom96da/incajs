// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs the timers a script started when they fall due.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{AnyWindowHandle, App, FutureExt as _, Window};
use inca_jsenv::{Engine, ErrorReporter, run_timers};
use rquickjs::{Ctx, Function, Result as JsResult};

use super::{EventDispatcher, drain_jobs_and_refresh, observe_focus, report_failures};
use crate::bindings::Host;

impl EventDispatcher {
    /// Starts the task that runs the timers of this dispatcher's engine in
    /// `window` when they fall due. Call it once the window exists.
    pub fn start_timers(&self, window: AnyWindowHandle, cx: &mut App) {
        let engine = Rc::downgrade(&self.engine);
        let host = Rc::downgrade(&self.host);
        let reporter = Rc::clone(&self.reporter);
        let wake = self.host.borrow().timer_wake.clone();
        let task = cx.spawn(async move |cx| {
            loop {
                let wait: Option<Duration> = match host.upgrade() {
                    Some(host) => host.borrow().timers.wait(),
                    None => break,
                };
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

    /// Ends the task [`start_timers`](Self::start_timers) started, so no timer
    /// of this dispatcher's engine runs again.
    pub fn stop_timers(&self) {
        self.state.timer_pump.borrow_mut().take();
    }

    /// Runs the callbacks that are due now, for a session that has no window
    /// yet, and reports their failures.
    pub fn run_timers_idle(&self) {
        let timers = Rc::clone(&self.host.borrow().timers);
        run_timers(&self.engine, &timers, &self.reporter, &call_timer_hook);
        self.report_pending();
    }

    /// The time left until the earliest timer is due, or `None` with no timer.
    #[must_use]
    pub fn next_timer_wait(&self) -> Option<Duration> {
        self.host.borrow().timers.wait()
    }
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
    if run_timers(engine, &timers, reporter, &call_timer_hook) == 0 {
        return;
    }
    drain_jobs_and_refresh(engine, reporter, window);
    report_failures(host, reporter);
}

/// Calls the global `__inca_timer__(id)`. A global that is not a function is
/// skipped.
fn call_timer_hook(ctx: &Ctx<'_>, id: u32) -> JsResult<()> {
    let Ok(hook) = ctx.globals().get::<_, Function>("__inca_timer__") else {
        return Ok(());
    };
    hook.call((id,))
}
