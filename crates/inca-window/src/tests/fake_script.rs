// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A fake [`Script`] that records the walks the dispatch asks for.

#![allow(clippy::unwrap_used)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use std::time::Duration;

use inca_document::{EventPayload, EventTargetId, MousePayload};
use inca_node::NodeId;

use async_channel::Sender;
use inca_document::{Outcome, Walk};

use crate::test_support::cells;
use crate::{Cells, EventDispatcher, Script};

/// One walk the dispatch asked a [`Script`] for.
#[derive(Clone, Debug)]
pub(crate) struct Walked {
    pub(crate) name: String,
    pub(crate) target: EventTargetId,
    pub(crate) payload: EventPayload,
    pub(crate) bubbles: bool,
    /// The nodes the walk skips.
    pub(crate) cut: Vec<NodeId>,
}

impl Walked {
    /// `target` as the text of a node id or `$window`.
    pub(crate) fn target_text(&self) -> String {
        match self.target {
            EventTargetId::Node(node) => node.to_string(),
            EventTargetId::Window => "$window".to_owned(),
            EventTargetId::Signal(id) => format!("signal{id}"),
        }
    }

    /// `type@target`.
    pub(crate) fn label(&self) -> String {
        format!("{}@{}", self.name, self.target_text())
    }

    /// The `related_target` of a mouse or focus payload.
    pub(crate) fn related(&self) -> Option<NodeId> {
        match &self.payload {
            EventPayload::Mouse(mouse) => mouse.related_target,
            EventPayload::Focus { related_target } => *related_target,
            _ => None,
        }
    }

    /// The payload as a mouse payload.
    pub(crate) fn mouse(&self) -> MousePayload {
        match &self.payload {
            EventPayload::Mouse(mouse) => *mouse,
            other => panic!("expected a mouse payload, got {other:?}"),
        }
    }
}

type Handler = Rc<dyn Fn(&Walked) -> bool>;

/// A [`Script`] that records every walk and answers each with what its handler returns as
/// `canceled`. It runs no listener, and every walk counts as one that reached the script.
pub(crate) struct FakeScript {
    walks: RefCell<Vec<Walked>>,
    handler: RefCell<Handler>,
    jobs: RefCell<Vec<Box<dyn FnOnce()>>>,
    timers: RefCell<Rc<dyn Fn() -> usize>>,
    frames: RefCell<Rc<dyn Fn() -> usize>>,
    turns: Cell<[usize; 2]>,
    timer_wait: RefCell<Rc<dyn Fn() -> Option<Duration>>>,
    frames_wait: RefCell<Rc<dyn Fn() -> bool>>,
    wake: RefCell<Option<Sender<()>>>,
}

impl Default for FakeScript {
    fn default() -> Self {
        Self {
            walks: RefCell::default(),
            handler: RefCell::new(Rc::new(|_| false)),
            jobs: RefCell::default(),
            timers: RefCell::new(Rc::new(|| 0)),
            frames: RefCell::new(Rc::new(|| 0)),
            turns: Cell::default(),
            timer_wait: RefCell::new(Rc::new(|| None)),
            frames_wait: RefCell::new(Rc::new(|| false)),
            wake: RefCell::new(None),
        }
    }
}

impl FakeScript {
    /// Runs `handler` on each walk from now on. It returns whether the event is canceled.
    pub(crate) fn on_walk(&self, handler: impl Fn(&Walked) -> bool + 'static) {
        *self.handler.borrow_mut() = Rc::new(handler);
    }

    /// Queues `job` for the next [`Script::drain_jobs`].
    pub(crate) fn queue_job(&self, job: impl FnOnce() + 'static) {
        self.jobs.borrow_mut().push(Box::new(job));
    }

    /// Runs `turn` on each [`Script::run_timers`] call from now on. It returns how many timers it
    /// ran.
    pub(crate) fn on_timers(&self, turn: impl Fn() -> usize + 'static) {
        *self.timers.borrow_mut() = Rc::new(turn);
    }

    /// Runs `turn` on each [`Script::run_frames`] call from now on. It returns how many frame
    /// callbacks it ran.
    pub(crate) fn on_frames(&self, turn: impl Fn() -> usize + 'static) {
        *self.frames.borrow_mut() = Rc::new(turn);
    }

