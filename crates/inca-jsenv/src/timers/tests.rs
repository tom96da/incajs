// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

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
