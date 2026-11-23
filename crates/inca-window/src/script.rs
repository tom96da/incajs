// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The seam between the dispatch of a window and the script that listens to it.

use std::time::Duration;

use inca_document::{Outcome, Released, Walk};
use inca_node::NodeId;

/// What the dispatch needs of the script side. A method that runs script keeps what the script
/// threw, and [`report_pending`](Self::report_pending) sends it on. Call every method with no
/// borrow of a cell held and outside any call into the script.
pub trait Script {
    /// Runs the listeners of `walk` and reports whether one canceled the event and whether the
    /// walk entered the script.
    fn walk(&self, walk: &Walk<'_>) -> Outcome;

    /// Runs the timers that are due now and returns how many ran.
    fn run_timers(&self) -> usize;

    /// Runs the frame callbacks pending now and returns how many ran.
    fn run_frames(&self) -> usize;

    /// Tells the script that `nodes` were freed with the listeners `released` names, hands it the
    /// signals whose hold ends, and returns whether it entered the script.
    fn release(&self, nodes: &[NodeId], released: &Released) -> bool;

    /// Runs the queued jobs, and keeps what they threw.
    fn drain_jobs(&self);

    /// Sends what the script threw so far to its reporter, in order.
    fn report_pending(&self);

    /// The time left until the earliest timer is due, or `None` with no timer.
    fn next_timer_wait(&self) -> Option<Duration>;

    /// Whether a frame callback waits for the next frame.
    fn frames_pending(&self) -> bool;
}