    /// Answers [`Script::next_timer_wait`] with what `wait` returns from now on.
    pub(crate) fn on_timer_wait(&self, wait: impl Fn() -> Option<Duration> + 'static) {
        *self.timer_wait.borrow_mut() = Rc::new(wait);
    }

    /// Answers [`Script::frames_pending`] with what `pending` returns from now on.
    pub(crate) fn on_frames_pending(&self, pending: impl Fn() -> bool + 'static) {
        *self.frames_wait.borrow_mut() = Rc::new(pending);
    }

    /// Keeps `sender` to wake the timer pump with.
    pub(crate) fn set_wake(&self, sender: Sender<()>) {
        *self.wake.borrow_mut() = Some(sender);
    }

    /// Wakes the timer pump, as a script that starts a timer does.
    pub(crate) fn wake(&self) {
        if let Some(sender) = &*self.wake.borrow() {
            let _ = sender.try_send(());
        }
    }

    /// How many times the dispatch asked for the due timers and for the frame callbacks.
    pub(crate) fn turns(&self) -> (usize, usize) {
        let [timers, frames] = self.turns.get();
        (timers, frames)
    }

    /// The walks recorded since the last call.
    pub(crate) fn take(&self) -> Vec<Walked> {
        std::mem::take(&mut *self.walks.borrow_mut())
    }
}

impl Script for FakeScript {
    fn walk(&self, walk: &Walk<'_>) -> Outcome {
        let walked = Walked {
            name: walk.name.to_owned(),
            target: walk.target,
            payload: walk.payload.clone(),
            bubbles: walk.flags.bubbles,
            cut: walk.cut.to_vec(),
        };
        let handler = Rc::clone(&self.handler.borrow());
        self.walks.borrow_mut().push(walked.clone());
        Outcome {
            canceled: handler(&walked),
            entered: true,
        }
    }

    fn run_timers(&self) -> usize {
        let [timers, frames] = self.turns.get();
        self.turns.set([timers + 1, frames]);
        let turn = Rc::clone(&self.timers.borrow());
        turn()
    }

    fn run_frames(&self) -> usize {
        let [timers, frames] = self.turns.get();
        self.turns.set([timers, frames + 1]);
        let turn = Rc::clone(&self.frames.borrow());
        turn()
    }

    fn release(&self, _nodes: &[NodeId], _callback_ids: &[u32]) -> bool {
        false
    }

    fn drain_jobs(&self) {
        let jobs = std::mem::take(&mut *self.jobs.borrow_mut());
        for job in jobs {
            job();
        }
    }

    fn report_pending(&self) {}

    fn next_timer_wait(&self) -> Option<Duration> {
        let wait = Rc::clone(&self.timer_wait.borrow());
        wait()
    }

    fn frames_pending(&self) -> bool {
        let pending = Rc::clone(&self.frames_wait.borrow());
        pending()
    }
}

/// A dispatcher over a [`FakeScript`] and default cells.
pub(crate) fn fake_dispatcher() -> (EventDispatcher, Rc<Cells>, Rc<FakeScript>) {
    let (cells, wake) = cells();
    let (dispatcher, fake) = fake_dispatcher_on(&cells);
    *fake.wake.borrow_mut() = Some(wake);
    (dispatcher, cells, fake)
}

/// A dispatcher over a [`FakeScript`] and `cells`.
pub(crate) fn fake_dispatcher_on(cells: &Rc<Cells>) -> (EventDispatcher, Rc<FakeScript>) {
    let fake = Rc::new(FakeScript::default());
    let dispatcher = EventDispatcher::new(Rc::clone(&fake) as Rc<dyn Script>, Rc::clone(cells));
    (dispatcher, fake)
}

/// What `setPointerCapture(node, 1)` does to the document while a button is held.
pub(crate) fn request_capture(cells: &Cells, node: NodeId) {
    let held = cells.window.borrow().held_buttons != 0;
    let mut document = cells.document.borrow_mut();
    if held && document.tree().is_connected(node) {
        document.capture_pending = Some(node);
    }
}

/// What `releasePointerCapture(node, 1)` does to the document.
pub(crate) fn release_capture(cells: &Cells, node: NodeId) {
    let mut document = cells.document.borrow_mut();
    if document.capture_pending == Some(node) {
        document.capture_pending = None;
    }
}
