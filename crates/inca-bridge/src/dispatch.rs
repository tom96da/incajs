// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Turns the native input of a window into events and runs the JS listeners
//! registered for them via `__inca_native__.addEventListener` (see
//! [`crate::bindings`]), then requests a redraw.
//!
//! An event only enters the JS engine when a node on its path listens for it.
//! Only an event that entered the engine drains the job queue and requests a
//! redraw. Rendering runs JS only when input arrives.
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
//! `addEventListener` with that id. The walk of an event looks the real
//! function up fresh inside one `Engine::with` call and drops it before that
//! call returns — it never crosses into Rust-held state.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Instant;

use gpui::{App, MouseButton, MouseUpEvent, ScrollHandle, Window};
use inca_gpui::{EventPayload, KeyPayload, MousePayload, NodeId, PointerSource, dom_buttons_bit};
use inca_jsenv::{Engine, EngineError};

use inca_gpui::EventSink;

use crate::bindings::Host;
use crate::event::{self, Flags, Walk};

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

/// The rules of one event name.
#[derive(Clone, Copy)]
struct EventSpec {
    name: &'static str,
    flags: Flags,
    /// Takes `movementX`/`movementY` from the raw pointer move.
    movement: bool,
    /// Carries the `buttons` of its own press or release.
    own_buttons: bool,
}

