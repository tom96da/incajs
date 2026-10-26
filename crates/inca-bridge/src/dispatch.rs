// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Turns the native input of a window into events and runs the JS listeners
//! registered for them via `__inca_native__.addEventListener` (see
//! [`crate::bindings`]), then requests a redraw.
//!
//! An event only enters the JS engine when it reaches a listener: a capture
//! listener on its path, or a bubble listener on the nodes the event bubbles
//! to.
//! An event that entered the engine and a turn that ran a due timer each
//! drain the job queue and request a redraw. Rendering runs no JS of its own.
//!
//! ## Where the real JS function lives
//!
//! [`crate::bindings::EventListeners`] stores a [`crate::bindings::Listener`]
//! per registration: a plain `u32` callback id with its capture and passive
//! flags, never an `rquickjs::Value`/`Function`/`Persistent<T>`, because a JS
//! handle kept past the call that produced it outlives the context it
//! belongs to. `addEventListener` returns the id the host allocated, and the JS
//! caller stores the function at `globalThis.__inca_callbacks__[callbackId]`.
//! The walk of an event looks the real function up fresh inside one
//! `Engine::with` call and drops it before that call returns. It never
//! crosses into Rust-held state.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{App, Window};
use inca_gpui::event_sink::{flags_of, hover_path_difference, with_screen};
use inca_gpui::{
    EventPayload, MousePayload, NO_BUTTON, NodeId, PointerSource, buttons_mask, is_disabled_control,
};
use inca_jsenv::{Engine, EngineError, ErrorReporter, report_jobs, stderr_reporter};
use rquickjs::Function;

use crate::EventTargetId;
use crate::bindings::Host;
use crate::event::{self, Walk};
use crate::focus::FocusTransition;

mod input;
mod pointer;
mod sink;
mod timers;

/// The payload of the window's `focus` and `blur`.
const FOCUS_PAYLOAD: EventPayload = EventPayload::Focus {
    related_target: None,
};

pub(crate) use inca_gpui::event_sink::spec_of;

/// Everything needed to dispatch a native event into JS: the engine to call
/// into, and the registry of which JS callback ids are listening for which
/// `(event target, event name)`.
#[derive(Clone)]
pub struct EventDispatcher {
    engine: Rc<Engine>,
    host: Rc<RefCell<Host>>,
    reporter: ErrorReporter,
    state: Rc<State>,
}

/// The mutable state every clone of an [`EventDispatcher`] shares.
#[derive(Default)]
struct State {
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
    hovered: RefCell<BTreeSet<NodeId>>,
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
        event::time_stamp();
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

    /// Whether the containers report the pointer: a listener is registered
    /// or a node can take focus. The render path asks once per frame.
    #[must_use]
    pub fn tracks_pointer(&self) -> bool {
        let host = self.host.borrow();
        !host.listeners.is_empty() || host.focus.has_handles()
    }

    /// Returns `payload` with its `buttons` bitmask set to [`Host::held_buttons`] unless the
    /// event carries the buttons of its own press or release.
    fn with_held_buttons(&self, event: &str, mut payload: EventPayload) -> EventPayload {
        let held = self.host.borrow().held_buttons;
        if let Some(mouse) = payload.mouse_mut().filter(|_| !spec_of(event).own_buttons) {
            mouse.buttons = held;
        }
        payload
    }

    /// Updates [`Host::held_buttons`] for a raw `"mousedown"` or `"mouseup"` of `button` and
    /// returns the new value. `button` is `MousePayload`'s own field, a public part of
    /// `inca-gpui`'s API, so an out-of-range value (only 0..=4 are ever produced by this crate)
    /// is handled: [`buttons_mask`] maps it to 0.
    fn update_held_buttons(&self, event: &str, button: u8) -> u8 {
        let bit = buttons_mask(button);
        let mut host = self.host.borrow_mut();
        match event {
            "mousedown" => host.held_buttons |= bit,
            "mouseup" => host.held_buttons &= !bit,
            _ => {}
        }
        host.held_buttons
    }

    /// Returns `payload` with `movementX`/`movementY` set to the delta of the
    /// raw pointer move being dispatched, for `mousemove`. Every other event
    /// keeps 0.
    fn with_movement(&self, mut payload: EventPayload, event: &str) -> EventPayload {
        if spec_of(event).movement
            && let EventPayload::Mouse(mouse) = &mut payload
        {
            let (movement_x, movement_y) = self.state.current_movement.get().unwrap_or((0.0, 0.0));
            mouse.movement_x = movement_x;
            mouse.movement_y = movement_y;
        }
        payload
    }

    /// Takes the window's current modifier keys as the baseline for the next
    /// change. With `force` unset, an existing baseline stays.
    pub fn sync_modifiers(&self, window: &Window, force: bool) {
        if force || self.state.held_modifiers.get().is_none() {
            self.state.held_modifiers.set(Some(window.modifiers()));
        }
    }

