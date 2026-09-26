// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dispatches a native input event into the JS callbacks registered for it
//! via `__inca_native__.addEventListener` (see [`crate::bindings`]), then
//! requests a redraw.
//!
//! Zero-overhead by construction: [`EventDispatcher::dispatch`] is the only
//! thing that ever touches the JS engine or calls
//! [`Window::refresh`](gpui::Window::refresh) on this path, so a re-render
//! with no new input never runs JS.
//!
//! ## Where the real JS function lives
//!
//! [`crate::bindings::EventListeners`] only ever stores a plain `u32`
//! callback id, never an `rquickjs::Value`/`Function`/`Persistent<T>`: a JS
//! handle kept past the call that produced it outlives the context it
//! belongs to. The actual function has to live somewhere, so the convention
//! is:
//! the JS caller stores it itself, at
//! `globalThis.__inca_callbacks__[callbackId]`, before calling
//! `addEventListener` with that id. [`EventDispatcher::dispatch`] looks the
//! real function up fresh inside one `Engine::with` call and drops it
//! before that call returns — it never crosses into Rust-held state.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{App, Window};
use inca_gpui::{
    EventKind, EventMask, EventPayload, KeyPayload, MousePayload, NodeId, WheelPayload,
};
use inca_jsenv::{Engine, EngineError};
use rquickjs::{Ctx, Function, Object};

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
    /// Every button currently held, as a `mousedown`/`mouseup`/`mousemove`
    /// payload's `buttons` bitmask — GPUI's own mouse events carry only the
    /// one button each is about, not which others are held alongside it.
    held_buttons: Rc<Cell<u8>>,
}

impl EventDispatcher {
    /// Reports failures through [`stderr_reporter`].
    pub fn new(engine: Rc<Engine>, host: Rc<RefCell<Host>>) -> Self {
        Self {
            engine,
            host,
            reporter: stderr_reporter(),
            held_buttons: Rc::new(Cell::new(0)),
        }
    }

    /// Sends failures to `reporter` instead. A host with a channel to its
    /// parent reports there; nothing else about dispatch changes.
    #[must_use]
    pub fn with_reporter(mut self, reporter: ErrorReporter) -> Self {
        self.reporter = reporter;
        self
    }

    /// Every kind registered on `node_id`. The render path asks before
    /// wiring an element for input.
    #[must_use]
    pub fn listens(&self, node_id: NodeId) -> EventMask {
        let host = self.host.borrow();
        EventKind::ALL
            .iter()
            .filter(|kind| {
                !host
                    .listeners
                    .callbacks_for(node_id, kind.name())
                    .is_empty()
            })
            .fold(EventMask::NONE, |mask, kind| mask | kind.mask())
    }

    /// Returns `payload` with its `buttons` bitmask corrected against
    /// [`Self::held_buttons`]. A DOM `mouseup` excludes the button just
    /// released; every other kind includes every button still held.
    fn with_held_buttons(&self, event: &str, payload: &EventPayload) -> EventPayload {
        match payload {
            EventPayload::Mouse(mouse) => EventPayload::Mouse(MousePayload {
                buttons: self.update_held_buttons(event, mouse.button),
                ..*mouse
            }),
            EventPayload::Wheel(wheel) => EventPayload::Wheel(WheelPayload {
                mouse: MousePayload {
                    buttons: self.update_held_buttons(event, wheel.mouse.button),
                    ..wheel.mouse
                },
                ..*wheel
            }),
            EventPayload::None | EventPayload::Key(_) => payload.clone(),
        }
    }

    /// Updates [`Self::held_buttons`] for `"mousedown"`/`"mouseup"` and
    /// returns the current value — `"mousemove"`/`"wheel"` only read it.
    /// `button` is `MousePayload`'s own field, a public part of
    /// `inca-gpui`'s API, so an out-of-range value (only 0..=4 are ever
    /// produced by this crate) is handled rather than shifted unchecked.
    fn update_held_buttons(&self, event: &str, button: u8) -> u8 {
        let bit = 1u8.checked_shl(u32::from(button)).unwrap_or(0);
        match event {
            "mousedown" => self.held_buttons.set(self.held_buttons.get() | bit),
            "mouseup" => self.held_buttons.set(self.held_buttons.get() & !bit),
            _ => {}
        }
        self.held_buttons.get()
    }