const fn spec(name: &'static str, flags: Flags) -> EventSpec {
    EventSpec {
        name,
        flags,
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
    own_buttons(spec("click", Flags::ALL)),
    own_buttons(spec("dblclick", Flags::ALL)),
    own_buttons(spec("auxclick", Flags::ALL)),
    own_buttons(spec("contextmenu", Flags::ALL)),
    spec("mousedown", Flags::ALL),
    spec("mouseup", Flags::ALL),
    EventSpec {
        movement: true,
        ..spec("mousemove", Flags::ALL)
    },
    spec("mouseenter", Flags::NONE),
    spec("mouseleave", Flags::NONE),
    spec("mouseover", Flags::ALL),
    spec("mouseout", Flags::ALL),
    spec("wheel", Flags::ALL),
    spec("keydown", Flags::ALL),
    spec("keyup", Flags::ALL),
    spec("focus", Flags::FOCUS),
    spec("blur", Flags::FOCUS),
    spec(
        "focusin",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
    spec(
        "focusout",
        Flags {
            bubbles: true,
            ..Flags::FOCUS
        },
    ),
];

/// The rules of `event`. Any other name takes all three flags as false.
fn spec_of(event: &str) -> EventSpec {
    EVENTS
        .iter()
        .find(|spec| spec.name == event)
        .copied()
        .unwrap_or(spec("", Flags::NONE))
}

/// The origin of every `timeStamp`.
fn time_origin() -> Instant {
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    *ORIGIN.get_or_init(Instant::now)
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

    /// Whether the containers report the pointer: a listener is registered
    /// or a node can take focus. The render path asks once per frame.
    #[must_use]
    pub fn tracks_pointer(&self) -> bool {
        let host = self.host.borrow();
        !host.listeners.is_empty() || host.focus.has_handles()
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

    /// Dispatches `event` along the path of `target`, with `target` as its
    /// target, and reports what the listeners threw. Returns whether a
    /// listener prevented the default action and whether the walk entered the
    /// engine. `cut` lists the nodes whose listeners stay silent.
    fn walk(
        &self,
        target: NodeId,
        event: &str,
        payload: &EventPayload,
        cut: &[NodeId],
    ) -> (bool, bool) {
        let path = self.path_from(target);
        let outcome = event::walk(
            &self.engine,
            &self.host,
            &Walk {
                name: event,
                flags: spec_of(event).flags,
                target,
                path: &path,
                cut,
                payload,
                time_stamp: time_origin().elapsed().as_secs_f64() * 1000.0,
            },
        );
        // Reported after the walk: a reporter is free to do anything, and
        // re-entering the engine from inside `Engine::with` panics.
        for failure in &outcome.failures {
            (self.reporter)(failure);
        }
        (outcome.canceled, outcome.entered)
    }

    /// The nodes that receive no pointer `mousedown`, `mouseup`, `click` or
    /// `dblclick`: a disabled button on the path from the mouse target or the
    /// pressed target of the event, and the ancestors of that button. Listeners
    /// below the button run. A keyboard click always runs.
    fn cut_by_disabled_button(&self, event: &str, payload: &EventPayload, cx: &App) -> Vec<NodeId> {
        let keyboard = matches!(
            payload,
            EventPayload::Mouse(MousePayload {
                pointer: Some(PointerSource::Keyboard),
                ..
            })
        );
        if keyboard || !matches!(event, "mousedown" | "mouseup" | "click" | "dblclick") {
            return Vec::new();
        }
        let host = self.host.borrow();
        let disabled = |id: &NodeId| {
            host.tree.get(*id).is_some_and(|node| {
                node.tag_name() == "button" && node.attributes().contains_key("disabled")
            })
        };
        let button = match payload {
            EventPayload::Mouse(mouse) => Some(mouse.button),
            _ => None,
        };
        let pressed = button.and_then(|button| inca_gpui::pressed_target(cx, button));
        let mut cut = Vec::new();
        for start in [inca_gpui::mouse_target(cx), pressed].into_iter().flatten() {
            let path = self.path_from(start);
            if let Some(button) = path.iter().position(disabled) {
                cut.extend_from_slice(&path[button..]);
            }
        }
        cut
    }

    /// The default action of a `mousedown` that reached `target`: the nearest
    /// node on its path that can take focus takes it, else the window blurs.
    /// A prevented `mousedown` leaves the focus alone.
    fn focus_on_press(&self, target: NodeId, prevented: bool, window: &mut Window, cx: &mut App) {
        if prevented {
            return;
        }
        let path = self.path_from(target);
        let handle = {
            let host = self.host.borrow();
            path.iter().find_map(|id| host.focus.handle(*id))
        };
        match handle {
            Some(handle) => handle.focus(window, cx),
            None => window.blur(cx),
        }
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

    /// Fires `event` at `start` with `start` as the `target`. The listeners
    /// of `start` run first, then those of each ancestor while the event
    /// bubbles, until one calls `stopPropagation()`. Every listener receives
    /// the same event object, whose `eventId` is larger than the one of the
    /// event fired before. Returns whether a listener prevented the default
    /// action. A pointer `click` or `dblclick` skips a disabled button and its
    /// ancestors, as `mousedown` and `mouseup` do. A `start` that left the tree
    /// fires nothing. When a listener ran, the job queue drains after the walk
    /// and the window redraws.
    ///
    /// While the listeners run, `eventPhase` is 2 where `currentTarget` is
    /// `target` and 3 elsewhere, and `composedPath()` lists the node ids from
    /// `target` up to the root. Afterwards `currentTarget` is `null`,
    /// `eventPhase` is 0 and `composedPath()` is empty. A listener that throws
    /// is reported, and the remaining listeners run.
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
        let cut = self.cut_by_disabled_button(event, payload, cx);
        let (canceled, entered) = self.walk(start, event, payload, &cut);
        if entered {
            self.drain_jobs_and_refresh(window);
        }
        canceled
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

impl EventSink for EventDispatcher {
    fn tracks_pointer(&self) -> bool {
        self.tracks_pointer()
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

    fn pointer_released(
        &self,
        event: &MouseUpEvent,
        pressed: Option<NodeId>,
        released: NodeId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(pressed) = pressed else {
            return;
        };
        let Some(start) = self.common_ancestor(pressed, released) else {
            return;
        };
        let mut payload = EventPayload::click(event);
        if event.button != MouseButton::Left {
            self.fire(start, "auxclick", &payload, window, cx);
            return;
        }
        self.fire(start, "click", &payload, window, cx);
        if event.click_count == 2 {
            // `dblclick` carries the mouse fields only.
            if let Some(mouse) = payload.mouse_mut() {
                mouse.pointer = None;
            }
            self.fire(start, "dblclick", &payload, window, cx);
        }
    }

    fn pointer_input(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        let payload = self.prepare(event, payload, window);
        let cut = self.cut_by_disabled_button(event, &payload, cx);
        let (canceled, entered) = self.walk(target, event, &payload, &cut);
        // The default action reads the tree the listeners' queued jobs leave.
        if entered {
            self.drain_jobs_and_refresh(window);
        }
        match event {
            "mousedown" => self.focus_on_press(target, canceled, window, cx),
            // The scroll rollback of `inca-gpui` reads the flag.
            "wheel" if canceled => window.prevent_default(),
            _ => {}
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

    fn key_input(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (root, focused) = {
            let host = self.host.borrow();
            (host.root, host.focus.focused_node(window, cx))
        };
        let target = self.resolve(focused, root);
        let event = if up { "keyup" } else { "keydown" };
        let (canceled, entered) = self.walk(target, event, payload, &[]);
        if entered {
            self.drain_jobs_and_refresh(window);
        }
        if canceled {
            // Handled for the platform: a shortcut or an input method sees nothing.
            window.prevent_default();
            cx.stop_propagation();
        }
        self.key_default(keystroke, up, canceled, window, cx);
    }
}

impl EventDispatcher {
    // Tab moves focus and dispatches nothing here. The next frame reports it.
    fn key_default(
        &self,
        keystroke: &gpui::Keystroke,
        up: bool,
        canceled: bool,
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
        if canceled {
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
            dispatcher.fire(node_id, "click", &EventPayload::None, window, cx);
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
        host.borrow_mut().listeners.register(node_id, "click", 0);
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.__inca_callbacks__ = { 0: () => { throw new Error('boom'); } };",
            )
            .unwrap();

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
            dispatcher.fire(node_id, "click", &EventPayload::None, window, cx);
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
        let rows = [
            ("click", true, true, true),
            ("dblclick", true, true, true),
            ("auxclick", true, true, true),
            ("contextmenu", true, true, true),
            ("mouseenter", false, false, false),
            ("mouseleave", false, false, false),
            ("mouseover", true, true, true),
            ("mouseout", true, true, true),
            ("mousedown", true, true, true),
            ("mouseup", true, true, true),
            ("mousemove", true, true, true),
            ("wheel", true, true, true),
            ("keydown", true, true, true),
            ("keyup", true, true, true),
            ("focus", false, false, true),
            ("blur", false, false, true),
            ("focusin", true, false, true),
            ("focusout", true, false, true),
        ];
        assert_eq!(rows.len(), EVENTS.len(), "every EVENTS row has a test row");
        for (event, bubbles, cancelable, composed) in rows {
            listen(&host, node, event);
            cx.update(|window, cx| {
                dispatcher.fire(node, event, &EventPayload::None, window, cx);
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
            dispatcher.fire(node, "menu:1", &EventPayload::None, window, cx);
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

    #[gpui::test]
    fn time_stamp_counts_milliseconds_from_the_host_start_and_grows(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        listen(&host, node, "click");
        let cx = cx.add_empty_window();
        for _ in 0..3 {
            cx.update(|window, cx| {
                dispatcher.fire(node, "click", &EventPayload::None, window, cx);
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
                "(() => { const p = e.preventDefault; try { p(); } catch (err) { return `${err instanceof TypeError}:${e.defaultPrevented}`; } return 'ran'; })()",
                "true:false",
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
                dispatcher.fire(node, "click", &EventPayload::None, window, cx);
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
    fn every_event_fired_at_a_node_targets_that_node(cx: &mut TestAppContext) {
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
                dispatcher.fire(child, event, &EventPayload::None, window, cx);
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
            dispatcher.fire(parent, "click", &EventPayload::None, window, cx);
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
            cx.update(|window, cx| dispatcher.fire(node, event, &payload, window, cx));
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

    #[gpui::test]
    fn a_release_clicks_only_when_both_containers_share_an_ancestor(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, a] = tree_of_three(&host);
        let (b, apart) = {
            let mut h = host.borrow_mut();
            let b = h.tree.create_node("div").unwrap();
            h.tree.append_child(parent, b).unwrap();
            (b, h.tree.create_node("div").unwrap())
        };
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        for node in [parent, a, b, apart] {
            listen(&host, node, "click");
        }
        let release = gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        };
        let cx = cx.add_empty_window();
        let mut clicked = |pressed, released| {
            cx.update(|window, cx| {
                dispatcher.pointer_released(&release, pressed, released, window, cx);
            });
            seen_rows(&dispatcher).len()
        };

        assert_eq!(clicked(Some(a), b), 1, "siblings share the parent");
        assert_eq!(clicked(Some(a), apart), 0, "no common ancestor");
        assert_eq!(clicked(None, a), 0, "no press container");

        host.borrow_mut().tree.destroy_node(b);
        assert_eq!(clicked(Some(a), b), 0, "the release node is gone");
        assert_eq!(clicked(Some(b), a), 0, "the press node is gone");
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
            dispatcher.fire(outer, "mouseover", &at(Some(parent)), window, cx);
            dispatcher.fire(outer, "mouseover", &at(Some(99)), window, cx);
            for related_target in [Some(parent), Some(99), None] {
                let payload = EventPayload::Focus { related_target };
                dispatcher.fire(outer, "focus", &payload, window, cx);
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

    /// Fires `event` at `start` once and returns `globalThis.out` as JSON.
    fn fire_and_read(
        cx: &mut TestAppContext,
        dispatcher: &EventDispatcher,
        start: NodeId,
        event: &str,
    ) -> serde_json::Value {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.fire(start, event, &EventPayload::None, window, cx);
        });
        let json = dispatcher
            .engine
            .eval::<String>("JSON.stringify(globalThis.out)")
            .unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn callbacks(dispatcher: &EventDispatcher, js: &str) {
        dispatcher
            .engine
            .eval::<()>(&format!(
                "globalThis.out = []; globalThis.__inca_callbacks__ = {{ {js} }};"
            ))
            .unwrap();
    }

    #[gpui::test]
    fn one_event_object_reaches_every_node(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        for node in [child, parent, outer] {
            listen(&host, node, "click");
        }
        callbacks(
            &dispatcher,
            "0: (e) => { globalThis.first ??= e; e.seen = (e.seen ?? 0) + 1; \
                out.push([e === first, e.seen]); }",
        );

        let out = fire_and_read(cx, &dispatcher, child, "click");

        assert_eq!(out, serde_json::json!([[true, 1], [true, 2], [true, 3]]));
    }

    #[gpui::test]
    fn after_dispatch_current_target_is_null_phase_0_and_stop_flags_clear(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        listen(&host, child, "click");
        callbacks(
            &dispatcher,
            "0: (e) => { globalThis.kept = e; e.stopPropagation(); e.preventDefault(); \
                out.push([e.currentTarget, e.eventPhase, e.cancelBubble]); }",
        );
        let during = fire_and_read(cx, &dispatcher, child, "click");
        assert_eq!(during, serde_json::json!([[child, 2, true]]));

        let after = dispatcher
            .engine
            .eval::<String>(
                "JSON.stringify([kept.currentTarget, kept.eventPhase, kept.cancelBubble, \
                 kept.defaultPrevented, kept.composedPath().length])",
            )
            .unwrap();
        assert_eq!(after, "[null,0,false,true,0]");
    }

    #[gpui::test]
    fn the_path_is_fixed_when_dispatch_starts(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        for node in [child, parent, outer] {
            listen(&host, node, "click");
        }
        callbacks(
            &dispatcher,
            &format!(
                "0: (e) => {{ out.push([e.currentTarget, e.composedPath()]); \
                    if (e.currentTarget === {child}) \
                        __inca_native__.removeChild({parent}, {child}); }}"
            ),
        );

        let out = fire_and_read(cx, &dispatcher, child, "click");

        let path = serde_json::json!([child, parent, outer]);
        assert_eq!(
            out,
            serde_json::json!([[child, path], [parent, path], [outer, path]])
        );
    }

    #[gpui::test]
    fn a_listener_removed_mid_dispatch_does_not_run(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        for (node, id) in [(child, 0), (child, 1), (parent, 2)] {
            host.borrow_mut().listeners.register(node, "click", id);
        }
        callbacks(
            &dispatcher,
            &format!(
                "0: () => {{ out.push(0); \
                    __inca_native__.removeEventListener({child}, 'click', 1); \
                    __inca_native__.removeEventListener({parent}, 'click', 2); }}, \
                 1: () => out.push(1), \
                 2: () => out.push(2)"
            ),
        );

        let out = fire_and_read(cx, &dispatcher, child, "click");

        assert_eq!(out, serde_json::json!([0]));
    }

    #[gpui::test]
    fn a_listener_added_to_an_ancestor_mid_dispatch_runs_and_one_added_to_the_node_waits(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        host.borrow_mut().listeners.register(child, "click", 0);
        host.borrow_mut().listeners.register(parent, "click", 1);
        callbacks(
            &dispatcher,
            &format!(
                "0: () => {{ out.push(0); \
                    __inca_native__.addEventListener({child}, 'click', 2); \
                    __inca_native__.addEventListener({parent}, 'click', 3); }}, \
                 1: () => out.push(1), 2: () => out.push(2), 3: () => out.push(3)"
            ),
        );

        let out = fire_and_read(cx, &dispatcher, child, "click");

        assert_eq!(out, serde_json::json!([0, 1, 3]));
    }

    #[gpui::test]
    fn each_dispatch_takes_the_next_event_id_and_an_unheard_one_takes_none(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(&host, parent, "click");
        listen(&host, parent, "focusin");
        callbacks(&dispatcher, "0: (e) => out.push([e.type, e.eventId])");
        let cx = cx.add_empty_window();

        cx.update(|window, cx| {
            let none = EventPayload::None;
            dispatcher.fire(child, "click", &none, window, cx);
            dispatcher.fire(child, "keydown", &none, window, cx);
            dispatcher.fire(child, "click", &none, window, cx);
            dispatcher.fire(child, "focusin", &none, window, cx);
        });

        let out = dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap();
        assert_eq!(out, r#"[["click",1],["click",2],["focusin",3]]"#);
    }

    #[gpui::test]
    fn stop_propagation_keeps_the_nodes_other_listeners_and_stop_immediate_does_not(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        for (node, id) in [(child, 0), (child, 1), (parent, 2)] {
            host.borrow_mut().listeners.register(node, "click", id);
        }
        callbacks(
            &dispatcher,
            "0: (e) => { out.push(0); globalThis.immediate ? \
                    e.stopImmediatePropagation() : e.stopPropagation(); }, \
             1: () => out.push(1), 2: () => out.push(2)",
        );
        let stopped = fire_and_read(cx, &dispatcher, child, "click");
        assert_eq!(stopped, serde_json::json!([0, 1]));

        dispatcher
            .engine
            .eval::<()>("out.length = 0; globalThis.immediate = true;")
            .unwrap();
        let immediate = fire_and_read(cx, &dispatcher, child, "click");
        assert_eq!(immediate, serde_json::json!([0]));
    }

    #[gpui::test]
    fn every_event_shares_one_prototype_and_its_methods(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&host, node, "click");
        callbacks(
            &dispatcher,
            "0: (e) => { (globalThis.events ??= []).push(e); }",
        );
        fire_and_read(cx, &dispatcher, node, "click");
        fire_and_read(cx, &dispatcher, node, "click");

        let same = dispatcher
            .engine
            .eval::<bool>(
                "const [a, b] = events; \
                 a !== b && Object.getPrototypeOf(a) === Object.getPrototypeOf(b) \
                    && a.preventDefault === b.preventDefault \
                    && Object.getPrototypeOf(a) !== Object.prototype",
            )
            .unwrap();
        assert!(same);
    }

    #[gpui::test]
    fn an_event_method_throws_a_type_error_for_any_receiver_that_is_not_an_event(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&host, node, "click");
        callbacks(
            &dispatcher,
            "0: (e) => { \
                const proto = Object.getPrototypeOf(e); \
                const getter = (name) => Object.getOwnPropertyDescriptor(proto, name).get; \
                const setter = (name) => Object.getOwnPropertyDescriptor(proto, name).set; \
                const calls = [ \
                    e.stopPropagation, e.stopImmediatePropagation, e.preventDefault, \
                    e.composedPath, getter('defaultPrevented'), getter('returnValue'), \
                    getter('cancelBubble'), (r) => setter('returnValue').call(r, false), \
                    (r) => setter('cancelBubble').call(r, true), \
                ]; \
                for (const receiver of [undefined, null, 5, {}, Object.create(e), proto]) { \
                    for (const call of calls) { \
                        try { call.call(receiver, receiver); out.push('ran'); } \
                        catch (err) { out.push(err instanceof TypeError); } \
                    } \
                } \
                e.preventDefault(); \
                out.push(e.defaultPrevented); }",
        );

        let out = fire_and_read(cx, &dispatcher, node, "click");

        let mut want = vec![serde_json::json!(true); 6 * 9];
        want.push(serde_json::json!(true));
        assert_eq!(out, serde_json::Value::Array(want));
    }

    #[gpui::test]
    fn tracks_pointer_only_with_a_listener_or_a_focusable_node(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        assert!(!dispatcher.tracks_pointer());

        host.borrow_mut().listeners.register(node, "keydown", 0);
        assert!(dispatcher.tracks_pointer());
        host.borrow_mut().listeners.unregister(node, "keydown", 0);
        assert!(!dispatcher.tracks_pointer());

        host.borrow_mut()
            .tree
            .set_attribute(node, "tabindex", "0")
            .unwrap();
        host.borrow_mut().focus.mark_tab_dirty(node);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let host = &mut *host.borrow_mut();
            let _ = host.focus.apply_pending(&host.tree, window, cx);
        });
        assert!(dispatcher.tracks_pointer());
    }

    #[gpui::test]
    fn an_event_nobody_keeps_is_collected_with_its_state(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&host, node, "click");
        callbacks(
            &dispatcher,
            "0: (e) => { globalThis.weak = new WeakRef(e); e.stopPropagation(); }",
        );
        fire_and_read(cx, &dispatcher, node, "click");

        dispatcher.engine.with(|ctx| ctx.run_gc());

        let gone = dispatcher
            .engine
            .eval::<bool>("weak.deref() === undefined")
            .unwrap();
        assert!(gone);
    }

    #[gpui::test]
    fn an_unlistened_pointer_or_key_input_leaves_the_engine_untouched(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&host, node, "click");
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
            .unwrap();
        let cx = cx.add_empty_window();
        let key = gpui::Keystroke::parse("a").unwrap();
        let ran = |d: &EventDispatcher| d.engine.eval::<bool>("globalThis.ran").unwrap();

        cx.update(|window, cx| {
            let sink: &dyn EventSink = &dispatcher;
            sink.pointer_input("mousemove", node, &mouse_at(1.0, 1.0), window, cx);
            sink.pointer_input("wheel", node, &wheel_at(1.0, 1.0), window, cx);
            sink.pointer_input("mousedown", node, &mouse_at(1.0, 1.0), window, cx);
            sink.pointer_input("mouseup", node, &mouse_at(1.0, 1.0), window, cx);
            sink.key_input(&key, false, &EventPayload::None, window, cx);
        });
        assert!(!ran(&dispatcher), "no listener, no drain");

        for event in ["mousemove", "wheel", "keydown"] {
            // A key goes to the root when nothing is focused.
            let listener = if event == "keydown" {
                host.borrow().root
            } else {
                node
            };
            listen(&host, listener, event);
            dispatcher
                .engine
                .eval::<()>(
                    "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
                )
                .unwrap();
            cx.update(|window, cx| {
                let sink: &dyn EventSink = &dispatcher;
                match event {
                    "keydown" => sink.key_input(&key, false, &EventPayload::None, window, cx),
                    "wheel" => sink.pointer_input(event, node, &wheel_at(1.0, 1.0), window, cx),
                    _ => sink.pointer_input(event, node, &mouse_at(1.0, 1.0), window, cx),
                }
            });
            assert!(ran(&dispatcher), "a heard {event} drains");
        }
    }

    #[gpui::test]
    fn stopping_a_handled_event_changes_nothing(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(&host, child, "click");
        listen(&host, parent, "click");
        callbacks(
            &dispatcher,
            "0: (e) => { out.push(e.type); if (!globalThis.kept) globalThis.kept = e; }",
        );
        fire_and_read(cx, &dispatcher, child, "click");
        let state = dispatcher
            .engine
            .eval::<String>(
                "kept.stopPropagation(); kept.stopImmediatePropagation(); kept.cancelBubble = true; \
                 String(kept.cancelBubble)",
            )
            .unwrap();
        assert_eq!(state, "false");

        let again = fire_and_read(cx, &dispatcher, child, "click");
        assert_eq!(
            again,
            serde_json::json!(["click", "click", "click", "click"])
        );
    }

    #[gpui::test]
    fn a_script_that_patches_the_builtins_cannot_reach_or_break_event_state(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&host, node, "click");
        dispatcher
            .engine
            .eval::<()>(
                "WeakMap.prototype.get = () => { throw new Error('get'); }; \
                 WeakMap.prototype.set = () => { throw new Error('set'); }; \
                 Reflect.apply = () => { throw new Error('apply'); };",
            )
            .unwrap();
        callbacks(
            &dispatcher,
            "0: (e) => { \
                const proto = Object.getPrototypeOf(e); \
                const names = (o) => [...Object.getOwnPropertyNames(o), \
                    ...Object.getOwnPropertySymbols(o).map(String)]; \
                e.preventDefault(); \
                out.push(e.defaultPrevented, \
                    ['stop', 'immediate', 'canceled', 'dispatching', 'path'] \
                        .some((n) => names(e).includes(n) || names(proto).includes(n))); }",
        );

        let out = fire_and_read(cx, &dispatcher, node, "click");

        assert_eq!(out, serde_json::json!([true, false]));
    }
}
