// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Turns the native input of a window into events and runs the JS listeners
//! registered for them via `__inca_native__.addEventListener` (see
//! [`crate::bindings`]), then requests a redraw.
//!
//! An event only enters the JS engine when it reaches a listener: a capture
//! listener on its path, or a bubble listener on the nodes the event bubbles
//! to.
//! Only an event that entered the engine drains the job queue and requests a
//! redraw. Rendering runs JS only when input arrives.
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
//! `Engine::with` call and drops it before that call returns — it never
//! crosses into Rust-held state.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{App, Window};
use inca_gpui::event_sink::{hover_path_difference, with_screen};
use inca_gpui::{
    EventPayload, MousePayload, NodeId, PointerSource, buttons_mask, is_disabled_button,
};
use inca_jsenv::{Engine, ErrorReporter, report_jobs, stderr_reporter};

use crate::bindings::Host;
use crate::event::{self, Walk};
use crate::focus::FocusTransition;

mod sink;

pub(crate) use inca_gpui::event_sink::spec_of;

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
        event::time_stamp();
        Self {
            engine,
            host,
            reporter: stderr_reporter(),
            held_buttons: Rc::new(Cell::new(0)),
            last_position: Rc::new(Cell::new(None)),
            current_movement: Rc::new(Cell::new(None)),
            held_modifiers: Rc::new(Cell::new(None)),
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
    /// [`Self::held_buttons`]. A `mouseup` excludes the button just
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
    /// produced by this crate) is handled: [`buttons_mask`] maps it to 0.
    fn update_held_buttons(&self, event: &str, button: u8) -> u8 {
        let bit = buttons_mask(button);
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
        let pending = std::mem::take(&mut self.host.borrow_mut().failures);
        for failure in &pending {
            (self.reporter)(failure);
        }
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
        let disabled = |id: &NodeId| host.tree.get(*id).is_some_and(is_disabled_button);
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
    /// reads `None`, and the buttons, movement, screen and offset fields are
    /// set. `target` is the node the event fires at.
    fn prepare(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &Window,
    ) -> EventPayload {
        let payload = self.live_related(payload);
        let payload = self.with_held_buttons(event, &payload);
        let payload = self.with_movement(&payload, event);
        let payload = with_screen(&payload, window);
        self.with_offset(target, &payload)
    }

    /// `payload` with `offsetX`/`offsetY` measured from the padding edge of
    /// `target` in the last drawn frame, or equal to the client position when
    /// `target` has no recorded box.
    fn with_offset(&self, target: NodeId, payload: &EventPayload) -> EventPayload {
        let host = self.host.borrow();
        let layout = host.layout.borrow().get(&target).copied();
        let mut payload = payload.clone();
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
        event::path_from(&self.host.borrow(), target)
    }

    /// The nearest node that is `a`, `b` or an ancestor of both.
    fn common_ancestor(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        let above_b = self.path_from(b);
        self.path_from(a).into_iter().find(|i| above_b.contains(i))
    }

    /// Fires `event` at `start` with `start` as the `target`. The capture
    /// listeners of the root down to `start` run first, then the bubble
    /// listeners of `start` and, while the event bubbles, of each ancestor,
    /// until one calls `stopPropagation()`. Every listener receives the same
    /// event object, whose `eventId` is larger than the one of the event
    /// fired before. Returns whether a listener prevented the default
    /// action. `preventDefault()` calls from a passive listener are ignored.
    /// A pointer `click` or `dblclick` skips a disabled button and its
    /// ancestors, as `mousedown` and `mouseup` do. A `start` that left the
    /// tree fires nothing. When a listener ran, the job queue drains after
    /// the walk and the window refreshes. A call made while the window draws
    /// has no refresh to make, and the caller requests the next frame.
    ///
    /// While the listeners run, `eventPhase` is 1 on the ancestors of `target`
    /// during the capture pass, 2 where `currentTarget` is `target` and 3 on
    /// the ancestors during the bubble pass, and `composedPath()` lists the
    /// node ids from `target` up to the root. Afterwards `currentTarget` is
    /// `null`, `eventPhase` is 0 and `composedPath()` is empty. A listener
    /// that throws is reported, and the remaining listeners run.
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
        self.host.borrow_mut().focus.observe(window, cx);
        let payload = &self.prepare(event, start, payload, window);
        let cut = self.cut_by_disabled_button(event, payload, cx);
        let (canceled, entered) = self.walk(start, event, payload, &cut);
        if entered {
            self.drain_observed(window);
        }
        canceled
    }

    /// Runs the work a window owes before it draws a frame: syncs the
    /// modifier keys, applies a requested focus and fires the resulting
    /// `focus`/`blur` events, reveals the focused node and fires pending
    /// `scroll` events. Call it first in the window's render.
    pub fn begin_frame(&self, window: &mut Window, cx: &mut App) {
        self.sync_modifiers(window, false);
        let transitions = {
            let host = &mut *self.host.borrow_mut();
            host.focus.apply_pending(&host.tree, window, cx)
        };
        for transition in &transitions {
            transition.dispatch(self, window, cx);
        }
        self.host.borrow_mut().reveal_focus(window);
        self.fire_scrolls(window, cx);
    }

    /// Handles the window gaining (`active` true) or losing focus. The
    /// modifier keys are re-read, a lost window clears the held presses, and
    /// the focused node receives `focus` or `blur` while keeping its focus.
    pub fn window_activated(&self, active: bool, window: &mut Window, cx: &mut App) {
        self.sync_modifiers(window, true);
        if !active {
            inca_gpui::clear_presses(cx);
        }
        self.host.borrow_mut().focus.set_window_blurred(!active);
        let focused = self.host.borrow().focus.focused_node(window, cx);
        if let Some(node) = focused {
            FocusTransition::window_activation(node, active).dispatch(self, window, cx);
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

    /// Records the node the window focuses, so `focusedNode()` in JS that runs
    /// next sees focus the window applied itself. Call it before running JS
    /// from outside an event.
    pub fn observe_focus(&self, window: &Window, cx: &App) {
        self.host.borrow_mut().focus.observe(window, cx);
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
#[allow(clippy::unwrap_used)]
mod tests;