    /// Calls every JS callback registered for `(node_id, event)` (via
    /// `__inca_native__.addEventListener`), passing one shared object shaped
    /// `{ type, target, currentTarget, ...payload }` plus DOM's three
    /// propagation methods, then drains the job queue and requests a redraw.
    ///
    /// `target` and `currentTarget` are both `node_id`. `currentTarget` (the
    /// node this call is dispatching for) is exact; `target` is only an
    /// approximation of the same value.
    ///
    /// A callback calling `stopImmediatePropagation()` stops the remaining
    /// callbacks *on this node*. `stopPropagation()`/`preventDefault()` are
    /// read back after every callback here has run, and forwarded to `cx`'s
    /// bubble (`node_id`'s ancestors) and `window`'s default handling.
    ///
    /// Never panics. A callback that throws is reported and the rest still
    /// run — one bad listener must not take the host down, nor stop its
    /// siblings. A missing `__inca_callbacks__` registry, or an entry that
    /// is missing or isn't a function, is a stale id and is skipped.
    ///
    /// The drain happens whether or not a listener ran: a job queued earlier
    /// is still owed a turn, and whether this particular event had a
    /// listener says nothing about that.
    pub fn dispatch(
        &self,
        node_id: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        let callback_ids = self
            .host
            .borrow()
            .listeners
            .callbacks_for(node_id, event)
            .to_vec();

        let payload = self.with_held_buttons(event, payload);
        let payload = &payload;

        let stop_propagation = Rc::new(Cell::new(false));
        let stop_immediate = Rc::new(Cell::new(false));
        let prevent_default = Rc::new(Cell::new(false));

        // Collected rather than reported in place: a reporter is free to do
        // anything, and re-entering the engine from inside `with` panics.
        let failures = self.engine.with(|ctx| {
            let mut failures = Vec::new();
            let Ok(callbacks) = ctx.globals().get::<_, Object>("__inca_callbacks__") else {
                return failures;
            };

            let event_object = build_event_object(
                &ctx,
                event,
                node_id,
                payload,
                &stop_propagation,
                &stop_immediate,
                &prevent_default,
            );
            let event_object = match event_object {
                Ok(event_object) => event_object,
                Err(err) => {
                    failures.push(EngineError::capture(&ctx, &err));
                    return failures;
                }
            };

            for callback_id in callback_ids {
                if stop_immediate.get() {
                    break;
                }
                let Ok(callback) = callbacks.get::<_, Function>(callback_id) else {
                    continue;
                };
                if let Err(err) = callback.call::<_, ()>((event_object.clone(),)) {
                    failures.push(EngineError::capture(&ctx, &err));
                }
            }
            failures
        });
        for failure in &failures {
            (self.reporter)(failure);
        }

        if stop_propagation.get() {
            cx.stop_propagation();
        }
        if prevent_default.get() {
            window.prevent_default();
        }

        drain_jobs_and_refresh(&self.engine, window);
    }
}

/// Builds the shared `{ type, target, currentTarget, ...payload }` object a
/// callback is called with, wired to DOM's three propagation methods —
/// setting one of `stop_propagation`/`stop_immediate`/`prevent_default` is
/// how a callback signals it back to the caller.
fn build_event_object<'js>(
    ctx: &Ctx<'js>,
    event: &str,
    node_id: NodeId,
    payload: &EventPayload,
    stop_propagation: &Rc<Cell<bool>>,
    stop_immediate: &Rc<Cell<bool>>,
    prevent_default: &Rc<Cell<bool>>,
) -> rquickjs::Result<Object<'js>> {
    let event_object = Object::new(ctx.clone())?;
    event_object.set("type", event)?;
    event_object.set("target", node_id)?;
    event_object.set("currentTarget", node_id)?;
    set_payload(&event_object, payload)?;

    event_object.set(
        "stopImmediatePropagation",
        Function::new(ctx.clone(), {
            let stop_propagation = Rc::clone(stop_propagation);
            let stop_immediate = Rc::clone(stop_immediate);
            move || {
                stop_propagation.set(true);
                stop_immediate.set(true);
            }
        })?,
    )?;
    event_object.set(
        "stopPropagation",
        Function::new(ctx.clone(), {
            let stop_propagation = Rc::clone(stop_propagation);
            move || stop_propagation.set(true)
        })?,
    )?;
    event_object.set(
        "preventDefault",
        Function::new(ctx.clone(), {
            let prevent_default = Rc::clone(prevent_default);
            move || prevent_default.set(true)
        })?,
    )?;

    Ok(event_object)
}

