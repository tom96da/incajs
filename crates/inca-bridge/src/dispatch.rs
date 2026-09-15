// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dispatches a native input event into the JS callbacks registered for it
//! via `__inca_native__.addEventListener` (`crate::bindings`), then
//! requests a redraw.
//!
//! Zero-overhead by construction: [`EventDispatcher::dispatch`] is the only
//! thing that ever touches the JS engine or calls
//! [`Window::refresh`](gpui::Window::refresh) on this path, so a re-render
//! with no new input never runs JS.
//!
//! ## Where the real JS function lives
//!
//! `EventListeners` (`crate::bindings`) only ever stores a plain `u32`
//! callback id, never an `rquickjs::Value`/`Function`/`Persistent<T>`: a JS
//! handle kept past the call that produced it outlives the context it
//! belongs to. The actual function has to live somewhere, so the convention
//! is:
//! the JS caller stores it itself, at
//! `globalThis.__inca_callbacks__[callbackId]`, before calling
//! `addEventListener` with that id. [`EventDispatcher::dispatch`] looks the
//! real function up fresh inside one `Engine::with` call and drops it
//! before that call returns — it never crosses into Rust-held state.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::Window;
use inca_gpui::NodeId;
use inca_jsenv::{Engine, EngineError};
use rquickjs::{Function, Object};

use inca_gpui::EventSink;

use crate::bindings::Host;

/// Where a failure inside the running app goes. Nothing asked for it, so
/// there is no caller to return it to and no `Result` to put it in.
pub type ErrorReporter = Rc<dyn Fn(&EngineError)>;

/// The reporter a dispatcher uses unless given another: one line per failure
/// on stderr.
///
/// stderr rather than stdout because an embedder may be using stdout as a
/// message channel, and because a failure that reaches nobody is the reason
/// this exists at all.
#[must_use]
pub fn stderr_reporter() -> ErrorReporter {
    Rc::new(|err| eprintln!("{err}"))
}

/// Everything needed to dispatch a native event into JS: the engine to call
/// into, and the registry of which JS callback ids are listening for which
/// `(node id, event name)`.
#[derive(Clone)]
pub struct EventDispatcher {
    engine: Rc<Engine>,
    host: Rc<RefCell<Host>>,
    reporter: ErrorReporter,
}

impl EventDispatcher {
    /// Reports failures through [`stderr_reporter`].
    pub fn new(engine: Rc<Engine>, host: Rc<RefCell<Host>>) -> Self {
        Self {
            engine,
            host,
            reporter: stderr_reporter(),
        }
    }

    /// Sends failures to `reporter` instead. A host with a channel to its
    /// parent reports there; nothing else about dispatch changes.
    #[must_use]
    pub fn with_reporter(mut self, reporter: ErrorReporter) -> Self {
        self.reporter = reporter;
        self
    }

    /// Whether anything is registered for `(node_id, event)`. The render
    /// path asks before wiring an element for input.
    #[must_use]
    pub fn listens(&self, node_id: NodeId, event: &str) -> bool {
        !self
            .host
            .borrow()
            .listeners
            .callbacks_for(node_id, event)
            .is_empty()
    }

    /// Calls every JS callback registered for `(node_id, event)` (via
    /// `__inca_native__.addEventListener`), passing `node_id`, then drains
    /// the job queue and requests a redraw.
    ///
    /// Never panics. A callback that throws is reported and the rest still
    /// run — one bad listener must not take the host down, nor stop its
    /// siblings. A missing `__inca_callbacks__` registry, or an entry that
    /// is missing or isn't a function, is a stale id and is skipped.
    ///
    /// The drain happens whether or not a listener ran: a job queued earlier
    /// is still owed a turn, and whether this particular click had a
    /// listener says nothing about that.
    pub fn dispatch(&self, node_id: NodeId, event: &str, window: &mut Window) {
        let callback_ids = self
            .host
            .borrow()
            .listeners
            .callbacks_for(node_id, event)
            .to_vec();

        // Collected rather than reported in place: a reporter is free to do
        // anything, and re-entering the engine from inside `with` panics.
        let failures = self.engine.with(|ctx| {
            let mut failures = Vec::new();
            let Ok(callbacks) = ctx.globals().get::<_, Object>("__inca_callbacks__") else {
                return failures;
            };
            for callback_id in callback_ids {
                if let Ok(callback) = callbacks.get::<_, Function>(callback_id)
                    && let Err(err) = callback.call::<_, ()>((node_id,))
                {
                    failures.push(EngineError::capture(&ctx, &err));
                }
            }
            failures
        });
        for failure in &failures {
            (self.reporter)(failure);
        }

        drain_jobs_and_refresh(&self.engine, window);
    }
}

