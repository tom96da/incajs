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
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;

use gpui::{App, ScrollHandle, Window};
use inca_gpui::{
    EventKind, EventMask, EventPayload, KeyPayload, MousePayload, NodeId, WheelPayload,
    dom_buttons_bit,
};
use inca_jsenv::{Engine, EngineError};
use rquickjs::convert::Coerced;
use rquickjs::object::{Accessor, Property};
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

/// `bubbles`, `cancelable` and `composed` of every event the host dispatches.
/// Any other name takes all three as false.
const EVENT_FLAGS: [(&str, [bool; 3]); 11] = [
    ("click", [true, true, true]),
    ("mousedown", [true, true, true]),
    ("mouseup", [true, true, true]),
    ("mousemove", [true, true, true]),
    ("mouseenter", [false, false, false]),
    ("mouseleave", [false, false, false]),
    ("wheel", [true, true, true]),
    ("keydown", [true, true, true]),
    ("keyup", [true, true, true]),
    ("focus", [false, false, true]),
    ("blur", [false, false, true]),
];

fn flags_of(event: &str) -> [bool; 3] {
    EVENT_FLAGS
        .iter()
        .find(|(name, _)| *name == event)
        .map_or([false; 3], |(_, flags)| *flags)
}

/// The origin of every `timeStamp`.
fn time_origin() -> Instant {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    *ORIGIN.get_or_init(Instant::now)
}

/// What every node of one event shares: the `eventId`, whether a callback
/// has called `preventDefault()` and the `timeStamp`.
#[derive(Clone, Copy)]
struct EventState {
    id: u64,
    prevented: bool,
    time_stamp: f64,
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
    // Position of the previous raw pointer move, shared by every window using
    // this dispatcher. `None` before the first move and after a window exit.
    last_position: Rc<Cell<Option<(f32, f32)>>>,
    // Delta of the raw pointer move being dispatched now, shared by every
    // node and event name that move produces.
    current_movement: Rc<Cell<Option<(f32, f32)>>>,
    /// Set while the `wheel` event being dispatched right now has been
    /// stopped. Later `wheel` dispatches skip their callbacks, and GPUI's own
    /// bubble carries on so a scroll container still scrolls.
    wheel_stopped: Rc<Cell<bool>>,
    /// Last number handed out as an `eventId`.
    last_event_id: Rc<Cell<u64>>,
    /// The state of each event name dispatched during the input being
    /// handled now. Emptied when that input's update ends.
    current_events: Rc<RefCell<HashMap<String, EventState>>>,
    /// The scroll state of every scrolling container, created on first ask.
    scroll_handles: Rc<RefCell<HashMap<NodeId, ScrollHandle>>>,
}