    /// Dispatches `event` along the path of `target`, with `target` as its
    /// target, and reports what the listeners threw. Returns whether a
    /// listener prevented the default action and whether the walk entered the
    /// engine. `cut` lists the nodes whose listeners stay silent.
    fn walk(
        &self,
        target: EventTargetId,
        event: &str,
        payload: &EventPayload,
        cut: &[NodeId],
    ) -> (bool, bool) {
        let outcome = event::walk(
            &self.engine,
            &self.host,
            &Walk {
                name: event,
                flags: flags_of(event, payload),
                target,
                cut,
                payload,
                trusted: true,
            },
        );
        // Reported after the walk: a reporter is free to do anything, and
        // re-entering the engine from inside `Engine::with` panics.
        self.report_pending();
        (outcome.canceled, outcome.entered)
    }

    /// Reports listener errors, in the order they happened. Call it outside any
    /// [`Engine::with`].
    pub fn report_pending(&self) {
        report_failures(&self.host, &self.reporter);
    }

    /// The nodes that receive no pointer `mousedown`, `mouseup`, `click` or
    /// `dblclick`: a disabled button or input on the path from the mouse target
    /// or the pressed target of the event, and the ancestors of that control.
    /// Listeners below the control run. A keyboard click always runs.
    fn cut_by_disabled_control(
        &self,
        event: &str,
        payload: &EventPayload,
        cx: &App,
    ) -> Vec<NodeId> {
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
        let disabled = |id: &NodeId| host.tree.get(*id).is_some_and(is_disabled_control);
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
    /// A prevented `mousedown` leaves the focus alone. Every press records
    /// `target` as where Tab starts while no node is focused.
    fn focus_on_press(&self, target: NodeId, prevented: bool, window: &mut Window, cx: &mut App) {
        let path = self.path_from(target);
        let handle = {
            let mut host = self.host.borrow_mut();
            let focusing = if prevented {
                None
            } else {
                path.iter()
                    .copied()
                    .find(|&id| host.focus.handle(id).is_some())
            };
            let focused = host.focus.focused_node(window, cx);
            let root = host.root();
            host.focus
                .press_start(target, root, focusing.filter(|&node| Some(node) != focused));
            focusing.and_then(|node| host.focus.handle(node))
        };
        if prevented {
            return;
        }
        match handle {
            Some(handle) => handle.focus(window, cx),
            None => window.blur(cx),
        }
    }

    /// `payload` as a listener sees it: a `related_target` that left the tree
    /// reads `None`, and the buttons, movement, screen and offset fields are
    /// set. `target` is the node the event fires at.
    fn prepare(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &Window,
    ) -> EventPayload {
        let payload = self.live_related(payload.clone());
        let payload = self.with_held_buttons(event, payload);
        let payload = self.with_movement(payload, event);
        let payload = with_screen(payload, window);
        self.with_offset(target, payload)
    }

    /// `payload` with `offsetX`/`offsetY` measured from the padding edge of
    /// `target` in the last drawn frame, or equal to the client position when
    /// `target` has no recorded box.
    fn with_offset(&self, target: NodeId, mut payload: EventPayload) -> EventPayload {
        let host = self.host.borrow();
        let layout = host.layout.borrow().get(&target).copied();
        if let Some(mouse) = payload.mouse_mut() {
            let (left, top) = layout.map_or((0.0, 0.0), |l| {
                let border = f32::from(l.border);
                (
                    f32::from(l.bounds.origin.x) + border,
                    f32::from(l.bounds.origin.y) + border,
                )
            });
            mouse.offset_x = mouse.client_x - left;
            mouse.offset_y = mouse.client_y - top;
        }
        payload
    }

    /// `payload` with a `related_target` that left the tree set to `None`.
    fn live_related(&self, mut payload: EventPayload) -> EventPayload {
        let gone = |id: &NodeId| self.host.borrow().tree.get(*id).is_none();
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

    /// `target` and its ancestors up to the host root, nearest first.
    fn path_from(&self, target: NodeId) -> Vec<NodeId> {
        self.host.borrow().tree.ancestors(target).collect()
    }

    /// The nearest node that is `a`, `b` or an ancestor of both.
    fn common_ancestor(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        let above_b = self.path_from(b);
        self.path_from(a).into_iter().find(|i| above_b.contains(i))
    }

    /// Fires `event` at `start` with `start` as the `target`. The capture
    /// listeners from `window` down to `start` run first, then the bubble
    /// listeners of `start` and, while the event bubbles, of each ancestor,
    /// until one calls `stopPropagation()`. Every listener receives the same
    /// event object, whose `eventId` is larger than the one of the event
    /// fired before. Returns whether a listener prevented the default
    /// action. `preventDefault()` calls from a passive listener are ignored.
    /// A pointer `click` or `dblclick` skips a disabled button or input and its
    /// ancestors, as `mousedown` and `mouseup` do. A `start` that left the
    /// tree fires nothing. When a listener ran, the job queue drains after
    /// the walk and the window refreshes. A call made while the window draws
    /// has no refresh to make, and the caller requests the next frame.
    ///
    /// While the listeners run, `eventPhase` is 1 on the ancestors of `target`
    /// during the capture pass, 2 where `currentTarget` is `target` and 3 on
    /// the ancestors during the bubble pass, and `composedPath()` lists the path,
    /// from `target` up through its element ancestors, document and window, as the
    /// objects that `__inca_targets__` gives, or as node ids and the tags
    /// `"$document"` and `"$window"` when no such global exists. Afterwards
    /// `currentTarget` is `null`, `eventPhase` is 0 and `composedPath()` is
    /// empty. A listener that throws is reported, and the remaining listeners
    /// run. A throw of `__inca_targets__` is reported and no listener runs.
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
        self.fire_inner(start, event, payload, window, cx)
    }

    fn fire_inner(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.host.borrow_mut().focus.observe(window, cx);
        let payload = &self.prepare(event, start, payload, window);
        let cut = self.cut_by_disabled_control(event, payload, cx);
        let (canceled, entered) = self.walk(EventTargetId::Node(start), event, payload, &cut);
        if entered {
            self.drain_observed(window);
        }
        canceled
    }

    fn key_target(&self, window: &mut Window, cx: &mut App) -> NodeId {
        let (root, focused) = {
            let host = self.host.borrow();
            (host.root(), host.focus.focused_node(window, cx))
        };
        self.resolve(focused, root)
    }

    /// Runs the work a window owes before it draws a frame: syncs the
    /// modifier keys, applies a requested focus and fires the resulting
    /// `focus`/`blur` events, frees the nodes nothing reaches, reveals the
    /// focused node and fires pending `scroll` events. Call it first in the
    /// window's render.
    pub fn begin_frame(&self, window: &mut Window, cx: &mut App) {
        self.sync_modifiers(window, false);
        let transitions = {
            let host = &mut *self.host.borrow_mut();
            host.focus.apply_pending(&host.tree, window, cx)
        };
        for transition in &transitions {
            self.input_focus_moved(transition, window, cx);
            transition.dispatch(self, window, cx);
        }
        self.release_orphans(window, cx);
        self.host.borrow_mut().reveal_focus(window);
        self.fire_scrolls(window, cx);
    }

    /// Frees the orphans of the tree and tells the script through the global
    /// `__inca_released__(nodeIds, callbackIds)`. The global is looked up per
    /// call and skipped when it is not a function. A thrown error is
    /// reported and the frame goes on. Nodes the hook leaves orphaned wait
    /// for the next frame, which this requests.
    fn release_orphans(&self, window: &mut Window, cx: &mut App) {
        let swept = self.host.borrow_mut().sweep_orphans();
        let Some((nodes, callbacks)) = swept else {
            return;
        };
        self.host.borrow_mut().focus.observe(window, cx);
        let host = &self.host;
        let entered = self.engine.with(|ctx| {
            let Ok(hook) = ctx.globals().get::<_, Function>("__inca_released__") else {
                return false;
            };
            let called = event::array_of(&ctx, nodes).and_then(|nodes| {
                let callbacks = event::array_of(&ctx, callbacks)?;
                hook.call::<_, ()>((nodes, callbacks))
            });
            if let Err(err) = called {
                host.borrow_mut()
                    .failures
                    .push(EngineError::capture(&ctx, &err));
            }
            true
        });
        if entered {
            self.drain_observed(window);
        }
        // The requests end when a frame leaves no orphan.
        if self.host.borrow().tree.has_orphans() {
            window.request_animation_frame();
        }
    }

    /// Handles the window gaining (`active` true) or losing focus. The
    /// modifier keys are re-read, a lost window clears the held presses, and
    /// the focused node receives `focus` or `blur` while keeping its focus,
    /// and an edited focused `input` fires `change` before its `blur`.
    /// `window` receives `focus` before the node and `blur` after it.
    pub fn window_activated(&self, active: bool, window: &mut Window, cx: &mut App) {
        self.sync_modifiers(window, true);
        if !active {
            inca_gpui::clear_presses(cx);
            self.reset_pointer(window, cx);
        }
        self.host.borrow_mut().focus.set_window_blurred(!active);
        let focused = self.host.borrow().focus.focused_node(window, cx);
        if active {
            self.fire_at_global(EventTargetId::Window, "focus", &FOCUS_PAYLOAD, window, cx);
        }
        if let Some(node) = focused {
            if !active {
                self.input_window_blur(node, window, cx);
            }
            FocusTransition::window_activation(node, active).dispatch(self, window, cx);
        }
        if !active {
            self.fire_at_global(EventTargetId::Window, "blur", &FOCUS_PAYLOAD, window, cx);
        }
    }

    /// Ends the pointer capture with `lostpointercapture`, then forgets the held buttons and a
    /// canceled `pointerdown`. The release of a button the window never sees would leave them set.
    fn reset_pointer(&self, window: &mut Window, cx: &mut App) {
        self.host.borrow_mut().capture_pending = None;
        let payload =
            EventPayload::Mouse(MousePayload::at(gpui::Point::default(), window.modifiers()));
        self.process_capture(&payload, NO_BUTTON, window, cx);
        self.state.pointerdown_canceled.set(false);
        self.state.input_drag.set(None);
        let mut host = self.host.borrow_mut();
        host.held_buttons = 0;
        host.capture_pending = None;
        host.capture_active = None;
    }

    /// Fires `event` at `document` or `window` alone, with no node on its path.
    fn fire_at_global(
        &self,
        target: EventTargetId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.host.borrow_mut().focus.observe(window, cx);
        let payload = self.with_held_buttons(event, payload.clone());
        let payload = with_screen(payload, window);
        let (_, entered) = self.walk(target, event, &payload, &[]);
        if entered {
            self.drain_observed(window);
        }
    }

    /// Fires `scroll` at each container whose offset changed since the last
    /// call and that a `scroll` listener reaches. A script write and a wheel
    /// scroll each fire once. Only the containers a listener reaches run JS.
    pub fn fire_scrolls(&self, window: &mut Window, cx: &mut App) {
        let moved = self.host.borrow_mut().scrolled_nodes();
        let any = !moved.is_empty();
        for node in moved {
            self.fire(node, "scroll", &EventPayload::None, window, cx);
        }
        // A refresh requested while drawing has no effect, so a listener's own
        // writes need the next frame.
        if any {
            window.request_animation_frame();
        }
    }

    /// The deepest container `gpui` reports as hovered. Containers that left
    /// the tree drop out of the set.
    fn deepest_hovered(&self) -> Option<NodeId> {
        let mut set = self.state.hovered.borrow_mut();
        set.retain(|id| self.host.borrow().tree.get(*id).is_some());
        set.iter()
            .copied()
            .max_by_key(|id| (self.path_from(*id).len(), *id))
    }

    /// Queues the hover report for the end of the update, once. It reports the
    /// hovered set unless the update already reported its deepest container.
    fn schedule_hover_report(&self, window: &mut Window, cx: &mut App) {
        if self.state.hover_scheduled.replace(true) {
            return;
        }
        let this = self.clone();
        window.defer(cx, move |window, cx| {
            this.state.hover_scheduled.set(false);
            if !this.state.hover_settled.replace(false) {
                this.report_hover(this.deepest_hovered(), window, cx);
            }
        });
    }

    /// [`Self::report_hover_with`] for a move, which is about no button.
    fn report_hover(&self, next: Option<NodeId>, window: &mut Window, cx: &mut App) {
        self.report_hover_with(next, NO_BUTTON, window, cx);
    }

    /// Fires the hover events for the pointer moving from the deepest
    /// container reported last time to the one under it now, or to the capturing
    /// node while a capture is active. The pointer events come first, then the
    /// mouse ones. Each group is `out` at the old container, `leave` at each
    /// container left (innermost first), `over` at the new one and `enter` at
    /// each container entered (outermost first). The pointer events are about
    /// `button`.
    fn report_hover_with(
        &self,
        physical: Option<NodeId>,
        button: u8,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mouse = MousePayload::at(window.mouse_position(), window.modifiers());
        self.process_capture(&EventPayload::Mouse(mouse), button, window, cx);
        let next = self.hover_target(physical);
        let next_path = next.map(|id| self.path_from(id)).unwrap_or_default();
        // A removed container is replaced by its nearest live ancestor.
        let old_path = self.state.hover_prev.replace(next_path.clone());
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
        let with = |related_target| {
            EventPayload::Mouse(MousePayload {
                related_target,
                ..mouse
            })
        };
        let pointer = |related| pointer::pointer_payload(&with(related), button);
        // A removed container receives no `out`.
        let old = prev.filter(|_| !removed);
        if let Some(old) = old {
            self.fire(old, "pointerout", &pointer(next), window, cx);
        }
        for &id in &leave {
            self.fire(id, "pointerleave", &pointer(next), window, cx);
        }
        if let Some(new) = next {
            self.fire(new, "pointerover", &pointer(prev), window, cx);
        }
        for &id in &enter {
            self.fire(id, "pointerenter", &pointer(prev), window, cx);
        }
        if let Some(old) = old {
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
mod tests;
