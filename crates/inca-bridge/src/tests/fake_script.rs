// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A fake [`Script`] that records the walks the dispatch asks for.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use inca_document::{EventPayload, EventTargetId, MousePayload};
use inca_node::NodeId;

use crate::event::{Outcome, Walk};
use crate::script::Script;
use crate::{EventDispatcher, Host};

/// One walk the dispatch asked a [`Script`] for.
#[derive(Clone, Debug)]
pub(crate) struct Walked {
    pub(crate) name: String,
    pub(crate) target: EventTargetId,
    pub(crate) payload: EventPayload,
    pub(crate) bubbles: bool,
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
/// `canceled`. It runs no listener, and every walk counts as one that entered the engine.
pub(crate) struct FakeScript {
    walks: RefCell<Vec<Walked>>,
    handler: RefCell<Handler>,
}

impl Default for FakeScript {
    fn default() -> Self {
        Self {
            walks: RefCell::default(),
            handler: RefCell::new(Rc::new(|_| false)),
        }
    }
}

impl FakeScript {
    /// Runs `handler` on each walk from now on. It returns whether the event is canceled.
    pub(crate) fn on_walk(&self, handler: impl Fn(&Walked) -> bool + 'static) {
        *self.handler.borrow_mut() = Rc::new(handler);
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
        };
        let handler = Rc::clone(&self.handler.borrow());
        self.walks.borrow_mut().push(walked.clone());
        Outcome {
            canceled: handler(&walked),
            entered: true,
        }
    }

    fn run_timers(&self) -> usize {
        0
    }

    fn run_frames(&self) -> usize {
        0
    }

    fn release(&self, _nodes: &[NodeId], _callback_ids: &[u32]) -> bool {
        false
    }

    fn drain_jobs(&self) {}

    fn report_pending(&self) {}

    fn set_reporter(&self, _reporter: inca_jsenv::ErrorReporter) {}
}

/// A dispatcher over a [`FakeScript`] and a default host.
pub(crate) fn fake_dispatcher() -> (EventDispatcher, Rc<Host>, Rc<FakeScript>) {
    let host = Rc::new(Host::default());
    let fake = Rc::new(FakeScript::default());
    let dispatcher = EventDispatcher {
        engine: Rc::new(inca_jsenv::Engine::new().unwrap()),
        script: Rc::clone(&fake) as Rc<dyn Script>,
        host: Rc::clone(&host),
        state: Rc::default(),
    };
    (dispatcher, host, fake)
}

/// What `setPointerCapture(node, 1)` does to the document while a button is held.
pub(crate) fn request_capture(host: &Host, node: NodeId) {
    crate::bindings::capture::request_capture(host, node);
}

/// What `releasePointerCapture(node, 1)` does to the document.
pub(crate) fn release_capture(host: &Host, node: NodeId) {
    crate::bindings::capture::drop_capture(host, node);
}