impl EventDispatcher {
    /// Reports failures through [`stderr_reporter`].
    pub fn new(engine: Rc<Engine>, host: Rc<RefCell<Host>>) -> Self {
        time_origin();
        Self {
            engine,
            host,
            reporter: stderr_reporter(),
            held_buttons: Rc::new(Cell::new(0)),
            last_position: Rc::new(Cell::new(None)),
            current_movement: Rc::new(Cell::new(None)),
            wheel_stopped: Rc::new(Cell::new(false)),
            last_event_id: Rc::new(Cell::new(0)),
            current_events: Rc::new(RefCell::new(HashMap::new())),
            scroll_handles: Rc::new(RefCell::new(HashMap::new())),
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

    /// Returns the [`EventState`] for `event`. The first dispatch of a name
    /// during one input takes the next `eventId` and the current time. Later
    /// dispatches of that name during the same input reuse them. The state is
    /// forgotten when the input's update ends.
    fn event_state(&self, event: &str, cx: &mut App) -> EventState {
        let mut events = self.current_events.borrow_mut();
        if let Some(state) = events.get(event) {
            return *state;
        }
        let id = self.last_event_id.get() + 1;
        self.last_event_id.set(id);
        if events.is_empty() {
            let current = Rc::clone(&self.current_events);
            cx.defer(move |_| current.borrow_mut().clear());
        }
        let state = EventState {
            id,
            prevented: false,
            time_stamp: time_origin().elapsed().as_secs_f64() * 1000.0,
        };
        events.insert(event.to_owned(), state);
        state
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
    /// produced by this crate) is handled: [`dom_buttons_bit`] maps it to 0.
    fn update_held_buttons(&self, event: &str, button: u8) -> u8 {
        let bit = dom_buttons_bit(button);
        match event {
            "mousedown" => self.held_buttons.set(self.held_buttons.get() | bit),
            "mouseup" => self.held_buttons.set(self.held_buttons.get() & !bit),
            _ => {}
        }
        self.held_buttons.get()
    }

    /// Returns `payload` with `movementX`/`movementY` set. `mousemove`,
    /// `mouseenter` and `mouseleave` take the delta of the raw move being
    /// dispatched, 0 when none is. Every other event takes its position minus
    /// [`Self::last_position`], 0 when that is `None`.
    fn with_movement(&self, payload: &EventPayload, event: &str) -> EventPayload {
        let movement_for = |mouse: &MousePayload| match event {
            "mousemove" | "mouseenter" | "mouseleave" => {
                self.current_movement.get().unwrap_or((0.0, 0.0))
            }
            _ => self.last_position.get().map_or((0.0, 0.0), |(x, y)| {
                (mouse.client_x - x, mouse.client_y - y)
            }),
        };
        match payload {
            EventPayload::Mouse(mouse) => {
                let (movement_x, movement_y) = movement_for(mouse);
                EventPayload::Mouse(MousePayload {
                    movement_x,
                    movement_y,
                    ..*mouse
                })
            }
            EventPayload::Wheel(wheel) => {
                let (movement_x, movement_y) = movement_for(&wheel.mouse);
                EventPayload::Wheel(WheelPayload {
                    mouse: MousePayload {
                        movement_x,
                        movement_y,
                        ..wheel.mouse
                    },
                    ..*wheel
                })
            }
            EventPayload::None | EventPayload::Key(_) => payload.clone(),
        }
    }

    /// Calls every JS callback registered for `(node_id, event)` (via
    /// `__inca_native__.addEventListener`), passing one shared object shaped
    /// `{ type, target, currentTarget, eventId, ...payload }` plus DOM's three
    /// propagation methods, then drains the job queue and requests a redraw.
    ///
    /// `eventId` identifies the event. Every node dispatched for one event
    /// name within one input carries the same number, and a later input gets
    /// a larger one. A dispatch that no input produced, such as the `focus`
    /// and `blur` of a focus change, takes a new number per event name.
    ///
    /// `bubbles`, `cancelable`, `composed`, `defaultPrevented`, `eventPhase`,
    /// `isTrusted` and `timeStamp` are the base fields. `eventPhase` is 2
    /// where `currentTarget` is `target` and 3 elsewhere. `composedPath()`
    /// returns the node ids from `target` up to the root.
    ///
    /// `currentTarget` is `node_id`. `target` is the deepest container under
    /// the pointer for `mousedown`, `mouseup`, `mousemove` and `wheel`, the
    /// nearest common ancestor of the press and release containers for
    /// `click`, and the focused node for `keydown` and `keyup`. Every other
    /// event, and any of these when no target is recorded (a dispatch made
    /// outside input handling), uses `node_id`.
    ///
    /// A callback calling `stopImmediatePropagation()` stops the remaining
    /// callbacks *on this node*. `stopPropagation()`/`preventDefault()` are
    /// read back after every callback here has run, and forwarded to `cx`'s
    /// bubble (`node_id`'s ancestors) and `window`'s default handling.
    /// For `"wheel"`, `stopPropagation()` only skips the ancestors' callbacks.
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
        let callback_ids = if event == "wheel" && self.wheel_stopped.get() {
            Vec::new()
        } else {
            self.host
                .borrow()
                .listeners
                .callbacks_for(node_id, event)
                .to_vec()
        };

        let payload = self.with_held_buttons(event, payload);
        let payload = self.with_movement(&payload, event);
        let state = self.event_state(event, cx);
        let target = self.target_of(node_id, event, window, cx);
        let outcome = self.run_callbacks(node_id, target, event, &payload, state, callback_ids);

        if outcome.stop_propagation {
            if event == "wheel" {
                self.wheel_stopped.set(true);
                let stopped = Rc::clone(&self.wheel_stopped);
                cx.defer(move |_| stopped.set(false));
            } else {
                cx.stop_propagation();
            }
        }
        // Only GPUI's own defaults read this flag. For `wheel`, the scroll
        // rollback in `inca-gpui` reads it too and restores the offsets.
        if outcome.prevent_default {
            window.prevent_default();
        }

        self.drain_jobs_and_refresh(window);
    }

    /// The `target` of `event` dispatched for `node_id`.
    fn target_of(&self, node_id: NodeId, event: &str, window: &Window, cx: &App) -> NodeId {
        let found = match event {
            "mousedown" | "mouseup" | "mousemove" | "wheel" => inca_gpui::mouse_target(cx),
            "click" => match (inca_gpui::pressed_target(cx), inca_gpui::mouse_target(cx)) {
                (Some(down), Some(up)) => self.common_ancestor(down, up),
                (_, up) => up,
            },
            "keydown" | "keyup" => self.host.borrow().focus.focused_node(window, cx),
            _ => None,
        };
        self.resolve(found, node_id)
    }

    /// `found` when that node is still in the tree, else `node_id`.
    fn resolve(&self, found: Option<NodeId>, node_id: NodeId) -> NodeId {
        let host = self.host.borrow();
        found
            .filter(|id| host.tree.get(*id).is_some())
            .unwrap_or(node_id)
    }

    /// `target` and its ancestors, nearest first.
    fn path_from(&self, target: NodeId) -> Vec<NodeId> {
        let host = self.host.borrow();
        std::iter::successors(Some(target), |&id| {
            host.tree.get(id).and_then(inca_gpui::VirtualNode::parent)
        })
        .collect()
    }

    /// The nearest node that is `a`, `b` or an ancestor of both.
    fn common_ancestor(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        let above_b = self.path_from(b);
        self.path_from(a).into_iter().find(|i| above_b.contains(i))
    }

    /// Calls `callback_ids` for one `event` with `target` as the event's
    /// `target` and `node_id` as its `currentTarget`. Reports what the
    /// callbacks asked for; the caller acts on it.
    fn run_callbacks(
        &self,
        node_id: NodeId,
        target: NodeId,
        event: &str,
        payload: &EventPayload,
        state: EventState,
        callback_ids: Vec<u32>,
    ) -> Outcome {
        let stop_propagation = Rc::new(Cell::new(false));
        let stop_immediate = Rc::new(Cell::new(false));
        let prevent_default = Rc::new(Cell::new(state.prevented));

        // Collected rather than reported in place: a reporter is free to do
        // anything, and re-entering the engine from inside `with` panics.
        let path = self.path_from(target);
        let live = Rc::new(Cell::new(true));
        let failures = self.engine.with(|ctx| {
            let mut failures = Vec::new();
            let Ok(callbacks) = ctx.globals().get::<_, Object>("__inca_callbacks__") else {
                return failures;
            };

            let event_object = build_event_object(
                &ctx,
                event,
                (node_id, target),
                (&path, &live),
                payload,
                &state,
                &[
                    Rc::clone(&stop_propagation),
                    Rc::clone(&stop_immediate),
                    Rc::clone(&prevent_default),
                ],
            );
            let event_object = event_object.and_then(|object| {
                object.set("eventId", state.id)?;
                Ok(object)
            });
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
        live.set(false);
        for failure in &failures {
            (self.reporter)(failure);
        }
        if let Some(shared) = self.current_events.borrow_mut().get_mut(event) {
            shared.prevented |= prevent_default.get();
        }
        Outcome {
            stop_propagation: stop_propagation.get(),
            prevent_default: prevent_default.get(),
        }
    }

    /// Fires the `click` a key press on `node_id` produces: `button` 0,
    /// `buttons` 0, `detail` 0 and coordinates 0, with the key event's
    /// `modifiers`. It runs the listeners of `node_id` and then each
    /// ancestor's, with `target` the node pressed, until one calls
    /// `stopPropagation()`.
    pub fn activate(
        &self,
        node_id: NodeId,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        let payload = EventPayload::Mouse(MousePayload {
            client_x: 0.0,
            client_y: 0.0,
            movement_x: 0.0,
            movement_y: 0.0,
            button: 0,
            buttons: 0,
            detail: 0,
            modifiers,
        });
        let mut current = Some(node_id);
        while let Some(id) = current {
            let callback_ids = self
                .host
                .borrow()
                .listeners
                .callbacks_for(id, "click")
                .to_vec();
            if callback_ids.is_empty() {
                current = self
                    .host
                    .borrow()
                    .tree
                    .get(id)
                    .and_then(inca_gpui::VirtualNode::parent);
                continue;
            }
            let state = self.event_state("click", cx);
            let outcome = self.run_callbacks(id, node_id, "click", &payload, state, callback_ids);
            self.drain_jobs_and_refresh(window);
            if outcome.stop_propagation {
                break;
            }
            current = self
                .host
                .borrow()
                .tree
                .get(id)
                .and_then(inca_gpui::VirtualNode::parent);
        }
    }

    /// [`drain_jobs_and_refresh`] with this dispatcher's engine and reporter.
    pub fn drain_jobs_and_refresh(&self, window: &mut Window) {
        drain_jobs_and_refresh(&self.engine, &self.reporter, window);
    }
}

/// What the callbacks of one dispatch asked for.
struct Outcome {
    stop_propagation: bool,
    prevent_default: bool,
}

/// Builds the shared `{ type, target, currentTarget, ...payload }` object a
/// callback is called with, wired to DOM's three propagation methods —
/// setting one of `stop_propagation`/`stop_immediate`/`prevent_default` is
/// how a callback signals it back to the caller.
fn build_event_object<'js>(
    ctx: &Ctx<'js>,
    event: &str,
    (node_id, target): (NodeId, NodeId),
    (path, live): (&[NodeId], &Rc<Cell<bool>>),
    payload: &EventPayload,
    state: &EventState,
    signals: &[Rc<Cell<bool>>; 3],
) -> rquickjs::Result<Object<'js>> {
    let [stop_propagation, stop_immediate, prevent_default] = signals;
    let [bubbles, cancelable, composed] = flags_of(event);
    let event_object = Object::new(ctx.clone())?;
    event_object.set("type", event)?;
    event_object.set("target", target)?;
    event_object.set("currentTarget", node_id)?;
    event_object.prop("srcElement", Property::from(target))?;
    for (name, value) in [
        ("NONE", 0),
        ("CAPTURING_PHASE", 1),
        ("AT_TARGET", 2),
        ("BUBBLING_PHASE", 3),
    ] {
        event_object.prop(name, Property::from(value))?;
    }
    event_object.set("bubbles", bubbles)?;
    event_object.set("cancelable", cancelable)?;
    event_object.set("composed", composed)?;
    event_object.prop(
        "defaultPrevented",
        Accessor::new_get({
            let prevent_default = Rc::clone(prevent_default);
            move || prevent_default.get()
        })
        .enumerable(),
    )?;
    event_object.prop(
        "returnValue",
        Accessor::new_get({
            let prevent_default = Rc::clone(prevent_default);
            move || !prevent_default.get()
        })
        .set({
            let prevent_default = Rc::clone(prevent_default);
            move |value: Coerced<bool>| {
                let value = value.0;
                if !value && cancelable {
                    prevent_default.set(true);
                }
            }
        }),
    )?;
    event_object.prop(
        "cancelBubble",
        Accessor::new_get({
            let stop_propagation = Rc::clone(stop_propagation);
            move || stop_propagation.get()
        })
        .set({
            let stop_propagation = Rc::clone(stop_propagation);
            move |value: Coerced<bool>| {
                let value = value.0;
                if value {
                    stop_propagation.set(true);
                }
            }
        }),
    )?;
    event_object.set("eventPhase", if node_id == target { 2 } else { 3 })?;
    event_object.set(
        "composedPath",
        Function::new(ctx.clone(), {
            let path = path.to_vec();
            let live = Rc::clone(live);
            move || if live.get() { path.clone() } else { Vec::new() }
        })?,
    )?;
    event_object.set("isTrusted", true)?;
    event_object.set("timeStamp", state.time_stamp)?;
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
            move || {
                if cancelable {
                    prevent_default.set(true);
                }
            }
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
    // Identical to clientX/clientY today — nothing here scrolls the page
    // itself, which is the only thing that would tell them apart.
    event_object.set("pageX", mouse.client_x)?;
    event_object.set("pageY", mouse.client_y)?;
    event_object.set("movementX", mouse.movement_x)?;
    event_object.set("movementY", mouse.movement_y)?;
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

    fn activate(
        &self,
        node_id: NodeId,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.activate(node_id, modifiers, window, cx);
    }

    fn focus_handle(&self, node_id: NodeId) -> Option<gpui::FocusHandle> {
        self.host.borrow().focus.handle(node_id)
    }

    fn scroll_handle(&self, node_id: NodeId) -> Option<ScrollHandle> {
        Some(
            self.scroll_handles
                .borrow_mut()
                .entry(node_id)
                .or_default()
                .clone(),
        )
    }

    // The first call of an update computes the delta and defers clearing it,
    // so later calls in that update keep it.
    fn pointer_moved(&self, position: gpui::Point<gpui::Pixels>, cx: &mut App) {
        if self.current_movement.get().is_some() {
            return;
        }
        let here = (f32::from(position.x), f32::from(position.y));
        let movement = self
            .last_position
            .replace(Some(here))
            .map_or((0.0, 0.0), |(x, y)| (here.0 - x, here.1 - y));
        self.current_movement.set(Some(movement));
        let current = Rc::clone(&self.current_movement);
        cx.defer(move |_| current.set(None));
    }

    fn pointer_left(&self) {
        self.last_position.set(None);
    }
}

/// Drains `QuickJS`'s pending-job queue, sends each failure it leaves (a job
/// that threw, a promise rejected with no handler) to `reporter`, then
/// requests a redraw. Code that only schedules work (a `@vue/runtime-core`
/// reactivity effect, batched via a microtask) hasn't mutated the tree once
/// the scheduling call returns, so the queue has to run before the frame is
/// drawn.
///
/// Call it outside any [`Engine::with`] — it takes the context itself, and
/// nesting that panics with "`RefCell` already borrowed".
pub fn drain_jobs_and_refresh(engine: &Engine, reporter: &ErrorReporter, window: &mut Window) {
    for failure in engine.run_jobs() {
        reporter(&failure);
    }
    window.refresh();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bindings::install;
    use gpui::{TestAppContext, point, px};

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
        let registered = host.borrow_mut().tree.create_node("div").unwrap();
        let quiet = host.borrow_mut().tree.create_node("div").unwrap();
        host.borrow_mut().listeners.register(registered, "click", 0);

        assert_eq!(dispatcher.listens(registered), EventMask::CLICK);
        assert_eq!(dispatcher.listens(quiet), EventMask::NONE);
    }

    /// `inca-gpui` only ever produces a `button` index in `0..=4`, but
    /// `update_held_buttons` takes a plain `u8` — a value outside that
    /// range must not panic, only fail to set any bit.
    #[test]
    fn an_out_of_range_button_does_not_panic() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        assert_eq!(dispatcher.update_held_buttons("mousedown", 200), 0);
        assert_eq!(dispatcher.held_buttons.get(), 0);
    }