/// Writes `payload`'s fields onto `event_object`, DOM-named.
fn set_payload(event_object: &Object, payload: &EventPayload) -> rquickjs::Result<()> {
    match payload {
        EventPayload::None => {}
        EventPayload::Mouse(mouse) => set_mouse_fields(event_object, mouse)?,
        EventPayload::Wheel(wheel) => {
            set_mouse_fields(event_object, &wheel.mouse)?;
            event_object.set("deltaX", wheel.delta_x)?;
            event_object.set("deltaY", wheel.delta_y)?;
            event_object.set("deltaZ", wheel.delta_z)?;
            event_object.set("deltaMode", wheel.delta_mode)?;
        }
        EventPayload::Key(key) => set_key_fields(event_object, key)?,
    }
    Ok(())
}

/// Writes [`MousePayload`]'s fields onto `event_object`, DOM-named — shared
/// by [`EventPayload::Mouse`] and [`EventPayload::Wheel`] (DOM's
/// `WheelEvent` extends `MouseEvent`).
fn set_mouse_fields(event_object: &Object, mouse: &MousePayload) -> rquickjs::Result<()> {
    event_object.set("clientX", mouse.client_x)?;
    event_object.set("clientY", mouse.client_y)?;
    event_object.set("button", mouse.button)?;
    event_object.set("buttons", mouse.buttons)?;
    event_object.set("detail", mouse.detail)?;
    set_modifier_fields(event_object, mouse.modifiers)
}

/// Writes [`KeyPayload`]'s fields onto `event_object`, DOM-named.
fn set_key_fields(event_object: &Object, key: &KeyPayload) -> rquickjs::Result<()> {
    event_object.set("key", key.key.clone())?;
    event_object.set("repeat", key.repeat)?;
    set_modifier_fields(event_object, key.modifiers)
}

/// Writes `modifiers`' fields onto `event_object`, DOM-named — shared by
/// [`set_mouse_fields`] and [`set_key_fields`].
fn set_modifier_fields(event_object: &Object, modifiers: gpui::Modifiers) -> rquickjs::Result<()> {
    event_object.set("ctrlKey", modifiers.control)?;
    event_object.set("shiftKey", modifiers.shift)?;
    event_object.set("altKey", modifiers.alt)?;
    event_object.set("metaKey", modifiers.platform)?;
    Ok(())
}

impl EventSink for EventDispatcher {
    fn listens(&self, node_id: NodeId) -> EventMask {
        self.listens(node_id)
    }

    fn dispatch(
        &self,
        node_id: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.dispatch(node_id, event, payload, window, cx);
    }

    fn focus_handle(&self, node_id: NodeId) -> Option<gpui::FocusHandle> {
        self.host.borrow().focus.handle(node_id)
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

    #[test]
    fn listens_reports_click_only_where_something_is_registered() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let registered = host.borrow_mut().tree.create_node("div");
        let quiet = host.borrow_mut().tree.create_node("div");
        host.borrow_mut().listeners.register(registered, "click", 0);

        assert_eq!(dispatcher.listens(registered), EventMask::CLICK);
        assert_eq!(dispatcher.listens(quiet), EventMask::NONE);
    }

    /// `EventKind`/`dom_button_bit` only ever produce `0..=4`, but
    /// `update_held_buttons` takes a plain `u8` — a value outside that
    /// range must not panic the shift, only fail to set any bit.
    #[test]
    fn an_out_of_range_button_does_not_panic() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        assert_eq!(dispatcher.update_held_buttons("mousedown", 200), 0);
        assert_eq!(dispatcher.held_buttons.get(), 0);
    }

    #[gpui::test]
    fn no_listener_registered_reports_nothing(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div");

        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.dispatch(node_id, "click", &EventPayload::None, window, cx);
        });

        assert!(reported.borrow().is_empty());
    }

    #[gpui::test]
    fn a_stale_callback_id_is_skipped_rather_than_reported(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div");
        host.borrow_mut().listeners.register(node_id, "click", 0);

        // No `__inca_callbacks__` global defined at all.
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.dispatch(node_id, "click", &EventPayload::None, window, cx);
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
        cx.update(|window, cx| {
            dispatcher.dispatch(node_id, "click", &EventPayload::None, window, cx);
        });

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
        cx.update(|window, cx| {
            dispatcher.dispatch(node_id, "click", &EventPayload::None, window, cx);
        });

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
        cx.update(|window, cx| {
            dispatcher.dispatch(node_id, "click", &EventPayload::None, window, cx);
        });

        assert_eq!(reported.borrow().len(), 1);
        assert!(dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap());
    }
}
