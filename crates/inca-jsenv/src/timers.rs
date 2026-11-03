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
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::{Cell, RefCell};

    use rquickjs::Function;

    use super::*;

    /// The script side of the fixture, over the natives `start` and `stop`:
    /// `after(handler, delay)` and `every(handler, delay)` queue a handler and
    /// return its id, `cancel(id)` drops it, and `fire(id)` runs it.
    const SCRIPT: &str = r"
        globalThis.log = [];
        const handlers = new Map();
        const queue = (repeat) => (handler, delay) => {
            const id = start(delay, repeat);
            handlers.set(id, handler);
            return id;
        };
        globalThis.after = queue(false);
        globalThis.every = queue(true);
        globalThis.cancel = (id) => {
            handlers.delete(id);
            stop(id);
        };
        globalThis.fire = (id) => {
            handlers.get(id)();
        };
        ";

    /// Runs timer `id` through the script's `fire`.
    fn fire(ctx: &Ctx<'_>, id: u32) -> JsResult<()> {
        ctx.globals().get::<_, Function>("fire")?.call((id,))
    }

    /// An engine with the fixture script, a clock the test moves, and the reports.
    struct Fixture {
        engine: Engine,
        timers: Rc<Timers>,
        now: Rc<Cell<Instant>>,
        wakes: Rc<Cell<u32>>,
        reported: Rc<RefCell<Vec<String>>>,
        reporter: ErrorReporter,
    }

    fn fixture() -> Fixture {
        let now = Rc::new(Cell::new(Instant::now()));
        let clock: Clock = {
            let now = Rc::clone(&now);
            Rc::new(move || now.get())
        };
        let wakes = Rc::new(Cell::new(0));
        let timers = Timers::new(clock, {
            let wakes = Rc::clone(&wakes);
            Rc::new(move || wakes.set(wakes.get() + 1))
        });
        let engine = Engine::new().unwrap();
        engine.with(|ctx| {
            let starter = Rc::clone(&timers);
            let start = Function::new(ctx.clone(), move |delay: u32, repeat: bool| {
                starter.start(delay, repeat).unwrap()
            })
            .unwrap();
            let stopper = Rc::clone(&timers);
            let stop = Function::new(ctx.clone(), move |id: u32| stopper.stop(id)).unwrap();
            ctx.globals().set("start", start).unwrap();
            ctx.globals().set("stop", stop).unwrap();
        });
        let reported = Rc::new(RefCell::new(Vec::new()));
        let report: ErrorReporter = {
            let reported = Rc::clone(&reported);
            Rc::new(move |err: &EngineError| reported.borrow_mut().push(err.message().to_owned()))
        };
        let fx = Fixture {
            engine,
            timers,
            now,
            wakes,
            reported,
            reporter: report,
        };
        fx.eval(SCRIPT);
        fx
    }

    impl Fixture {
        fn eval(&self, script: &str) {
            self.engine.eval::<()>(script).unwrap();
        }

        fn eval_string(&self, script: &str) -> String {
            self.engine.eval::<String>(script).unwrap()
        }

        fn advance(&self, ms: u64) {
            self.now.set(self.now.get() + Duration::from_millis(ms));
        }

        fn run(&self) -> usize {
            run_timers(&self.engine, &self.timers, &self.reporter, &fire)
        }

        fn log(&self) -> String {
            self.eval_string("log.join(',')")
        }

        fn due_in(&self, ms: u64) -> Instant {
            self.now.get() + Duration::from_millis(ms)
        }
    }

    #[test]
    fn a_timeout_runs_once_at_its_due_time() {
        let fx = fixture();
        fx.eval("after(() => log.push('a'), 100);");

        fx.advance(99);
        assert_eq!(fx.run(), 0);
        assert_eq!(fx.log(), "");
        fx.advance(1);
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "a");
        fx.advance(1000);
        assert_eq!(fx.run(), 0);
        assert_eq!(fx.log(), "a");
    }

    #[test]
    fn an_interval_runs_once_per_period() {
        let fx = fixture();
        fx.eval("every(() => log.push('a'), 10);");
        for _ in 0..3 {
            fx.advance(10);
            assert_eq!(fx.run(), 1);
        }
        assert_eq!(fx.log(), "a,a,a");
    }

    #[test]
    fn a_late_interval_returns_to_its_schedule() {
        let fx = fixture();
        fx.eval("every(() => log.push('a'), 50);");

        fx.advance(175);
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.timers.next_due(), Some(fx.due_in(25)));
        assert_eq!(fx.run(), 0);
    }

    #[test]
    fn clear_interval_in_its_own_callback_stops_the_interval() {
        let fx = fixture();
        fx.eval(
            r"
            const id = every(() => {
                log.push('a');
                cancel(id);
            }, 10);
            ",
        );

        fx.advance(10);
        assert_eq!(fx.run(), 1);
        fx.advance(10);
        assert_eq!(fx.run(), 0);
        assert_eq!(fx.log(), "a");
        assert_eq!(fx.timers.next_due(), None);
    }

    #[test]
    fn a_throwing_callback_reaches_the_reporter() {
        let fx = fixture();
        fx.eval(
            r"
            after(() => {
                throw new Error('boom');
            }, 0);
            ",
        );
        assert_eq!(fx.run(), 1);
        assert_eq!(*fx.reported.borrow(), ["Error: boom"]);
    }

    #[test]
    fn a_timer_runs_after_an_earlier_callback_threw() {
        let fx = fixture();
        fx.eval(
            r"
            after(() => {
                throw new Error('boom');
            }, 0);
            after(() => log.push('b'), 0);
            ",
        );
        assert_eq!(fx.run(), 2);
        assert_eq!(fx.log(), "b");
    }

    #[test]
    fn a_throwing_interval_keeps_its_schedule() {
        let fx = fixture();
        fx.eval(
            r"
            every(() => {
                throw new Error('x');
            }, 10);
            ",
        );
        fx.advance(10);
        assert_eq!(fx.run(), 1);
        fx.advance(10);
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.reported.borrow().len(), 2);
    }

    #[test]
    fn timers_with_equal_delays_run_in_creation_order() {
        let fx = fixture();
        fx.eval(
            r"
            for (const n of ['a', 'b', 'c']) {
                after(() => log.push(n), 10);
            }
            ",
        );
        fx.advance(10);
        assert_eq!(fx.run(), 3);
        assert_eq!(fx.log(), "a,b,c");
    }

    #[test]
    fn a_shorter_delay_created_later_runs_first() {
        let fx = fixture();
        fx.eval(
            r"
            after(() => log.push('a'), 10);
            after(() => log.push('b'), 5);
            ",
        );
        fx.advance(10);
        assert_eq!(fx.run(), 2);
        assert_eq!(fx.log(), "b,a");
    }

    #[test]
    fn a_timer_started_in_a_callback_waits_for_the_next_turn() {
        let fx = fixture();
        fx.eval(
            r"
            after(() => {
                log.push('a');
                after(() => log.push('b'), 0);
            }, 0);
            ",
        );
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "a");
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "a,b");
    }

    #[test]
    fn a_callback_cleared_by_an_earlier_one_in_the_turn_does_not_run() {
        let fx = fixture();
        fx.eval(
            r"
            let b;
            after(() => {
                cancel(b);
                log.push('a');
            }, 5);
            b = after(() => log.push('b'), 5);
            ",
        );
        fx.advance(5);
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "a");
    }

    #[test]
    fn a_promise_job_of_a_callback_runs_before_the_next_callback() {
        let fx = fixture();
        fx.eval(
            r"
            after(() => {
                log.push('a');
                Promise.resolve().then(() => log.push('job'));
            }, 0);
            after(() => log.push('b'), 0);
            ",
        );
        assert_eq!(fx.run(), 2);
        assert_eq!(fx.log(), "a,job,b");
    }

    /// Runs the six turns whose timers wait no time, checks the seventh timer waits four
    /// milliseconds, and returns once it ran.
    fn assert_seventh_link_waits_four_ms(fx: &Fixture) {
        for _ in 0..6 {
            assert_eq!(fx.run(), 1);
        }
        assert_eq!(fx.log(), "1,2,3,4,5,6");
        assert_eq!(fx.run(), 0);
        fx.advance(3);
        assert_eq!(fx.run(), 0);
        fx.advance(1);
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "1,2,3,4,5,6,7");
    }

    #[test]
    fn the_seventh_nested_zero_delay_timeout_waits_four_milliseconds() {
        let fx = fixture();
        fx.eval(
            r"
            let n = 0;
            const link = () => {
                n += 1;
                log.push(n);
                if (n < 8) {
                    after(link, 0);
                }
            };
            after(link, 0);
            ",
        );
        assert_seventh_link_waits_four_ms(&fx);
    }

    #[test]
    fn a_timer_started_in_a_job_of_a_callback_keeps_the_nesting_level() {
        let fx = fixture();
        fx.eval(
            r"
            let n = 0;
            const sleep = () => new Promise((resolve) => after(resolve, 0));
            (async () => {
                for (let i = 0; i < 8; i++) {
                    await sleep();
                    n += 1;
                    log.push(n);
                }
            })();
            ",
        );
        assert_seventh_link_waits_four_ms(&fx);
    }

    #[test]
    fn a_timer_started_outside_a_callback_has_the_first_level() {
        let fx = fixture();
        fx.eval(
            r"
            let n = 0;
            const link = () => {
                n += 1;
                log.push(n);
                if (n < 8) {
                    after(link, 0);
                }
            };
            after(link, 0);
            ",
        );
        for _ in 0..6 {
            assert_eq!(fx.run(), 1);
        }
        fx.eval("after(() => log.push('top'), 0);");
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "1,2,3,4,5,6,top");
    }

    #[test]
    fn a_zero_period_interval_waits_four_milliseconds_after_six_runs() {
        let fx = fixture();
        fx.eval("every(() => log.push('x'), 0);");
        for _ in 0..6 {
            assert_eq!(fx.run(), 1);
        }
        assert_eq!(fx.run(), 0);
        fx.advance(3);
        assert_eq!(fx.run(), 0);
        fx.advance(1);
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "x,x,x,x,x,x,x");
    }

    #[test]
    fn next_due_is_the_earliest_queued_due_time() {
        let fx = fixture();
        assert_eq!(fx.timers.next_due(), None);
        let late = fx.timers.start(30, false).unwrap();
        let early = fx.timers.start(10, false).unwrap();
        assert_eq!(fx.timers.next_due(), Some(fx.due_in(10)));
        fx.timers.stop(early);
        assert_eq!(fx.timers.next_due(), Some(fx.due_in(30)));
        fx.timers.stop(late);
        assert_eq!(fx.timers.next_due(), None);
    }

    #[test]
    fn wait_is_the_time_left_by_the_queues_own_clock() {
        let fx = fixture();
        assert_eq!(fx.timers.wait(), None);
        fx.timers.start(10, false);
        assert_eq!(fx.timers.wait(), Some(Duration::from_millis(10)));
        fx.advance(4);
        assert_eq!(fx.timers.wait(), Some(Duration::from_millis(6)));
        fx.advance(20);
        assert_eq!(fx.timers.wait(), Some(Duration::ZERO));
    }

    #[test]
    fn start_numbers_the_timers_from_one_without_reuse() {
        let fx = fixture();
        let first = fx.timers.start(10, false);
        let second = fx.timers.start(10, true);
        fx.timers.stop(1);
        let third = fx.timers.start(10, false);

        assert_eq!([first, second, third], [Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn start_after_the_last_id_returns_none() {
        let fx = fixture();
        fx.timers.state.borrow_mut().last_id = LAST_ID - 1;

        assert_eq!(fx.timers.start(10, false), Some(LAST_ID));
        assert_eq!(fx.timers.start(10, false), None);
        assert_eq!(fx.wakes.get(), 1);
    }

    #[test]
    fn wake_runs_on_every_start() {
        let fx = fixture();
        fx.eval(
            r"
            after(() => {}, 1);
            every(() => {}, 1);
            ",
        );
        assert_eq!(fx.wakes.get(), 2);
    }

    #[test]
    fn wake_does_not_run_on_stop() {
        let fx = fixture();
        fx.eval("cancel(after(() => {}, 1));");
        assert_eq!(fx.wakes.get(), 1);
    }
}
