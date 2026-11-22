// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The script of a window, run by the bridge.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use inca_bridge::{Host, JsScript, JsState};
use inca_document::{DocumentState, Outcome, View, Walk};
use inca_jsenv::{Clock, Frames, Timers};
use inca_node::NodeId;
use inca_window::{Cells, Script, WindowState};

/// The host of the bindings and the cells of the dispatch over one document, one window state and
/// one JS state whose timers read `clock`.
pub fn compose(clock: Clock) -> (Rc<Host>, Rc<Cells>) {
    let (sender, wake) = async_channel::bounded(1);
    let send: Rc<dyn Fn()> = Rc::new(move || {
        let _ = sender.try_send(());
    });
    let timers = Timers::new(clock, Rc::clone(&send));
    let frames = Frames::new(send);
    let document = Rc::new(RefCell::new(DocumentState::default()));
    let window = Rc::new(RefCell::new(WindowState::new(wake)));
    let view: Rc<RefCell<dyn View>> = Rc::clone(&window) as Rc<RefCell<dyn View>>;
    let host = Rc::new(Host::new(
        Rc::clone(&document),
        view,
        JsState::new(timers, frames),
    ));
    (host, Rc::new(Cells { document, window }))
}

/// The turns of the JS engine, as the dispatch of a window asks for them.
pub struct BridgeScript(pub JsScript);

impl Script for BridgeScript {
    fn walk(&self, walk: &Walk<'_>) -> Outcome {
        self.0.walk(walk)
    }

    fn run_timers(&self) -> usize {
        self.0.run_timers()
    }

    fn run_frames(&self) -> usize {
        self.0.run_frames()
    }

    fn release(&self, nodes: &[NodeId], callback_ids: &[u32]) -> bool {
        self.0.release(nodes, callback_ids)
    }

    fn drain_jobs(&self) {
        self.0.drain_jobs();
    }

    fn report_pending(&self) {
        self.0.report_pending();
    }

    fn next_timer_wait(&self) -> Option<Duration> {
        self.0.next_timer_wait()
    }

    fn frames_pending(&self) -> bool {
        self.0.frames_pending()
    }
}