impl EventSink for EventDispatcher {
    fn listens(&self, node_id: NodeId, event: &str) -> bool {
        self.listens(node_id, event)
    }

    fn dispatch(&self, node_id: NodeId, event: &str, window: &mut Window) {
        self.dispatch(node_id, event, window);
    }
}

/// Drains `QuickJS`'s pending-job queue, then requests a redraw. Code that
/// only schedules work (a `@vue/runtime-core` reactivity effect, batched via
/// a microtask) hasn't mutated the tree once the scheduling call returns, so
/// the queue has to run before the frame is drawn.
///
/// Call it outside any [`Engine::with`] — it takes the context itself, and
/// nesting that panics with "`RefCell` already borrowed".
pub fn drain_jobs_and_refresh(engine: &Engine, window: &mut Window) {
    engine.with(|ctx| while ctx.execute_pending_job() {});
    window.refresh();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bindings::install;
    use gpui::TestAppContext;

    /// Every failure the dispatcher reported, in order.
    type Reported = Rc<RefCell<Vec<EngineError>>>;

    fn dispatcher_with_engine() -> (EventDispatcher, Rc<RefCell<Host>>, Reported) {
        let engine = Rc::new(Engine::new().unwrap());
        let host = Rc::new(RefCell::new(Host::default()));
        engine.with(|ctx| install(&ctx, &host)).unwrap();

        let reported: Reported = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&reported);
        let dispatcher = EventDispatcher::new(engine, Rc::clone(&host)).with_reporter(Rc::new(
            move |err: &EngineError| sink.borrow_mut().push(err.clone()),
        ));

        (dispatcher, host, reported)
    }

    #[gpui::test]
    fn no_listener_registered_reports_nothing(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div");

        let cx = cx.add_empty_window();
        cx.update(|window, _| dispatcher.dispatch(node_id, "click", window));

        assert!(reported.borrow().is_empty());
    }

    #[gpui::test]
    fn a_stale_callback_id_is_skipped_rather_than_reported(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div");
        host.borrow_mut().listeners.register(node_id, "click", 0);

        // No `__inca_callbacks__` global defined at all.
        let cx = cx.add_empty_window();
        cx.update(|window, _| dispatcher.dispatch(node_id, "click", window));

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
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
            .unwrap();
        assert!(
            !dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap(),
            "the microtask must still be pending before the drain"
        );

        let cx = cx.add_empty_window();
        cx.update(|window, _| drain_jobs_and_refresh(&dispatcher.engine, window));

        assert!(dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap());
    }

    #[gpui::test]
    fn a_pending_job_is_drained_even_when_no_listener_ran(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div");
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
            .unwrap();

        let cx = cx.add_empty_window();
        cx.update(|window, _| dispatcher.dispatch(node_id, "click", window));

        assert!(
            dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap(),
            "a job already queued is owed a turn regardless of this event"
        );
    }

    #[gpui::test]
    fn a_throwing_callback_is_reported(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div");
        host.borrow_mut().listeners.register(node_id, "click", 0);
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.__inca_callbacks__ = { 0: () => { throw new Error('boom'); } };",
            )
            .unwrap();

        let cx = cx.add_empty_window();
        cx.update(|window, _| dispatcher.dispatch(node_id, "click", window));

        let reported = reported.borrow();
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].message(), "Error: boom");
        assert!(reported[0].stack().is_some());
    }

    #[gpui::test]
    fn one_throwing_callback_does_not_stop_the_others(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div");
        host.borrow_mut().listeners.register(node_id, "click", 0);
        host.borrow_mut().listeners.register(node_id, "click", 1);
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.ran = false; \
                 globalThis.__inca_callbacks__ = { \
                    0: () => { throw new Error('boom'); }, \
                    1: () => { globalThis.ran = true; } \
                 };",
            )
            .unwrap();

        let cx = cx.add_empty_window();
        cx.update(|window, _| dispatcher.dispatch(node_id, "click", window));

        assert_eq!(reported.borrow().len(), 1);
        assert!(dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap());
    }
}
