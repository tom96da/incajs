// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Turns the native input of a window into events and runs the JS listeners
//! registered for them via `__inca_native__.addEventListener` (see
//! [`crate::bindings`]), then requests a redraw.
//!
//! An event only enters the JS engine when it reaches a listener: a capture
//! listener on its path, or a bubble listener on the nodes the event bubbles
//! to.
//! An event that entered the engine, a turn that ran a due timer and a frame that ran a frame
//! callback each drain the job queue and request a redraw. Rendering runs no JS of its own.
//!
//! ## Where the real JS function lives
//!
//! [`crate::bindings::EventListeners`] stores a [`crate::bindings::Listener`]
//! per registration: a plain `u32` callback id with its capture and passive
//! flags, never an `rquickjs::Value`/`Function`/`Persistent<T>`, because a JS
//! handle kept past the call that produced it outlives the context it
//! belongs to. `addEventListener` stores the function in the callback table of
//! the engine and returns the id it got there.
//! The walk of an event looks the real function up fresh inside one
//! `Engine::with` call and drops it before that call returns. It never
//! crosses into Rust-held state.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{App, Window};
use inca_gpui::NodeId;
use inca_jsenv::{Engine, ErrorReporter, report_jobs, stderr_reporter};

use crate::bindings::Host;

mod activation;
mod fire;
mod frame;
mod hover;
mod input;
mod keys;
mod pointer;
mod restyle;
mod sink;
mod timers;

pub(crate) use inca_gpui::event_sink::spec_of;

/// Everything needed to dispatch a native event into JS: the engine to call
/// into, and the registry of which JS callback ids are listening for which
/// `(event target, event name)`.
#[derive(Clone)]
pub struct EventDispatcher {
    pub(crate) engine: Rc<Engine>,
    pub(crate) host: Rc<RefCell<Host>>,
    reporter: ErrorReporter,
    pub(crate) state: Rc<State>,
}

/// The mutable state every clone of an [`EventDispatcher`] shares.
#[derive(Default)]
pub(crate) struct State {
    /// Whether the `pointerdown` of the press now held was canceled. It suppresses `mousedown`,
    /// `mousemove` and `mouseup` until the `pointerup`.
    pointerdown_canceled: Cell<bool>,
    /// The target of the last `pointerdown`, which the `contextmenu` of that press goes to.
    press_target: Cell<Option<NodeId>>,
    /// The target and button of the release that ended a capture, until its clicks and boundary
    /// events fire.
    captured_release: Cell<Option<(NodeId, u8)>>,
    // Position of the previous raw pointer move, shared by every window using
    // this dispatcher. `None` before the first move and after a window exit.
    last_position: Cell<Option<(f32, f32)>>,
    // Delta of the raw pointer move being dispatched now, shared by every
    // node and event name that move produces.
    current_movement: Cell<Option<(f32, f32)>>,
    /// The window's modifier keys as of the last change. `None` until
    /// [`EventDispatcher::sync_modifiers`] first reads them.
    held_modifiers: Cell<Option<gpui::Modifiers>>,
    /// The containers `gpui` reports as hovered.
    pub(crate) hovered: RefCell<BTreeSet<NodeId>>,
    /// The deepest hovered container the hover events last reported and its
    /// ancestors, nearest first.
    hover_prev: RefCell<Vec<NodeId>>,
    /// Whether a hover report is waiting for the end of the update.
    hover_scheduled: Cell<bool>,
    /// Whether the update being handled already reported its deepest
    /// container.
    hover_settled: Cell<bool>,
    /// The Space press waiting for its release: the window and the focused
    /// button. An Enter click, a Space key down, a Space key up or the button
    /// losing focus ends it.
    space_down: Cell<Option<(gpui::WindowId, NodeId)>>,
    /// The focused enabled button under a held mouse press: the window and
    /// the button. A Space key up clicks it. A click, the mouse release or the
    /// button losing focus ends it, and the next mouse press replaces it.
    mouse_held: Cell<Option<(gpui::WindowId, NodeId)>>,
    /// The `input` a held left press selects in, until its release.
    input_drag: Cell<Option<NodeId>>,
    /// The task that sleeps until the next timer is due, or `None` before
    /// [`EventDispatcher::start_timers`] and after [`EventDispatcher::stop_timers`].
    timer_pump: RefCell<Option<gpui::Task<()>>>,
    /// Whether the window holds a next-frame callback that runs the frame callbacks.
    frame_requested: Cell<bool>,
}

