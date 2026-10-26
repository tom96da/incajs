// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The timer queue behind the Web timer functions.
//!
//! Rust keeps ids, due times and periods. What each id runs stays with the
//! caller, so no JS value outlives the engine.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rquickjs::{Ctx, Result as JsResult};

use crate::engine::{Engine, EngineError, ErrorReporter};

/// The longest nesting level whose timers keep a delay under four milliseconds.
const NESTING_LIMIT: u32 = 6;
/// The delay of a timer started in a callback of level [`NESTING_LIMIT`] or deeper.
const NESTED_DELAY: Duration = Duration::from_millis(4);
/// The last id [`Timers::start`] returns. A script reads a timer id as
/// `id | 0`, which keeps every id up to this one.
const LAST_ID: u32 = 0x7fff_ffff;

/// Reads the current time. A host passes its executor's clock, so a test that
/// moves that clock moves the timers.
pub type Clock = Rc<dyn Fn() -> Instant>;

/// The clock of the operating system.
#[must_use]
pub fn system_clock() -> Clock {
    Rc::new(Instant::now)
}

/// The first point after `now` on the schedule `due`, `due + period`, and so
/// on. A zero period has no schedule and runs again at `now`.
fn next_on_grid(due: Instant, period: Duration, now: Instant) -> Instant {
    if period.is_zero() {
        return now;
    }
    let missed = now.saturating_duration_since(due).as_nanos() / period.as_nanos();
    due + period * (u32::try_from(missed).unwrap_or(u32::MAX) + 1)
}

/// A queued timer.
#[derive(Clone, Copy)]
struct Entry {
    key: (Instant, u64),
    period: Duration,
    repeat: bool,
    level: u32,
}

#[derive(Default)]
struct State {
    by_id: HashMap<u32, Entry>,
    queue: BTreeMap<(Instant, u64), u32>,
    next_seq: u64,
    /// The id the last [`Timers::start`] returned, 0 before the first.
    last_id: u32,
    running_level: u32,
}

impl State {
    fn remove(&mut self, id: u32) {
        if let Some(entry) = self.by_id.remove(&id) {
            self.queue.remove(&entry.key);
        }
    }

    fn insert(&mut self, id: u32, due: Instant, period: Duration, repeat: bool, level: u32) {
        let key = (due, self.next_seq);
        self.next_seq += 1;
        self.queue.insert(key, id);
        self.by_id.insert(
            id,
            Entry {
                key,
                period,
                repeat,
                level,
            },
        );
    }
}

/// The delay a timer of `level` waits.
fn nested(level: u32, delay: Duration) -> Duration {
    if level > NESTING_LIMIT && delay < NESTED_DELAY {
        NESTED_DELAY
    } else {
        delay
    }
}

/// The queue of the timers one script started.
pub struct Timers {
    clock: Clock,
    wake: Rc<dyn Fn()>,
    state: RefCell<State>,
}

impl Timers {
    /// An empty queue. `wake` runs inside every [`start`](Self::start) call.
    #[must_use]
    pub fn new(clock: Clock, wake: Rc<dyn Fn()>) -> Rc<Self> {
        Rc::new(Self {
            clock,
            wake,
            state: RefCell::new(State::default()),
        })
    }

    /// Queues a timer due `delay` milliseconds from now and returns its id.
    /// Ids start at 1, are shared by every timer and are never reused. `None`
    /// once the id `2^31 - 1` has been returned. A timer started while
    /// [`run_timers`] runs a timer of level six or deeper waits at least 4 ms.
    pub fn start(&self, delay: u32, repeat: bool) -> Option<u32> {
        let now = (self.clock)();
        let id = {
            let mut state = self.state.borrow_mut();
            let id = state.last_id + 1;
            if id > LAST_ID {
                return None;
            }
            state.last_id = id;
            let level = state.running_level + 1;
            let period = nested(level, Duration::from_millis(u64::from(delay)));
            state.insert(id, now + period, period, repeat, level);
            id
        };
        (self.wake)();
        Some(id)
    }

    /// Removes timer `id`. An id that is not queued does nothing.
    pub fn stop(&self, id: u32) {
        self.state.borrow_mut().remove(id);
    }

    /// The due time of the earliest queued timer.
    #[must_use]
    pub fn next_due(&self) -> Option<Instant> {
        self.state.borrow().queue.keys().next().map(|&(due, _)| due)
    }

    /// The time left until the earliest due time by this queue's own clock,
    /// zero when it has passed.
    #[must_use]
    pub fn wait(&self) -> Option<Duration> {
        self.next_due()
            .map(|due| due.saturating_duration_since((self.clock)()))
    }

    /// Takes timer `id` out of the queue for a turn that reads `now`.
    /// A repeating timer goes back on its schedule, at the first point after
    /// `now`, before its callback runs. Returns the level the callback runs at,
    /// or `None` when the timer left the queue.
    fn take(&self, id: u32, now: Instant) -> Option<u32> {
        let mut state = self.state.borrow_mut();
        let entry = *state.by_id.get(&id)?;
        state.queue.remove(&entry.key);
        if entry.repeat {
            let level = entry.level + 1;
            let period = nested(level, entry.period);
            let due = next_on_grid(entry.key.0, period, now);
            state.insert(id, due, period, true, level);
        } else {
            state.by_id.remove(&id);
        }
        Some(entry.level)
    }
}

/// Runs the timers due at one reading of the clock, in due order, and returns
/// how many ran. `fire` runs one timer, given its id. The job queue drains
/// after each timer, at its nesting level. An error `fire` returns, and each
/// failed job, goes to `reporter`. A timer started during the turn waits for
/// the next turn.
///
/// Call it outside any [`Engine::with`].
pub fn run_timers(
    engine: &Engine,
    timers: &Timers,
    reporter: &ErrorReporter,
    fire: &dyn Fn(&Ctx<'_>, u32) -> JsResult<()>,
) -> usize {
    let now = (timers.clock)();
    let due: Vec<u32> = timers
        .state
        .borrow()
        .queue
        .range(..=(now, u64::MAX))
        .map(|(_, &id)| id)
        .collect();
    let mut ran = 0;
    for id in due {
        let Some(level) = timers.take(id, now) else {
            continue;
        };
        ran += 1;
        timers.state.borrow_mut().running_level = level;
        let fired =
            engine.with(|ctx| fire(&ctx, id).map_err(|err| EngineError::capture(&ctx, &err)));
        let mut failures: Vec<EngineError> = fired.err().into_iter().collect();
        failures.extend(engine.run_jobs());
        timers.state.borrow_mut().running_level = 0;
        for failure in &failures {
            reporter(failure);
        }
    }
    ran
}

#[cfg(test)]
mod tests;
