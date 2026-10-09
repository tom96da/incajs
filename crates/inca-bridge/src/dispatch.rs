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
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;

use gpui::{App, ScrollHandle, Window};
use inca_gpui::{
    EventKind, EventMask, EventPayload, KeyPayload, MousePayload, NodeId, PointerSource,
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

/// Where an event's `target` comes from.
#[derive(Clone, Copy)]
enum TargetRule {
    /// The node the event is dispatched for.
    Node,
    /// The deepest container under the pointer.
    Pointer,
    /// The nearest common ancestor of the press and release containers.
    PressRelease,
    /// The focused node.
    Focused,
}

/// `bubbles`, `cancelable` and `composed` of one event.
#[derive(Clone, Copy)]
struct Flags {
    bubbles: bool,
    cancelable: bool,
    composed: bool,
}

impl Flags {
    const ALL: Self = Self {
        bubbles: true,
        cancelable: true,
        composed: true,
    };
    const NONE: Self = Self {
        bubbles: false,
        cancelable: false,
        composed: false,
    };
    /// Focus events: composed and not cancelable.
    const FOCUS: Self = Self {
        composed: true,
        ..Self::NONE
    };
}

/// The rules of one event name.
#[derive(Clone, Copy)]
struct EventSpec {
    name: &'static str,
    flags: Flags,
    target: TargetRule,
    /// Takes `movementX`/`movementY` from the raw pointer move.
    movement: bool,
    /// Carries the `buttons` of its own press or release.
    own_buttons: bool,
}

const fn spec(name: &'static str, flags: Flags, target: TargetRule) -> EventSpec {
    EventSpec {
        name,
        flags,
        target,
        movement: false,
        own_buttons: false,
    }
}

const fn own_buttons(mut event: EventSpec) -> EventSpec {
    event.own_buttons = true;
    event
}

/// Every event the host dispatches.
const EVENTS: [EventSpec; 18] = [
    own_buttons(spec("click", Flags::ALL, TargetRule::PressRelease)),
    own_buttons(spec("dblclick", Flags::ALL, TargetRule::PressRelease)),
    own_buttons(spec("auxclick", Flags::ALL, TargetRule::PressRelease)),
    own_buttons(spec("contextmenu", Flags::ALL, TargetRule::Pointer)),
    spec("mousedown", Flags::ALL, TargetRule::Pointer),
    spec("mouseup", Flags::ALL, TargetRule::Pointer),
    EventSpec {
        movement: true,
        ..spec("mousemove", Flags::ALL, TargetRule::Pointer)
    },
    spec("mouseenter", Flags::NONE, TargetRule::Node),
    spec("mouseleave", Flags::NONE, TargetRule::Node),
    spec("mouseover", Flags::ALL, TargetRule::Node),
    spec("mouseout", Flags::ALL, TargetRule::Node),
    spec("wheel", Flags::ALL, TargetRule::Pointer),
    spec("keydown", Flags::ALL, TargetRule::Focused),
    spec("keyup", Flags::ALL, TargetRule::Focused),
    spec("focus", Flags::FOCUS, TargetRule::Node),
    spec("blur", Flags::FOCUS, TargetRule::Node),
    spec(
        "focusin",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
        TargetRule::Node,
    ),
    spec(
        "focusout",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
        TargetRule::Node,
    ),
];

/// The rules of `event`. Any other name takes all three flags as false.
fn spec_of(event: &str) -> EventSpec {
    EVENTS
        .iter()
        .find(|spec| spec.name == event)
        .copied()
        .unwrap_or(spec("", Flags::NONE, TargetRule::Node))
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
    /// The window's modifier keys as of the last change. `None` until
    /// [`Self::sync_modifiers`] first reads them.
    held_modifiers: Rc<Cell<Option<gpui::Modifiers>>>,
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
    /// The containers `gpui` reports as hovered.
    hovered: Rc<RefCell<BTreeSet<NodeId>>>,
    /// The deepest hovered container the hover events last reported and its
    /// ancestors, nearest first.
    hover_prev: Rc<RefCell<Vec<NodeId>>>,
    /// Whether a hover report is waiting for the end of the update.
    hover_scheduled: Rc<Cell<bool>>,
    /// Whether the update being handled already reported its deepest
    /// container.
    hover_settled: Rc<Cell<bool>>,
    /// The Space press waiting for its release: the window and the focused
    /// button. A mouse press or another key press ends it.
    space_down: Rc<Cell<Option<(gpui::WindowId, NodeId)>>>,
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
            held_modifiers: Rc::new(Cell::new(None)),
            wheel_stopped: Rc::new(Cell::new(false)),
            last_event_id: Rc::new(Cell::new(0)),
            current_events: Rc::new(RefCell::new(HashMap::new())),
            scroll_handles: Rc::new(RefCell::new(HashMap::new())),
            hovered: Rc::new(RefCell::new(BTreeSet::new())),
            hover_prev: Rc::new(RefCell::new(Vec::new())),
            hover_scheduled: Rc::new(Cell::new(false)),
            hover_settled: Rc::new(Cell::new(false)),
            space_down: Rc::new(Cell::new(None)),
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
        let mut payload = payload.clone();
        if let Some(mouse) = payload.mouse_mut().filter(|_| !spec_of(event).own_buttons) {
            mouse.buttons = self.update_held_buttons(event, mouse.button);
        }
        payload
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

    /// Returns `payload` with `movementX`/`movementY` set to the delta of the
    /// raw pointer move being dispatched, for `mousemove`. Every other event
    /// keeps 0.
    fn with_movement(&self, payload: &EventPayload, event: &str) -> EventPayload {
        let mut payload = payload.clone();
        if spec_of(event).movement
            && let EventPayload::Mouse(mouse) = &mut payload
        {
            let (movement_x, movement_y) = self.current_movement.get().unwrap_or((0.0, 0.0));
            mouse.movement_x = movement_x;
            mouse.movement_y = movement_y;
        }
        payload
    }

    /// Takes the window's current modifier keys as the baseline for the next
    /// change. With `force` unset, an existing baseline stays.
    pub fn sync_modifiers(&self, window: &Window, force: bool) {
        if force || self.held_modifiers.get().is_none() {
            self.held_modifiers.set(Some(window.modifiers()));
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
    /// the pointer for `mousedown`, `mouseup`, `mousemove`, `wheel` and
    /// `contextmenu`. It is the nearest common ancestor of the press and
    /// release containers for `click`, `dblclick` and `auxclick`. It is the
    /// focused node for `keydown` and `keyup`, which the root receives when
    /// nothing is focused. Every other event, and any of these
    /// when no target is recorded (a dispatch made outside input handling),
    /// uses `node_id`.
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
        let callback_ids = if (event == "wheel" && self.wheel_stopped.get())
            || self.cut_by_disabled_button(node_id, event, cx)
        {
            Vec::new()
        } else {
            self.host
                .borrow()
                .listeners
                .callbacks_for(node_id, event)
                .to_vec()
        };

        let payload = self.prepare(event, payload, window);
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

    /// Whether `node_id` is a disabled button, or an ancestor of one, on the
    /// path from the target of this `mousedown`, `mouseup` or `click` or from
    /// its pressed target. Listeners below the button run.
    fn cut_by_disabled_button(&self, node_id: NodeId, event: &str, cx: &App) -> bool {
        if !matches!(event, "mousedown" | "mouseup" | "click") {
            return false;
        }
        let host = self.host.borrow();
        let disabled = |id: &NodeId| {
            host.tree.get(*id).is_some_and(|node| {
                node.tag_name() == "button" && node.attributes().contains_key("disabled")
            })
        };
        [inca_gpui::mouse_target(cx), inca_gpui::pressed_target(cx)]
            .into_iter()
            .flatten()
            .any(|start| {
                let path = self.path_from(start);
                path.iter()
                    .position(disabled)
                    .is_some_and(|cut| path[cut..].contains(&node_id))
            })
    }

    /// `payload` as a listener sees it: a `related_target` that left the tree
    /// reads `None`, and the buttons, movement and screen fields are set.
    fn prepare(&self, event: &str, payload: &EventPayload, window: &Window) -> EventPayload {
        let payload = self.live_related(payload);
        let payload = self.with_held_buttons(event, &payload);
        let payload = self.with_movement(&payload, event);
        with_screen(&payload, window)
    }

    /// `payload` with a `related_target` that left the tree set to `None`.
    fn live_related(&self, payload: &EventPayload) -> EventPayload {
        let gone = |id: &NodeId| self.host.borrow().tree.get(*id).is_none();
        let mut payload = payload.clone();
        match &mut payload {
            EventPayload::Focus { related_target } => {
                if related_target.as_ref().is_some_and(gone) {
                    *related_target = None;
                }
            }
            other => {
                if let Some(mouse) = other.mouse_mut()
                    && mouse.related_target.as_ref().is_some_and(gone)
                {
                    mouse.related_target = None;
                }
            }
        }
        payload
    }

    /// The `target` of `event` dispatched for `node_id`.
    fn target_of(&self, node_id: NodeId, event: &str, window: &Window, cx: &App) -> NodeId {
        let found = match spec_of(event).target {
            TargetRule::Node => None,
            TargetRule::Pointer => inca_gpui::mouse_target(cx),
            TargetRule::PressRelease => {
                match (inca_gpui::pressed_target(cx), inca_gpui::mouse_target(cx)) {
                    (Some(down), Some(up)) => self.common_ancestor(down, up),
                    (_, up) => up,
                }
            }
            TargetRule::Focused => self.host.borrow().focus.focused_node(window, cx),
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

    /// Fires `event` at `start` with `start` as the `target`. The listeners
    /// of `start` run first, then those of each ancestor while the event
    /// bubbles, until one calls `stopPropagation()`. Returns whether a
    /// listener prevented the default action. A `start` that left the tree
    /// fires nothing. The job queue drains after the walk.
    pub fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if self.host.borrow().tree.get(start).is_none() {
            return false;
        }
        let payload = &self.prepare(event, payload, window);
        let bubbles = spec_of(event).flags.bubbles;
        let mut prevented = false;
        let mut current = Some(start);
        while let Some(id) = current {
            let callback_ids = self
                .host
                .borrow()
                .listeners
                .callbacks_for(id, event)
                .to_vec();
            if !callback_ids.is_empty() {
                let state = self.event_state(event, cx);
                let outcome = self.run_callbacks(id, start, event, payload, state, callback_ids);
                prevented |= outcome.prevent_default;
                if outcome.stop_propagation {
                    break;
                }
            }
            current = self
                .host
                .borrow()
                .tree
                .get(id)
                .filter(|_| bubbles)
                .and_then(inca_gpui::VirtualNode::parent);
        }
        self.drain_jobs_and_refresh(window);
        prevented
    }

    /// The deepest container `gpui` reports as hovered. Containers that left
    /// the tree drop out of the set.
    fn deepest_hovered(&self) -> Option<NodeId> {
        let mut set = self.hovered.borrow_mut();
        set.retain(|id| self.host.borrow().tree.get(*id).is_some());
        set.iter()
            .copied()
            .max_by_key(|id| (self.path_from(*id).len(), *id))
    }

    /// Queues the hover report for the end of the update, once. It reports the
    /// hovered set unless the update already reported its deepest container.
    fn schedule_hover_report(&self, window: &mut Window, cx: &mut App) {
        if self.hover_scheduled.replace(true) {
            return;
        }
        let this = self.clone();
        window.defer(cx, move |window, cx| {
            this.hover_scheduled.set(false);
            if !this.hover_settled.replace(false) {
                this.report_hover(this.deepest_hovered(), window, cx);
            }
        });
    }

    /// Fires the hover events for the pointer moving from the deepest
    /// container reported last time to the deepest one hovered now: `mouseout`
    /// at the old one, `mouseleave` at each container left (innermost first),
    /// `mouseover` at the new one and `mouseenter` at each container entered
    /// (outermost first).
    fn report_hover(&self, next: Option<NodeId>, window: &mut Window, cx: &mut App) {
        let next_path = next.map(|id| self.path_from(id)).unwrap_or_default();
        // A removed container is replaced by its nearest live ancestor.
        let old_path = self.hover_prev.replace(next_path.clone());
        // A container is live while it still hangs below the old path's top.
        let top = old_path.last().copied();
        let live = |id: &NodeId| self.path_from(*id).last().copied() == top;
        let removed = old_path.first().is_some_and(|id| !live(id));
        let old_path: Vec<_> = old_path.into_iter().skip_while(|id| !live(id)).collect();
        let prev = old_path.first().copied();
        if prev == next {
            return;
        }
        let (leave, enter) = hover_path_difference(&old_path, &next_path);
        let mouse = MousePayload::at(window.mouse_position(), window.modifiers());
        let with = |related_target| {
            EventPayload::Mouse(MousePayload {
                related_target,
                ..mouse
            })
        };
        // A removed container receives no `mouseout`.
        if let Some(old) = prev.filter(|_| !removed) {
            self.fire(old, "mouseout", &with(next), window, cx);
        }
        for id in leave {
            self.fire(id, "mouseleave", &with(next), window, cx);
        }
        if let Some(new) = next {
            self.fire(new, "mouseover", &with(prev), window, cx);
        }
        for id in enter {
            self.fire(id, "mouseenter", &with(prev), window, cx);
        }
    }

    /// [`drain_jobs_and_refresh`] with this dispatcher's engine and reporter.
    pub fn drain_jobs_and_refresh(&self, window: &mut Window) {
        drain_jobs_and_refresh(&self.engine, &self.reporter, window);
    }
}

/// Returns `payload` with `screenX`/`screenY` set to the window's position
/// on the screen plus the client position.
fn with_screen(payload: &EventPayload, window: &Window) -> EventPayload {
    let origin = window.bounds().origin;
    let mut payload = payload.clone();
    if let Some(mouse) = payload.mouse_mut() {
        mouse.screen_x = f32::from(origin.x) + mouse.client_x;
        mouse.screen_y = f32::from(origin.y) + mouse.client_y;
    }
    payload
}

fn set_flags(event_object: &Object, flags: Flags) -> rquickjs::Result<()> {
    event_object.set("bubbles", flags.bubbles)?;
    event_object.set("cancelable", flags.cancelable)?;
    event_object.set("composed", flags.composed)
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
    let Flags { cancelable, .. } = spec_of(event).flags;
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
    set_flags(&event_object, spec_of(event).flags)?;
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
        EventPayload::Focus { related_target } => match related_target {
            Some(id) => event_object.set("relatedTarget", *id)?,
            None => event_object.set("relatedTarget", rquickjs::Null)?,
        },
    }
    Ok(())
}

/// Writes [`MousePayload`]'s fields onto `event_object`, DOM-named — shared
/// by [`EventPayload::Mouse`] and [`EventPayload::Wheel`] (DOM's
/// `WheelEvent` extends `MouseEvent`).
fn set_mouse_fields(event_object: &Object, mouse: &MousePayload) -> rquickjs::Result<()> {
    event_object.set("clientX", mouse.client_x)?;
    event_object.set("clientY", mouse.client_y)?;
    event_object.set("screenX", mouse.screen_x)?;
    event_object.set("screenY", mouse.screen_y)?;
    event_object.set("x", mouse.client_x)?;
    event_object.set("y", mouse.client_y)?;
    match mouse.related_target {
        Some(id) => event_object.set("relatedTarget", id)?,
        None => event_object.set("relatedTarget", rquickjs::Null)?,
    }
    // Identical to clientX/clientY today — nothing here scrolls the page
    // itself, which is the only thing that would tell them apart.
    event_object.set("pageX", mouse.client_x)?;
    event_object.set("pageY", mouse.client_y)?;
    event_object.set("movementX", mouse.movement_x)?;
    event_object.set("movementY", mouse.movement_y)?;
    event_object.set("button", mouse.button)?;
    event_object.set("buttons", mouse.buttons)?;
    event_object.set("detail", mouse.detail)?;
    if let Some(source) = mouse.pointer {
        let (id, kind, primary) = match source {
            PointerSource::Mouse => (1, "mouse", true),
            PointerSource::Keyboard => (-1, "", false),
        };
        event_object.set("pointerId", id)?;
        event_object.set("pointerType", kind)?;
        event_object.set("isPrimary", primary)?;
        event_object.set("width", 1)?;
        event_object.set("height", 1)?;
        // Pointer events report 0.5 while a button is held.
        event_object.set("pressure", if mouse.buttons == 0 { 0.0 } else { 0.5 })?;
    }
    set_modifier_fields(event_object, mouse.modifiers)
}

/// Writes [`KeyPayload`]'s fields onto `event_object`, DOM-named.
fn set_key_fields(event_object: &Object, key: &KeyPayload) -> rquickjs::Result<()> {
    event_object.set("key", key.key.clone())?;
    event_object.set("repeat", key.repeat)?;
    // gpui reports no key location or composition state.
    event_object.set("location", 0)?;
    event_object.set("isComposing", false)?;
    set_modifier_fields(event_object, key.modifiers)
}

/// Writes `modifiers`' fields onto `event_object`, DOM-named — shared by
/// [`set_mouse_fields`] and [`set_key_fields`].
fn set_modifier_fields(event_object: &Object, modifiers: gpui::Modifiers) -> rquickjs::Result<()> {
    event_object.set("ctrlKey", modifiers.control)?;
    event_object.set("shiftKey", modifiers.shift)?;
    event_object.set("altKey", modifiers.alt)?;
    event_object.set("metaKey", modifiers.platform)?;
    event_object.set(
        "getModifierState",
        Function::new(event_object.ctx().clone(), move |name: String| {
            modifier_state(modifiers, &name)
        })?,
    )?;
    Ok(())
}

/// Whether the modifier `name` is held. `Accel` is Meta on macOS and Control
/// elsewhere. Other names return false.
fn modifier_state(modifiers: gpui::Modifiers, name: &str) -> bool {
    match name {
        "Control" => modifiers.control,
        "Shift" => modifiers.shift,
        "Alt" => modifiers.alt,
        "Meta" => modifiers.platform,
        "Accel" => {
            if cfg!(target_os = "macos") {
                modifiers.platform
            } else {
                modifiers.control
            }
        }
        _ => false,
    }
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

    fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.fire(start, event, payload, window, cx)
    }

    fn focus_handle(&self, node_id: NodeId) -> Option<gpui::FocusHandle> {
        let host = self.host.borrow();
        host.focus.handle(node_id).or_else(|| {
            (node_id == host.root)
                .then(|| host.focus.parked_handle())
                .flatten()
        })
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

    // Changed flags fire in the order Shift, Control, Alt, Meta.
    fn modifiers_changed(&self, modifiers: gpui::Modifiers, window: &mut Window, cx: &mut App) {
        let before = self
            .held_modifiers
            .replace(Some(modifiers))
            .unwrap_or(modifiers);
        let (root, focused) = {
            let host = self.host.borrow();
            (host.root, host.focus.focused_node(window, cx))
        };
        let start = self.resolve(focused, root);
        for (key, was, is) in [
            ("Shift", before.shift, modifiers.shift),
            ("Control", before.control, modifiers.control),
            ("Alt", before.alt, modifiers.alt),
            ("Meta", before.platform, modifiers.platform),
        ] {
            if was != is {
                let payload = EventPayload::Key(KeyPayload {
                    key: key.to_owned(),
                    repeat: false,
                    modifiers,
                });
                let event = if is { "keydown" } else { "keyup" };
                self.fire(start, event, &payload, window, cx);
            }
        }
    }

    fn pointer_left(&self) {
        self.last_position.set(None);
    }

    fn pointer_over(&self, node_id: NodeId, window: &mut Window, cx: &mut App) {
        self.hover_settled.set(true);
        self.report_hover(Some(node_id), window, cx);
        self.schedule_hover_report(window, cx);
    }

    fn pointer_pressed(&self) {
        self.space_down.set(None);
    }

    // Tab moves focus and dispatches nothing here. The next frame reports it.
    fn key_default(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let key = keystroke.key.as_str();
        let modifiers = keystroke.modifiers;
        let window_id = window.window_handle().window_id();
        let pending = if up {
            (key == "space").then(|| self.space_down.take()).flatten()
        } else {
            self.space_down.set(None);
            None
        };
        if window.default_prevented() {
            return;
        }
        let focused_button = || {
            let host = self.host.borrow();
            host.focus.focused_node(window, cx).filter(|id| {
                host.tree.get(*id).is_some_and(|node| {
                    node.tag_name() == "button" && !node.attributes().contains_key("disabled")
                })
            })
        };
        let click = match (key, up) {
            ("tab", false) if !(modifiers.control || modifiers.alt || modifiers.platform) => {
                let host = self.host.borrow();
                host.focus
                    .tab_move(&host.tree, host.root, window, cx, modifiers.shift);
                None
            }
            ("enter", false) if !(modifiers.control || modifiers.alt || modifiers.platform) => {
                focused_button()
            }
            ("space", false) => {
                self.space_down
                    .set(focused_button().map(|button| (window_id, button)));
                None
            }
            ("space", true) => focused_button().filter(|b| pending == Some((window_id, *b))),
            _ => None,
        };
        if let Some(button) = click {
            let payload = EventPayload::Mouse(MousePayload::keyboard_click(modifiers));
            self.fire(button, "click", &payload, window, cx);
        }
    }

    fn hover_changed(&self, node_id: NodeId, hovered: bool, window: &mut Window, cx: &mut App) {
        {
            let mut set = self.hovered.borrow_mut();
            if hovered {
                set.insert(node_id);
            } else {
                set.remove(&node_id);
            }
        }
        self.schedule_hover_report(window, cx);
    }
}

/// The containers a pointer leaves and enters when its deepest container
/// changes from the end of `old_path` to the end of `new_path`. Each path
/// lists a container and its ancestors, nearest first. The first list holds
/// the containers only `old_path` has, innermost first. The second holds the
/// ones only `new_path` has, outermost first.
fn hover_path_difference(old_path: &[NodeId], new_path: &[NodeId]) -> (Vec<NodeId>, Vec<NodeId>) {
    let leave = old_path
        .iter()
        .copied()
        .filter(|id| !new_path.contains(id))
        .collect();
    let enter = new_path
        .iter()
        .rev()
        .copied()
        .filter(|id| !old_path.contains(id))
        .collect();
    (leave, enter)
}

/// Runs `engine`'s pending promise jobs and sends each failure to `reporter`:
/// a job that threw, including during module startup, and a promise rejected
/// with no handler. Use it where no window is open yet.
///
/// Call it outside any [`Engine::with`]: it takes the context itself, and
/// nesting that panics with "`RefCell` already borrowed".
pub fn report_jobs(engine: &Engine, reporter: &ErrorReporter) {
    for failure in engine.run_jobs() {
        reporter(&failure);
    }
}

/// [`report_jobs`], then requests a redraw. Code that only schedules work (a
/// `@vue/runtime-core` reactivity effect, batched via a microtask) hasn't
/// mutated the tree once the scheduling call returns, so the queue has to run
/// before the frame is drawn.
///
/// Call it outside any [`Engine::with`].
pub fn drain_jobs_and_refresh(engine: &Engine, reporter: &ErrorReporter, window: &mut Window) {
    report_jobs(engine, reporter);
    window.refresh();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bindings::install;
    use gpui::{TestAppContext, point, px};
    use inca_gpui::WheelPayload;

    #[test]
    fn modifier_state_reports_each_held_modifier_and_no_lock() {
        let mut mismatches = Vec::new();
        for (name, held) in [
            ("Control", gpui::Modifiers::control()),
            ("Shift", gpui::Modifiers::shift()),
            ("Alt", gpui::Modifiers::alt()),
            ("Meta", gpui::Modifiers::command()),
        ] {
            for other in ["Control", "Shift", "Alt", "Meta", "CapsLock", "NumLock"] {
                if modifier_state(held, other) != (other == name) {
                    mismatches.push(format!("{name} held, {other} queried"));
                }
            }
        }
        let accel = if cfg!(target_os = "macos") {
            gpui::Modifiers::command()
        } else {
            gpui::Modifiers::control()
        };
        if !modifier_state(accel, "Accel") || modifier_state(gpui::Modifiers::default(), "Accel") {
            mismatches.push("Accel".to_string());
        }
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }

    /// Every failure a test reporter received, in order.
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

    #[test]
    fn report_jobs_delivers_a_failed_job_once() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>("Promise.resolve().then(() => { throw new Error('boom'); });")
            .unwrap();
        let reported: Reported = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&reported);
        let report: ErrorReporter = Rc::new(move |err| sink.borrow_mut().push(err.clone()));

        report_jobs(&engine, &report);
        assert_eq!(reported.borrow().len(), 1);
        assert_eq!(reported.borrow()[0].message(), "Error: boom");

        report_jobs(&engine, &report);
        assert_eq!(reported.borrow().len(), 1);
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
    fn only_mousemove_takes_the_movement_of_the_raw_move(cx: &mut TestAppContext) {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();
        let cx = cx.add_empty_window();
        cx.update(|_, cx| dispatcher.pointer_moved(at(10.0, 10.0), cx));

        let (moved, others) = cx.update(|_, cx| {
            dispatcher.pointer_moved(at(30.0, 5.0), cx);
            let moved = dispatcher.with_movement(&mouse_at(30.0, 5.0), "mousemove");
            let others = [
                "mouseenter",
                "mouseleave",
                "mouseover",
                "mouseout",
                "mousedown",
                "mouseup",
            ]
            .map(|event| movement_of(&dispatcher.with_movement(&mouse_at(30.0, 5.0), event)));
            let wheel = dispatcher.with_movement(&wheel_at(30.0, 5.0), "wheel");
            (movement_of(&moved), (others, movement_of(&wheel)))
        });

        assert_eq!(moved, (20.0, -5.0));
        assert_eq!(others, ([(0.0, 0.0); 6], (0.0, 0.0)));
    }

    #[test]
    fn a_mousemove_with_no_raw_move_reports_zero() {
        let (dispatcher, _host, _reported) = dispatcher_with_engine();

        let payload = dispatcher.with_movement(&mouse_at(90.0, 90.0), "mousemove");

        assert_eq!(movement_of(&payload), (0.0, 0.0));
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

    fn seen(dispatcher: &EventDispatcher) -> serde_json::Value {
        let json = dispatcher
            .engine
            .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
            .unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn seen_rows(dispatcher: &EventDispatcher) -> Vec<Vec<serde_json::Value>> {
        serde_json::from_value(seen(dispatcher)).unwrap()
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
            ("dblclick", true, true, true),
            ("auxclick", true, true, true),
            ("contextmenu", true, true, true),
            ("mouseenter", false, false, false),
            ("mouseleave", false, false, false),
            ("mouseover", true, true, true),
            ("mouseout", true, true, true),
            ("wheel", true, true, true),
            ("keydown", true, true, true),
            ("focus", false, false, true),
            ("blur", false, false, true),
            ("focusin", true, false, true),
            ("focusout", true, false, true),
        ] {
            listen(&host, node, event);
            cx.update(|window, cx| {
                dispatcher.dispatch(node, event, &EventPayload::None, window, cx);
            });
            let rows = seen_rows(&dispatcher);
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
        let rows = seen_rows(&dispatcher);
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
        for row in seen_rows(&dispatcher) {
            assert_eq!(row[4], serde_json::json!(false));
            assert_eq!(row[5], serde_json::json!(false));
            assert_eq!(row[6], serde_json::json!(2), "focus is non-bubbling");
        }
    }

    #[gpui::test]
    fn fire_walks_up_only_bubbling_events_and_reports_prevention(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let parent = host.borrow_mut().tree.create_node("div").unwrap();
        let child = host.borrow_mut().tree.create_node("div").unwrap();
        host.borrow_mut().tree.append_child(parent, child).unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.__inca_callbacks__[1] = (e) => { \
                 globalThis.__inca_callbacks__[0](e); e.stopPropagation(); };",
            )
            .unwrap();
        let cx = cx.add_empty_window();
        // (event, start, callback of start, listening nodes, nodes reached, prevented)
        #[allow(clippy::type_complexity)]
        let rows: [(&str, NodeId, u32, &[NodeId], &[NodeId], bool); 8] = [
            ("click", child, 0, &[], &[], false),
            ("click", child, 0, &[child, parent], &[child, parent], true),
            ("click", child, 1, &[child, parent], &[child], true),
            ("contextmenu", child, 0, &[child], &[child], true),
            ("keydown", child, 0, &[child], &[child], true),
            ("mouseenter", child, 0, &[child, parent], &[child], false),
            ("focusin", child, 0, &[child], &[child], false),
            ("click", 99, 0, &[99, parent], &[], false),
        ];
        for (event, start, callback, listening, reached, prevented) in rows {
            host.borrow_mut().listeners = crate::bindings::EventListeners::default();
            for node in listening {
                host.borrow_mut().listeners.register(*node, event, callback);
            }
            let got = cx.update(|window, cx| {
                dispatcher.fire(start, event, &EventPayload::None, window, cx)
            });
            let nodes: Vec<_> = seen_rows(&dispatcher)
                .iter()
                .map(|r| r[7].clone())
                .collect();
            assert_eq!(got, prevented, "{event} from {start}: prevented");
            assert_eq!(nodes, reached, "{event} from {start}: nodes reached");
        }
    }

    #[test]
    fn every_event_kind_has_a_table_row() {
        for kind in EventKind::ALL {
            assert!(
                EVENTS.iter().any(|spec| spec.name == kind.name()),
                "{} has a row",
                kind.name()
            );
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
        let stamps: Vec<f64> = seen_rows(&dispatcher)
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
            dispatcher.fire(
                button,
                "click",
                &EventPayload::Mouse(MousePayload::keyboard_click(gpui::Modifiers::none())),
                window,
                cx,
            );
        });
        // [currentTarget, defaultPrevented before, eventPhase, isTrusted]
        let rows: Vec<_> = seen_rows(&dispatcher)
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
            "mouseover",
            "mouseout",
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
        let rows = seen_rows(&dispatcher);
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
        assert_eq!(
            seen_rows(&dispatcher)[0][3],
            serde_json::json!([parent, outer])
        );
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

    /// Every mouse-shaped event carries the `x`/`y` aliases and a null
    /// `relatedTarget`. A payload with a pointer source adds the pointer
    /// fields.
    #[gpui::test]
    fn mouse_events_carry_the_aliases_and_the_pointer_fields_of_their_type(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.seen = []; globalThis.__inca_callbacks__ = { 0: (e) => { \
                 globalThis.seen.push([e.type, e.x === e.clientX && e.y === e.clientY, \
                 e.relatedTarget === null, e.pointerId, e.pointerType, e.isPrimary, \
                 e.width, e.height, e.pressure].join(':')); } };",
            )
            .unwrap();
        let cx = cx.add_empty_window();
        let plain = || MousePayload::at(at(7.0, 9.0), gpui::Modifiers::none());
        let pointer = |source| MousePayload {
            pointer: Some(source),
            ..plain()
        };
        let wheel = WheelPayload {
            mouse: plain(),
            delta_x: 0.0,
            delta_y: 0.0,
            delta_z: 0.0,
            delta_mode: 0,
        };
        let blank = ":::::";
        let mut cases = vec![
            (
                "click",
                EventPayload::Mouse(pointer(PointerSource::Keyboard)),
                "-1::false:1:1:0",
            ),
            ("wheel", EventPayload::Wheel(wheel), blank),
        ];
        for event in ["click", "auxclick", "contextmenu"] {
            let payload = EventPayload::Mouse(pointer(PointerSource::Mouse));
            cases.push((event, payload, "1:mouse:true:1:1:0"));
        }
        for event in [
            "mousedown",
            "mouseup",
            "mousemove",
            "mouseenter",
            "mouseleave",
            "mouseover",
            "mouseout",
            "dblclick",
        ] {
            cases.push((event, EventPayload::Mouse(plain()), blank));
        }
        let mut mismatches = Vec::new();
        for (event, payload, fields) in cases {
            listen(&host, node, event);
            cx.update(|window, cx| dispatcher.dispatch(node, event, &payload, window, cx));
            let want = format!("{event}:true:true:{fields}");
            let got = seen(&dispatcher);
            if got != serde_json::json!([want]) {
                mismatches.push(format!("{event}: {got:?} != {want}"));
            }
        }
        assert!(mismatches.is_empty(), "{mismatches:?}");
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

    #[test]
    fn the_hover_path_difference_lists_the_containers_left_and_entered() {
        // Paths run nearest first: 3 sits in 2, which sits in 1, then 0.
        let nested = [3, 2, 1, 0];
        let cases: [[&[NodeId]; 4]; 5] = [
            // into a nested container
            [&[0], &nested, &[], &[1, 2, 3]],
            // to a sibling inside the same parent
            [&nested, &[4, 2, 1, 0], &[3], &[4]],
            // out of everything
            [&nested, &[], &[3, 2, 1, 0], &[]],
            // within the same container
            [&nested, &nested, &[], &[]],
            // the first move after entering the window
            [&[], &nested, &[], &[0, 1, 2, 3]],
        ];
        for [old, new, leave, enter] in cases {
            assert_eq!(
                hover_path_difference(old, new),
                (leave.to_vec(), enter.to_vec()),
                "{old:?} -> {new:?}"
            );
        }
    }

    #[gpui::test]
    fn a_removed_node_is_no_related_target_and_no_previous_container(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        {
            let mut host = host.borrow_mut();
            let root = host.root;
            host.tree.append_child(root, outer).unwrap();
        }
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.seen = []; globalThis.__inca_callbacks__ = { 0: (e) => { \
                 globalThis.seen.push([e.type, e.relatedTarget]); } };",
            )
            .unwrap();
        listen(&host, outer, "mouseover");
        listen(&host, outer, "mouseout");
        listen(&host, outer, "focus");
        let cx = cx.add_empty_window();
        let at = |related_target| {
            EventPayload::Mouse(MousePayload {
                related_target,
                ..MousePayload::at(gpui::Point::default(), gpui::Modifiers::none())
            })
        };

        cx.update(|window, cx| {
            dispatcher.dispatch(outer, "mouseover", &at(Some(parent)), window, cx);
            dispatcher.dispatch(outer, "mouseover", &at(Some(99)), window, cx);
            for related_target in [Some(parent), Some(99), None] {
                let payload = EventPayload::Focus { related_target };
                dispatcher.dispatch(outer, "focus", &payload, window, cx);
            }
        });
        assert_eq!(
            seen(&dispatcher),
            serde_json::json!([
                ["mouseover", parent],
                ["mouseover", null],
                ["focus", parent],
                ["focus", null],
                ["focus", null]
            ])
        );

        cx.update(|window, cx| dispatcher.report_hover(Some(child), window, cx));
        seen(&dispatcher);
        host.borrow_mut().tree.destroy_node(child);
        cx.update(|window, cx| dispatcher.report_hover(Some(parent), window, cx));
        assert_eq!(seen(&dispatcher), serde_json::json!([]));
        cx.update(|window, cx| dispatcher.report_hover(Some(outer), window, cx));
        assert_eq!(
            seen(&dispatcher),
            serde_json::json!([["mouseout", outer], ["mouseover", parent]])
        );

        // A detached deepest container gets no mouseout.
        cx.update(|window, cx| dispatcher.report_hover(Some(parent), window, cx));
        seen(&dispatcher);
        host.borrow_mut().tree.remove_child(outer, parent).unwrap();
        cx.update(|window, cx| dispatcher.report_hover(None, window, cx));
        assert_eq!(seen(&dispatcher), serde_json::json!([]));
    }
}