/// Records the node the window focuses in `host`.
pub(crate) fn observe_focus(host: &RefCell<Host>, window: &Window, cx: &App) {
    host.borrow_mut().focus.observe(window, cx);
}

/// Sends the listener errors `host` collected to `reporter`, in order. Call it
/// outside any [`Engine::with`].
pub(crate) fn report_failures(host: &RefCell<Host>, reporter: &ErrorReporter) {
    let pending = std::mem::take(&mut host.borrow_mut().failures);
    for failure in &pending {
        reporter(failure);
    }
}

impl EventDispatcher {
    /// Reports failures through [`stderr_reporter`].
    pub fn new(engine: Rc<Engine>, host: Rc<RefCell<Host>>) -> Self {
        Self {
            engine,
            host,
            reporter: stderr_reporter(),
            state: Rc::default(),
        }
    }

    /// Sends failures to `reporter` instead. A host with a channel to its
    /// parent reports there; nothing else about dispatch changes.
    #[must_use]
    pub fn with_reporter(mut self, reporter: ErrorReporter) -> Self {
        self.reporter = reporter;
        self
    }

    /// Whether the containers report the pointer: a listener is registered, a node can take
    /// focus or a style rule names `:hover` or `:active`. The render path asks once per frame.
    #[must_use]
    pub fn tracks_pointer(&self) -> bool {
        let host = self.host.borrow();
        !host.listeners.is_empty() || host.focus.has_handles() || host.tree.uses_pointer_state()
    }

    /// Takes the window's current modifier keys as the baseline for the next
    /// change. With `force` unset, an existing baseline stays.
    pub fn sync_modifiers(&self, window: &Window, force: bool) {
        if force || self.state.held_modifiers.get().is_none() {
            self.state.held_modifiers.set(Some(window.modifiers()));
        }
    }

    /// Reports listener errors, in the order they happened. Call it outside any
    /// [`Engine::with`].
    pub fn report_pending(&self) {
        report_failures(&self.host, &self.reporter);
    }

    /// Records the node the window focuses, so `focusedNode()` in JS that runs
    /// next sees focus the window applied itself. Call it before running JS
    /// from outside an event.
    pub fn observe_focus(&self, window: &Window, cx: &App) {
        observe_focus(&self.host, window, cx);
    }

    /// [`Self::observe_focus`], then [`drain_jobs_and_refresh`] with this
    /// dispatcher's engine and reporter, then [`Self::report_pending`].
    pub fn drain_jobs_and_refresh(&self, window: &mut Window, cx: &App) {
        self.observe_focus(window, cx);
        self.drain_observed(window);
    }

    fn drain_observed(&self, window: &mut Window) {
        drain_jobs_and_refresh(&self.engine, &self.reporter, window);
        self.report_pending();
    }
}

/// [`report_jobs`], then requests a redraw. Code that only schedules work (a
/// reactivity effect, batched via a microtask) hasn't
/// mutated the tree once the scheduling call returns, so the queue has to run
/// before the frame is drawn.
///
/// Call it outside any [`Engine::with`].
pub fn drain_jobs_and_refresh(engine: &Engine, reporter: &ErrorReporter, window: &mut Window) {
    report_jobs(engine, reporter);
    window.refresh();
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{dispatcher_with_engine, listen};
    use gpui::TestAppContext;
    use inca_gpui::EventPayload;

    #[gpui::test]
    fn a_stale_callback_id_is_skipped_rather_than_reported(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div").unwrap();
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
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
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
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
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
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div").unwrap();
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
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
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
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        let root = host.borrow().root();
        host.borrow_mut().tree.append_child(root, node).unwrap();
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
}
