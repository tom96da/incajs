// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Turns the native input of a window into events and runs the listeners registered for them
//! through a [`Script`], then requests a redraw.
//!
//! An event only enters the script when it reaches a listener: a capture listener on its path, or
//! a bubble listener on the nodes the event bubbles to.
//! An event that entered the script, a turn that ran a due timer and a frame that ran a frame
//! callback each drain the job queue and request a redraw. Rendering runs no script of its own.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{App, Window};
use inca_node::NodeId;

use crate::script::Script;
use crate::view::Cells;

/// Everything needed to dispatch a native event to a script: the script to call, and the cells
/// of the document and the window the dispatch reads and writes.
#[derive(Clone)]
pub struct EventDispatcher {
    pub(crate) script: Rc<dyn Script>,
    pub cells: Rc<Cells>,
    pub(crate) state: Rc<State>,
}

/// The mutable state every clone of an [`EventDispatcher`] shares.
#[derive(Default)]
pub(crate) struct State {
    /// Whether the `pointerdown` of the press now held was canceled. It suppresses `mousedown`,
    /// `mousemove` and `mouseup` until the `pointerup`.
    pub(crate) pointerdown_canceled: Cell<bool>,
    /// The container whose scrollbar thumb is being dragged. While it is set, the pointer moves
    /// and the hover state stay where they were.
    pub(crate) bar_drag: Cell<Option<NodeId>>,
    /// The target of the last `pointerdown`, which the `contextmenu` of that press goes to.
    pub(crate) press_target: Cell<Option<NodeId>>,
    /// The target and button of the release that ended a capture, until its clicks and boundary
    /// events fire.
    pub(crate) captured_release: Cell<Option<(NodeId, u8)>>,
    // Position of the previous raw pointer move, shared by every window using
    // this dispatcher. `None` before the first move and after a window exit.
    pub(crate) last_position: Cell<Option<(f32, f32)>>,
    // Delta of the raw pointer move being dispatched now, shared by every
    // node and event name that move produces.
    pub(crate) current_movement: Cell<Option<(f32, f32)>>,
    /// The window's modifier keys as of the last change. `None` until
    /// [`EventDispatcher::sync_modifiers`] first reads them.
    pub(crate) held_modifiers: Cell<Option<gpui::Modifiers>>,
    /// The containers `gpui` reports as hovered.
    pub(crate) hovered: RefCell<BTreeSet<NodeId>>,
    /// The deepest hovered container the hover events last reported and its
    /// ancestors, nearest first.
    pub(crate) hover_prev: RefCell<Vec<NodeId>>,
    /// Whether a hover report is waiting for the end of the update.
    pub(crate) hover_scheduled: Cell<bool>,
    /// Whether the update being handled already reported its deepest
    /// container.
    pub(crate) hover_settled: Cell<bool>,
    /// The Space press waiting for its release: the window and the focused
    /// button. An Enter click, a Space key down, a Space key up or the button
    /// losing focus ends it.
    pub(crate) space_down: Cell<Option<(gpui::WindowId, NodeId)>>,
    /// The focused enabled button under a held mouse press: the window and
    /// the button. A Space key up clicks it. A click, the mouse release or the
    /// button losing focus ends it, and the next mouse press replaces it.
    pub(crate) mouse_held: Cell<Option<(gpui::WindowId, NodeId)>>,
    /// The `input` a held left press selects in, until its release.
    pub(crate) input_drag: Cell<Option<NodeId>>,
    /// The task that sleeps until the next timer is due, or `None` before
    /// [`EventDispatcher::start_timers`] and after [`EventDispatcher::stop_timers`].
    pub(crate) timer_pump: RefCell<Option<gpui::Task<()>>>,
    /// Whether the window holds a next-frame callback that runs the frame callbacks.
    pub(crate) frame_requested: Cell<bool>,
}

impl EventDispatcher {
    /// A dispatcher that runs listeners through `script` and works on `cells`.
    #[must_use]
    pub fn new(script: Rc<dyn Script>, cells: Rc<Cells>) -> Self {
        Self {
            script,
            cells,
            state: Rc::default(),
        }
    }

    /// Whether the containers report the pointer: a listener is registered, a node can take
    /// focus or a style rule names `:hover` or `:active`. The render path asks once per frame.
    #[must_use]
    pub fn tracks_pointer(&self) -> bool {
        let document = self.cells.document.borrow();
        !document.listeners.is_empty()
            || self.cells.window.borrow().focus.has_handles()
            || document.style.uses_pointer_state(&document.tree)
    }

    /// Takes the window's current modifier keys as the baseline for the next
    /// change. With `force` unset, an existing baseline stays.
    pub fn sync_modifiers(&self, window: &Window, force: bool) {
        if force || self.state.held_modifiers.get().is_none() {
            self.state.held_modifiers.set(Some(window.modifiers()));
        }
    }

    /// Reports script errors, in the order they happened.
    pub fn report_pending(&self) {
        self.script.report_pending();
    }

    /// Runs `turn`, a turn of the JS side, after recording the node the window focuses, so the
    /// turn starts from the focus the window applied itself. Every turn the window starts runs
    /// through it: an event, the due timers, the frame callbacks, the released-node hook and the
    /// job queue.
    pub fn turn<R>(
        &self,
        window: &mut Window,
        cx: &mut App,
        turn: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        self.cells.observe_focus(window, cx);
        turn(window, cx)
    }

    /// Runs the queued jobs in a [`Self::turn`] and requests a redraw, then
    /// [`Self::report_pending`].
    pub fn drain_jobs_and_refresh(&self, window: &mut Window, cx: &mut App) {
        self.turn(window, cx, |window, _| self.drain_observed(window));
    }

    pub(crate) fn drain_observed(&self, window: &mut Window) {
        drain_jobs_and_refresh(&*self.script, window);
        self.report_pending();
    }
}

/// Drains the job queue of `script`, then requests a redraw. Code that only schedules work
/// hasn't mutated the tree once the scheduling call returns, so the queue has to run before the
/// frame is drawn.
pub(crate) fn drain_jobs_and_refresh(script: &dyn Script, window: &mut Window) {
    script.drain_jobs();
    window.refresh();
}
