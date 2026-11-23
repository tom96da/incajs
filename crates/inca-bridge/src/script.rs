// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The turns of the engine a window asks for: a walk, the due timers, the frame callbacks, the
//! released-node hook and the job queue. [`JsScript`] runs them in a `QuickJS` [`Engine`].

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use inca_document::{Outcome, Released, Walk};
use inca_jsenv::{
    Engine, EngineError, ErrorReporter, callbacks, report_jobs, run_frames, run_timers,
};
use inca_node::NodeId;

use crate::bindings::Host;
use crate::event;

/// The turns of a `QuickJS` engine and the host its bindings write to. A method that runs JS files
/// what the script threw in `JsState::failures`, and [`report_pending`](Self::report_pending)
/// sends it on. Call every method outside any [`Engine::with`] and with no borrow of the host
/// held.
///
/// # Where the real JS function lives
///
/// The listener store keeps a plain `u32` callback id with its capture and passive flags per
/// registration, never an `rquickjs::Value`, `Function` or `Persistent<T>`, because a JS handle
/// kept past the call that produced it outlives the context it belongs to. `addEventListener`
/// stores the function in the callback table of the engine and returns the id it got there. The
/// walk of an event looks the function up inside one `Engine::with` call and drops it before that
/// call returns.
pub struct JsScript {
    engine: Rc<Engine>,
    host: Rc<Host>,
    reporter: RefCell<ErrorReporter>,
}

impl JsScript {
    #[must_use]
    pub fn new(engine: Rc<Engine>, host: Rc<Host>, reporter: ErrorReporter) -> Self {
        Self {
            engine,
            host,
            reporter: RefCell::new(reporter),
        }
    }

    fn reporter(&self) -> ErrorReporter {
        Rc::clone(&self.reporter.borrow())
    }

    /// Runs the listeners of `walk` and reports whether one canceled the event and whether the
    /// walk entered the engine.
    #[must_use]
    pub fn walk(&self, walk: &Walk<'_>) -> Outcome {
        event::walk(&self.engine, &self.host, walk)
    }

    /// Runs the timers that are due now and returns how many ran.
    pub fn run_timers(&self) -> usize {
        let timers = Rc::clone(&self.host.js.borrow().timers);
        run_timers(&self.engine, &timers, &self.reporter())
    }

    /// Runs the frame callbacks pending now and returns how many ran.
    pub fn run_frames(&self) -> usize {
        let frames = Rc::clone(&self.host.js.borrow().frames);
        run_frames(&self.engine, &frames, &self.reporter())
    }

    /// Tells the script that `nodes` were freed with the listeners `released` names, ends the
    /// holds of the signals it names, and returns whether it entered the engine.
    pub fn release(&self, nodes: &[NodeId], released: &Released) -> bool {
        let host = &self.host;
        let callback_ids = &released.callbacks;
        self.engine.with(|ctx| {
            if let Err(err) = crate::abort::unpin(&ctx, &released.unpin) {
                host.js
                    .borrow_mut()
                    .failures
                    .push(EngineError::capture(&ctx, &err));
            }
            if let Err(err) = callbacks::release(&ctx, callback_ids) {
                host.js
                    .borrow_mut()
                    .failures
                    .push(EngineError::capture(&ctx, &err));
            }
            notify_released(&ctx, host, nodes, callback_ids)
        })
    }

    /// Runs the queued jobs, and reports what they threw.
    pub fn drain_jobs(&self) {
        report_jobs(&self.engine, &self.reporter());
    }

    /// Sends the listener errors collected so far to the reporter, in order.
    pub fn report_pending(&self) {
        let pending = std::mem::take(&mut self.host.js.borrow_mut().failures);
        let reporter = self.reporter();
        for failure in &pending {
            reporter(failure);
        }
    }

    /// Sends errors to `reporter` from now on.
    pub fn set_reporter(&self, reporter: ErrorReporter) {
        *self.reporter.borrow_mut() = reporter;
    }

    /// The time left until the earliest timer is due, or `None` with no timer.
    #[must_use]
    pub fn next_timer_wait(&self) -> Option<Duration> {
        self.host.js.borrow().timers.wait()
    }

    /// Whether a frame callback waits for the next frame.
    #[must_use]
    pub fn frames_pending(&self) -> bool {
        self.host.js.borrow().frames.is_pending()
    }
}

/// Calls the global `__inca_released__(nodeIds, callbackIds)` and files a throw in
/// `JsState::failures`. The global is looked up per call and skipped when it is not a function.
/// Returns whether it was a function.
pub(crate) fn notify_released(
    ctx: &rquickjs::Ctx<'_>,
    host: &Host,
    nodes: &[NodeId],
    callback_ids: &[u32],
) -> bool {
    let Ok(hook) = ctx
        .globals()
        .get::<_, rquickjs::Function>("__inca_released__")
    else {
        return false;
    };
    let called = event::array_of(ctx, nodes.iter().copied()).and_then(|nodes| {
        let callback_ids = event::array_of(ctx, callback_ids.iter().copied())?;
        hook.call::<_, ()>((nodes, callback_ids))
    });
    if let Err(err) = called {
        host.js
            .borrow_mut()
            .failures
            .push(EngineError::capture(ctx, &err));
    }
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::test_support::{capture_reporter, engine_for};
    use inca_jsenv::stderr_reporter;

    #[test]
    fn pending_failures_go_to_the_reporter_set_last() {
        let host = Rc::new(Host::default());
        let engine = engine_for(&host);
        let failure = engine.eval::<()>("throw new Error('boom');").unwrap_err();
        let script = JsScript::new(engine, Rc::clone(&host), stderr_reporter());
        let (report, reported) = capture_reporter();
        script.set_reporter(report);
        host.js.borrow_mut().failures.push(failure.clone());

        script.report_pending();

        assert_eq!(*reported.borrow(), vec![failure]);
        assert!(host.js.borrow().failures.is_empty());
    }
}