    #[test]
    fn a_right_mousedown_sets_the_right_buttons_bit() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        assert_eq!(dispatcher.update_held_buttons("mousedown", 2), 0b0_0010);
    }

    #[test]
    fn a_middle_mousedown_sets_the_middle_buttons_bit() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        assert_eq!(dispatcher.update_held_buttons("mousedown", 1), 0b0_0100);
    }

    #[gpui::test]
    fn no_listener_registered_reports_nothing(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div").unwrap();

        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.dispatch(node_id, "click", &EventPayload::None, window, cx);
        });

        assert!(reported.borrow().is_empty());
    }

    #[gpui::test]
    fn a_stale_callback_id_is_skipped_rather_than_reported(cx: &mut TestAppContext) {
        let (dispatcher, host, reported) = dispatcher_with_engine();
        let node_id = host.borrow_mut().tree.create_node("div").unwrap();
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
        cx.update(|window, _| dispatcher.drain_jobs_and_refresh(window));

        assert!(dispatcher.engine.eval::<bool>("globalThis.ran;").unwrap());
    }

    #[gpui::test]
    fn a_pending_job_is_drained_even_when_no_listener_ran(cx: &mut TestAppContext) {
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
        let node_id = host.borrow_mut().tree.create_node("div").unwrap();
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
        let node_id = host.borrow_mut().tree.create_node("div").unwrap();
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

    fn at(x: f32, y: f32) -> gpui::Point<gpui::Pixels> {
        point(px(x), px(y))
    }

    fn mouse_at(x: f32, y: f32) -> EventPayload {
        EventPayload::Mouse(MousePayload::at(at(x, y), gpui::Modifiers::none()))
    }

    fn movement_of(payload: &EventPayload) -> (f32, f32) {
        match payload {
            EventPayload::Mouse(mouse) => (mouse.movement_x, mouse.movement_y),
            EventPayload::Wheel(wheel) => (wheel.mouse.movement_x, wheel.mouse.movement_y),
            other => panic!("expected a mouse or wheel payload, got {other:?}"),
        }
    }

    fn wheel_at(x: f32, y: f32) -> EventPayload {
        EventPayload::Wheel(WheelPayload {
            mouse: MousePayload::at(at(x, y), gpui::Modifiers::none()),
            delta_x: 0.0,
            delta_y: 1.0,
            delta_z: 0.0,
            delta_mode: 0,
        })
    }

    // The deferred clear runs when the recording update ends, so the value is
    // read inside that update.
    #[gpui::test]
    fn pointer_moved_reports_zero_first_then_the_delta(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();

        let first = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(10.0, 10.0), cx);
            dispatcher.current_movement.get()
        });
        assert_eq!(first, Some((0.0, 0.0)));
        assert_eq!(dispatcher.current_movement.get(), None);

        let second = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(30.0, 5.0), cx);
            dispatcher.current_movement.get()
        });
        assert_eq!(second, Some((20.0, -5.0)));
        assert_eq!(dispatcher.last_position.get(), Some((30.0, 5.0)));
    }

    #[gpui::test]
    fn a_second_pointer_moved_in_one_cycle_keeps_the_first_delta(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));

        let seen = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(30.0, 5.0), cx);
            dispatcher.pointer_moved(at(100.0, 100.0), cx);
            dispatcher.current_movement.get()
        });

        assert_eq!(seen, Some((20.0, -5.0)));
        assert_eq!(dispatcher.last_position.get(), Some((30.0, 5.0)));
    }

    #[gpui::test]
    fn pointer_left_makes_the_next_move_report_zero(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));

        dispatcher.pointer_left();
        assert_eq!(dispatcher.last_position.get(), None);

        let seen = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(50.0, 60.0), cx);
            dispatcher.current_movement.get()
        });
        assert_eq!(seen, Some((0.0, 0.0)));
    }

    #[gpui::test]
    fn hover_events_share_the_movement_of_their_raw_move(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));

        let seen = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(30.0, 5.0), cx);
            ["mousemove", "mouseenter", "mouseleave"]
                .map(|event| movement_of(&dispatcher.with_movement(&mouse_at(30.0, 5.0), event)))
        });
        assert_eq!(seen, [(20.0, -5.0); 3]);
    }

    #[gpui::test]
    fn a_hover_with_no_raw_move_reports_zero(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));
        cx.run_until_parked();
        cx.update(|_, cx| dispatcher.pointer_moved(at(30.0, 5.0), cx));
        cx.run_until_parked();

        for event in ["mouseenter", "mouseleave", "mousemove"] {
            let payload = dispatcher.with_movement(&mouse_at(90.0, 90.0), event);
            assert_eq!(movement_of(&payload), (0.0, 0.0), "{event}");
        }
    }

    #[test]
    fn mousedown_mouseup_and_wheel_measure_from_the_last_move_and_leave_it_in_place() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        dispatcher.last_position.set(Some((10.0, 10.0)));

        for event in ["mousedown", "mouseup"] {
            let payload = dispatcher.with_movement(&mouse_at(30.0, 40.0), event);
            assert_eq!(movement_of(&payload), (20.0, 30.0), "{event}");
        }
        let payload = dispatcher.with_movement(&wheel_at(30.0, 40.0), "wheel");
        assert_eq!(movement_of(&payload), (20.0, 30.0));
        assert_eq!(dispatcher.last_position.get(), Some((10.0, 10.0)));
    }

    #[test]
    fn mousedown_mouseup_and_wheel_report_zero_before_any_move() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        for event in ["mousedown", "mouseup"] {
            let payload = dispatcher.with_movement(&mouse_at(30.0, 40.0), event);
            assert_eq!(movement_of(&payload), (0.0, 0.0), "{event}");
        }
        let payload = dispatcher.with_movement(&wheel_at(30.0, 40.0), "wheel");
        assert_eq!(movement_of(&payload), (0.0, 0.0));
        assert_eq!(dispatcher.last_position.get(), None);
    }

    #[test]
    fn a_click_or_key_payload_is_left_alone() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        dispatcher.last_position.set(Some((10.0, 10.0)));

        let payload = dispatcher.with_movement(&EventPayload::None, "click");
        assert_eq!(payload, EventPayload::None);
    }

    // Snapshot per call: [type, bubbles, cancelable, composed, defaultPrevented
    // before, defaultPrevented after preventDefault(), eventPhase,
    // currentTarget, isTrusted, timeStamp].
    const RECORDER: &str = "globalThis.seen = []; \
        globalThis.__inca_callbacks__ = { 0: (e) => { \
            const before = e.defaultPrevented; e.preventDefault(); \
            globalThis.seen.push([e.type, e.bubbles, e.cancelable, e.composed, \
                before, e.defaultPrevented, e.eventPhase, e.currentTarget, \
                e.isTrusted, e.timeStamp]); } };";

    fn seen(dispatcher: &EventDispatcher) -> Vec<Vec<serde_json::Value>> {
        let json = dispatcher
            .engine
            .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
            .unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn listen(host: &Rc<RefCell<Host>>, node: NodeId, event: &str) {
        host.borrow_mut().listeners.register(node, event, 0);
    }

    #[gpui::test]
    fn base_fields_follow_the_spec_rows_and_prevent_default_needs_cancelable(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        let cx = cx.add_empty_window();
        let mut mismatches = Vec::new();
        // (event, bubbles, cancelable, composed)
        for (event, bubbles, cancelable, composed) in [
            ("click", true, true, true),
            ("mouseenter", false, false, false),
            ("mouseleave", false, false, false),
            ("wheel", true, true, true),
            ("keydown", true, true, true),
            ("focus", false, false, true),
            ("blur", false, false, true),
        ] {
            listen(&host, node, event);
            cx.update(|window, cx| {
                dispatcher.dispatch(node, event, &EventPayload::None, window, cx);
            });
            let rows = seen(&dispatcher);
            let want = serde_json::json!([
                event, bubbles, cancelable, composed, false, cancelable, 2, node, true
            ]);
            if rows.len() != 1 || rows[0][..9] != *want.as_array().unwrap() {
                mismatches.push(format!("{event}: {rows:?}"));
            }
        }
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    #[gpui::test]
    fn an_unlisted_event_name_takes_all_three_flags_false(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        listen(&host, node, "menu:1");
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.dispatch(node, "menu:1", &EventPayload::None, window, cx);
        });
        let rows = seen(&dispatcher);
        assert_eq!(
            rows[0][1..6],
            serde_json::json!([false, false, false, false, false])
                .as_array()
                .unwrap()[..]
        );
    }

    #[gpui::test]
    fn a_non_cancelable_event_stays_unprevented_on_every_node(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let child = host.borrow_mut().tree.create_node("div").unwrap();
        let parent = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        listen(&host, child, "focus");
        listen(&host, parent, "focus");
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            for node in [child, parent] {
                dispatcher.dispatch(node, "focus", &EventPayload::None, window, cx);
            }
        });
        for row in seen(&dispatcher) {
            assert_eq!(row[4], serde_json::json!(false));
            assert_eq!(row[5], serde_json::json!(false));
            assert_eq!(row[6], serde_json::json!(2), "focus is non-bubbling");
        }
    }

    #[gpui::test]
    fn time_stamp_counts_milliseconds_from_the_host_start_and_grows(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        listen(&host, node, "click");
        let cx = cx.add_empty_window();
        for _ in 0..3 {
            cx.update(|window, cx| {
                dispatcher.dispatch(node, "click", &EventPayload::None, window, cx);
            });
        }
        let stamps: Vec<f64> = seen(&dispatcher)
            .iter()
            .map(|r| r[9].as_f64().unwrap())
            .collect();
        assert!(stamps[0] > 0.0, "{stamps:?}");
        assert!(stamps.windows(2).all(|w| w[1] >= w[0]), "{stamps:?}");
    }

    #[gpui::test]
    fn the_legacy_members_follow_the_event_state(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&host, node, "click");
        // Each entry runs in its own dispatch and reports one string.
        let probes = [
            (
                "[e.NONE, e.CAPTURING_PHASE, e.AT_TARGET, e.BUBBLING_PHASE].join()",
                "0,1,2,3",
            ),
            ("e.srcElement === e.target", "true"),
            (
                "[e.cancelBubble, (e.cancelBubble = true, e.cancelBubble)].join()",
                "false,true",
            ),
            ("(e.cancelBubble = false, e.cancelBubble)", "false"),
            ("(e.cancelBubble = 1, e.cancelBubble)", "true"),
            ("(e.cancelBubble = '', e.cancelBubble)", "false"),
            (
                "[e.returnValue, (e.returnValue = false, e.returnValue), e.defaultPrevented].join()",
                "true,false,true",
            ),
            ("(e.returnValue = true, e.defaultPrevented)", "false"),
            ("(e.returnValue = 0, e.defaultPrevented)", "true"),
            ("(e.returnValue = 'no', e.defaultPrevented)", "false"),
            (
                "(e.preventDefault(), [e.defaultPrevented, e.returnValue].join())",
                "true,false",
            ),
            (
                "(() => { const p = e.preventDefault; p(); return e.defaultPrevented; })()",
                "true",
            ),
            (
                "(e.preventDefault(), (() => { try { e.defaultPrevented = false; } catch {} return e.defaultPrevented; })())",
                "true",
            ),
        ];
        let cx = cx.add_empty_window();
        let mut mismatches = Vec::new();
        for (expr, want) in probes {
            dispatcher
                .engine
                .eval::<()>(&format!(
                    "globalThis.out = ''; globalThis.__inca_callbacks__ = {{ 0: (e) => {{ \
                        globalThis.out = String({expr}); }} }};"
                ))
                .unwrap();
            cx.update(|window, cx| {
                dispatcher.dispatch(node, "click", &EventPayload::None, window, cx);
            });
            let got = dispatcher.engine.eval::<String>("globalThis.out").unwrap();
            if got != want {
                mismatches.push(format!("{expr}: {got} (want {want})"));
            }
        }
        assert!(mismatches.is_empty(), "{mismatches:#?}");
    }

    #[gpui::test]
    fn a_keyboard_click_starts_at_the_innermost_node_with_a_listener(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let outer = host.borrow_mut().tree.create_node("div").unwrap();
        let parent = host.borrow_mut().tree.create_node("div").unwrap();
        let button = host.borrow_mut().tree.create_node("button").unwrap();
        host.borrow_mut().tree.append_child(outer, parent).unwrap();
        host.borrow_mut().tree.append_child(parent, button).unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        listen(&host, parent, "click");
        listen(&host, outer, "click");
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.activate(button, gpui::Modifiers::none(), window, cx);
        });
        // [currentTarget, defaultPrevented before, eventPhase, isTrusted]
        let rows: Vec<_> = seen(&dispatcher)
            .iter()
            .map(|r| [r[7].clone(), r[4].clone(), r[6].clone(), r[8].clone()])
            .collect();
        let want = |node: NodeId, before: bool, phase: u8| {
            [
                serde_json::json!(node),
                serde_json::json!(before),
                serde_json::json!(phase),
                serde_json::json!(true),
            ]
        };
        assert_eq!(rows, [want(parent, false, 3), want(outer, true, 3)]);
    }

    const PATHS: &str = "globalThis.seen = []; \
        globalThis.__inca_callbacks__ = { 0: (e) => { \
            globalThis.seen.push([e.target, e.currentTarget, e.eventPhase, \
                e.composedPath()]); globalThis.kept = e; } };";

    fn tree_of_three(host: &Rc<RefCell<Host>>) -> [NodeId; 3] {
        let mut h = host.borrow_mut();
        let outer = h.tree.create_node("div").unwrap();
        let parent = h.tree.create_node("div").unwrap();
        let child = h.tree.create_node("div").unwrap();
        h.tree.append_child(outer, parent).unwrap();
        h.tree.append_child(parent, child).unwrap();
        [outer, parent, child]
    }

    #[gpui::test]
    fn without_a_recorded_target_every_event_targets_its_own_node(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, _, child] = tree_of_three(&host);
        dispatcher.engine.eval::<()>(PATHS).unwrap();
        let events = [
            "mousedown",
            "mouseup",
            "mousemove",
            "wheel",
            "click",
            "keydown",
            "keyup",
            "focus",
            "blur",
            "mouseenter",
            "mouseleave",
        ];
        for event in events {
            listen(&host, child, event);
        }
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            for event in events {
                dispatcher.dispatch(child, event, &EventPayload::None, window, cx);
            }
        });
        let rows = seen(&dispatcher);
        assert_eq!(rows.len(), events.len());
        for row in rows {
            assert_eq!(row[0], serde_json::json!(child));
            assert_eq!(row[1], serde_json::json!(child));
            assert_eq!(row[2], serde_json::json!(2));
            assert_eq!(
                row[3].as_array().unwrap().last(),
                Some(&serde_json::json!(outer))
            );
        }
    }

    #[gpui::test]
    fn composed_path_lists_the_target_and_its_ancestors(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        dispatcher.engine.eval::<()>(PATHS).unwrap();
        listen(&host, parent, "click");
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.dispatch(parent, "click", &EventPayload::None, window, cx);
        });
        assert_eq!(seen(&dispatcher)[0][3], serde_json::json!([parent, outer]));
        assert_eq!(dispatcher.path_from(child), [child, parent, outer]);
        let after = dispatcher
            .engine
            .eval::<usize>("globalThis.kept.composedPath().length")
            .unwrap();
        assert_eq!(after, 0, "the path is empty once dispatch ends");
    }

    #[test]
    fn a_target_gone_from_the_tree_falls_back_to_the_current_target() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        assert_eq!(dispatcher.resolve(Some(child), parent), child);
        host.borrow_mut().tree.destroy_node(child);
        assert_eq!(dispatcher.resolve(Some(child), parent), parent);
        assert_eq!(dispatcher.resolve(None, parent), parent);
    }

    #[test]
    fn a_click_targets_the_nearest_common_ancestor_of_press_and_release() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        let other = {
            let mut h = host.borrow_mut();
            let other = h.tree.create_node("div").unwrap();
            h.tree.append_child(parent, other).unwrap();
            other
        };
        assert_eq!(dispatcher.common_ancestor(child, other), Some(parent));
        assert_eq!(dispatcher.common_ancestor(child, parent), Some(parent));
        assert_eq!(dispatcher.common_ancestor(child, child), Some(child));
        assert_eq!(dispatcher.common_ancestor(child, outer), Some(outer));
    }
}
