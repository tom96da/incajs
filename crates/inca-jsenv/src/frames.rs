// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frame callbacks behind `requestAnimationFrame` and `cancelAnimationFrame`.
//!
//! Rust keeps handles and table ids. The callback of each handle lives in the
//! [callback table](crate::callbacks), so no JS value outlives the engine.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use rquickjs::function::Opt;
use rquickjs::{Coerced, Ctx, Exception, Function, Result as JsResult, Value};

use crate::callbacks::{self, LAST_ID};
use crate::engine::{Engine, EngineError, ErrorReporter};
use crate::time::now;

/// A requested callback: the handle the script holds and the id in the callback table.
type Entry = (u32, u32);

#[derive(Default)]
struct State {
    pending: Vec<Entry>,
    running: VecDeque<Entry>,
    last_handle: u32,
}

/// The callbacks a script asked to run before a frame.
pub struct Frames {
    wake: Rc<dyn Fn()>,
    state: RefCell<State>,
}

impl Frames {
    /// An empty list. `wake` runs inside every [`request`](Self::request) call.
    #[must_use]
    pub fn new(wake: Rc<dyn Fn()>) -> Rc<Self> {
        Rc::new(Self {
            wake,
            state: RefCell::new(State::default()),
        })
    }

    /// Queues the callback stored under table id `id` and returns its handle. Handles start at 1
    /// and step by 1 per request. `None` after the handle [`LAST_ID`].
    pub fn request(&self, id: u32) -> Option<u32> {
        let handle = {
            let mut state = self.state.borrow_mut();
            let handle = state.last_handle.checked_add(1).filter(|&h| h <= LAST_ID)?;
            state.last_handle = handle;
            state.pending.push((handle, id));
            handle
        };
        (self.wake)();
        Some(handle)
    }

    /// Removes the callback of `handle` from the queue and from the frame being run, and returns
    /// its table id. A handle that is not queued does nothing.
    pub fn cancel(&self, handle: u32) -> Option<u32> {
        let mut state = self.state.borrow_mut();
        let id = state
            .pending
            .iter()
            .chain(&state.running)
            .find(|entry| entry.0 == handle)
            .map(|entry| entry.1)?;
        state.pending.retain(|entry| entry.0 != handle);
        state.running.retain(|entry| entry.0 != handle);
        Some(id)
    }

    /// Whether a callback waits for the next frame.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        !self.state.borrow().pending.is_empty()
    }

    /// Moves the queue into the batch of the frame.
    fn begin_batch(&self) {
        let mut state = self.state.borrow_mut();
        let batch = std::mem::take(&mut state.pending);
        state.running.extend(batch);
    }

    /// The next callback of the batch, or `None` when it is done.
    fn next_in_batch(&self) -> Option<Entry> {
        self.state.borrow_mut().running.pop_front()
    }
}

/// Defines `requestAnimationFrame` and `cancelAnimationFrame` on the global object of `ctx`, over
/// `frames`. The callback stays in the [callback table](crate::callbacks).
///
/// # Errors
///
/// Returns an error if a global cannot be defined.
pub fn install<'js>(ctx: &Ctx<'js>, frames: &Rc<Frames>) -> JsResult<()> {
    let globals = ctx.globals();
    let request = {
        let frames = Rc::clone(frames);
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, callback: Opt<Value<'js>>| request(&ctx, &frames, callback.0),
        )?
        .with_name("requestAnimationFrame")?
    };
    globals.set("requestAnimationFrame", request)?;
    let cancel = {
        let frames = Rc::clone(frames);
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, handle: Opt<Coerced<i32>>| cancel(&ctx, &frames, handle.0),
        )?
        .with_name("cancelAnimationFrame")?
    };
    globals.set("cancelAnimationFrame", cancel)
}

/// Queues `callback` and returns the handle of the request.
fn request<'js>(ctx: &Ctx<'js>, frames: &Frames, callback: Option<Value<'js>>) -> JsResult<u32> {
    let Some(callback) = callback.filter(Value::is_function) else {
        return Err(Exception::throw_type(
            ctx,
            "requestAnimationFrame takes a function. pass a function as the callback",
        ));
    };
    let exhausted = || Exception::throw_range(ctx, "timer ids are exhausted. restart the app");
    let id = callbacks::store(ctx, callback)?.ok_or_else(exhausted)?;
    frames.request(id).ok_or_else(|| {
        let _ = callbacks::release(ctx, &[id]);
        exhausted()
    })
}

/// Drops the request of `handle`. The handle is read as an unsigned 32-bit integer.
fn cancel(ctx: &Ctx<'_>, frames: &Frames, handle: Option<Coerced<i32>>) -> JsResult<()> {
    let Some(Coerced(handle)) = handle else {
        return Err(Exception::throw_type(
            ctx,
            "cancelAnimationFrame takes 1 argument. pass the handle requestAnimationFrame returned",
        ));
    };
    if let Some(id) = frames.cancel(u32::from_ne_bytes(handle.to_ne_bytes())) {
        callbacks::release(ctx, &[id])?;
    }
    Ok(())
}

/// Calls the callback stored under `id` with `timestamp`. The entry leaves the table first.
fn fire(ctx: &Ctx<'_>, id: u32, timestamp: f64) -> JsResult<()> {
    let Some(callback) = callbacks::get(ctx, id)? else {
        return Ok(());
    };
    callbacks::release(ctx, &[id])?;
    callback
        .into_function()
        .ok_or_else(|| rquickjs::Error::new_from_js("value", "function"))?
        .call::<_, Value>((timestamp,))?;
    Ok(())
}

/// Runs the callbacks requested before this call, in registration order, and returns how many
/// ran. Each receives the one timestamp of the frame, in milliseconds on the clock of
/// `performance.now()`. The job queue drains after each callback. A throw of a callback, and each
/// failed job, goes to `reporter`. A callback requested during the run waits for the next run, and
/// a callback canceled by an earlier one of the batch does not run.
///
/// Call it outside any [`Engine::with`].
///
/// # Panics
///
/// Panics when the engine has no vault, which no [`Engine`] lacks.
pub fn run_frames(engine: &Engine, frames: &Frames, reporter: &ErrorReporter) -> usize {
    if !frames.is_pending() {
        return 0;
    }
    frames.begin_batch();
    let timestamp = engine.with(|ctx| now(&ctx).expect("an engine has a vault"));
    let mut ran = 0;
    while let Some((_, id)) = frames.next_in_batch() {
        ran += 1;
        let fired = engine
            .with(|ctx| fire(&ctx, id, timestamp).map_err(|err| EngineError::capture(&ctx, &err)));
        let mut failures: Vec<EngineError> = fired.err().into_iter().collect();
        failures.extend(engine.run_jobs());
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
    use std::time::{Duration, Instant};

    use rstest::rstest;

    use super::*;
    use crate::timers::{self, Clock, Timers};

    /// An engine with the frame globals on a clock the test moves, and the failures the reporter
    /// received.
    struct Fixture {
        engine: Engine,
        frames: Rc<Frames>,
        at: Rc<Cell<Instant>>,
        reported: Rc<RefCell<Vec<String>>>,
        reporter: ErrorReporter,
    }

    fn fixture() -> Fixture {
        let at = Rc::new(Cell::new(Instant::now()));
        let clock: Clock = {
            let at = Rc::clone(&at);
            Rc::new(move || at.get())
        };
        let frames = Frames::new(Rc::new(|| {}));
        let timers = Timers::new(Rc::clone(&clock), Rc::new(|| {}));
        let engine = Engine::builder().clock(clock).build().unwrap();
        engine
            .with(|ctx| {
                install(&ctx, &frames)?;
                timers::install(&ctx, &timers)?;
                let advance = {
                    let at = Rc::clone(&at);
                    Function::new(ctx.clone(), move |ms: u64| {
                        at.set(at.get() + Duration::from_millis(ms));
                    })?
                };
                ctx.globals().set("advance", advance)
            })
            .unwrap();
        let reported = Rc::new(RefCell::new(Vec::new()));
        let report: ErrorReporter = {
            let reported = Rc::clone(&reported);
            Rc::new(move |err: &EngineError| reported.borrow_mut().push(err.message().to_owned()))
        };
        let fx = Fixture {
            engine,
            frames,
            at,
            reported,
            reporter: report,
        };
        fx.eval("globalThis.log = [];");
        fx
    }

    impl Fixture {
        fn eval(&self, script: &str) {
            self.engine.eval::<()>(script).unwrap();
        }

        fn run(&self) -> usize {
            run_frames(&self.engine, &self.frames, &self.reporter)
        }

        fn log(&self) -> String {
            self.engine.eval::<String>("log.join(',')").unwrap()
        }

        /// The name and message of what `call` throws, or `none`.
        fn thrown(&self, call: &str) -> String {
            self.engine
                .eval(&format!(
                    r"
                    try {{
                        {call}
                        'none';
                    }} catch (error) {{
                        `${{error.name}}: ${{error.message}}`;
                    }}
                    "
                ))
                .unwrap()
        }
    }

    #[test]
    fn callbacks_run_once_each_in_registration_order() {
        let fx = fixture();
        fx.eval(
            r"
            requestAnimationFrame(() => log.push('a'));
            requestAnimationFrame(() => log.push('b'));
            requestAnimationFrame(() => log.push('c'));
            ",
        );

        assert_eq!(fx.run(), 3);
        assert_eq!(fx.run(), 0);

        assert_eq!(fx.log(), "a,b,c");
    }

    #[test]
    fn handles_step_by_one_even_with_a_timer_between() {
        let fx = fixture();
        fx.eval(
            r"
            log.push(requestAnimationFrame(() => {}));
            setTimeout(() => {}, 0);
            log.push(requestAnimationFrame(() => {}));
            ",
        );
        fx.run();

        fx.eval("log.push(requestAnimationFrame(() => {}));");

        assert_eq!(fx.log(), "1,2,3");
    }

    #[test]
    fn a_callback_requested_during_the_run_waits_for_the_next_run() {
        let fx = fixture();
        fx.eval(
            r"
            requestAnimationFrame(() => {
                log.push('a');
                requestAnimationFrame(() => log.push('b'));
            });
            ",
        );

        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "a");
        assert!(fx.frames.is_pending());
        assert_eq!(fx.run(), 1);
        assert_eq!(fx.log(), "a,b");
    }

    #[test]
    fn a_throwing_callback_is_reported() {
        let fx = fixture();
        fx.eval(
            r"
            requestAnimationFrame(() => {
                throw new Error('boom');
            });
            requestAnimationFrame(() => log.push('b'));
            ",
        );

        fx.run();

        assert_eq!(*fx.reported.borrow(), ["Error: boom"]);
    }

    #[test]
    fn the_callback_after_a_throwing_one_runs() {
        let fx = fixture();
        fx.eval(
            r"
            requestAnimationFrame(() => {
                throw new Error('boom');
            });
            requestAnimationFrame(() => log.push('b'));
            ",
        );

        fx.run();

        assert_eq!(fx.log(), "b");
    }

    #[test]
    fn a_callback_canceled_by_an_earlier_one_of_the_frame_does_not_run() {
        let fx = fixture();
        fx.eval(
            r"
            globalThis.later = 0;
            requestAnimationFrame(() => {
                log.push('first');
                cancelAnimationFrame(later);
            });
            globalThis.later = requestAnimationFrame(() => log.push('later'));
            ",
        );

        assert_eq!(fx.run(), 1);

        assert_eq!(fx.log(), "first");
    }

    #[test]
    fn a_canceled_callback_leaves_the_table() {
        let fx = fixture();
        fx.eval("cancelAnimationFrame(requestAnimationFrame(() => {}));");

        assert!(!fx.frames.is_pending());
        assert!(
            fx.engine
                .with(|ctx| callbacks::get(&ctx, 1).unwrap().is_none())
        );
    }

    #[rstest]
    #[case::negative("-1", 1)]
    #[case::not_a_number("NaN", 1)]
    #[case::wrapped_past_32_bits("2 ** 32 + 1", 0)]
    fn cancel_animation_frame_reads_the_handle_as_an_unsigned_32_bit_integer(
        #[case] handle: &str,
        #[case] callbacks_left: usize,
    ) {
        let fx = fixture();
        fx.eval("requestAnimationFrame(() => {});");

        fx.eval(&format!(
            "globalThis.returned = cancelAnimationFrame({handle});"
        ));

        assert_eq!(
            fx.engine.eval::<String>("typeof returned").unwrap(),
            "undefined"
        );
        assert_eq!(fx.run(), callbacks_left);
    }

    #[test]
    fn cancel_animation_frame_runs_value_of_once() {
        let fx = fixture();

        fx.eval(
            r"
            cancelAnimationFrame({
                valueOf() {
                    log.push('valueOf');
                    return 1;
                },
            });
            ",
        );

        assert_eq!(fx.log(), "valueOf");
    }

    #[test]
    fn cancel_animation_frame_without_an_argument_throws_a_type_error() {
        let fx = fixture();

        assert_eq!(
            fx.thrown("cancelAnimationFrame();"),
            "TypeError: cancelAnimationFrame takes 1 argument. pass the handle \
             requestAnimationFrame returned"
        );
    }

    #[test]
    fn cancel_animation_frame_leaves_the_table_id_of_another_kind_of_callback() {
        let fx = fixture();
        fx.eval(
            r"
            requestAnimationFrame(() => {});
            requestAnimationFrame(() => {});
            globalThis.timer = setTimeout(() => {}, 0);
            ",
        );
        let timer = fx.engine.eval::<u32>("timer").unwrap();

        fx.eval(&format!("cancelAnimationFrame({timer});"));

        assert!(
            fx.engine
                .with(|ctx| callbacks::get(&ctx, timer).unwrap().is_some())
        );
    }

    #[test]
    fn every_callback_of_a_frame_receives_the_same_timestamp() {
        let fx = fixture();
        fx.eval(
            r"
            requestAnimationFrame((t) => {
                advance(5);
                log.push(t);
                log.push(performance.now());
            });
            requestAnimationFrame((t) => log.push(t));
            ",
        );
        fx.at.set(fx.at.get() + Duration::from_millis(10));

        fx.run();

        let seen: Vec<f64> = fx
            .engine
            .eval::<String>("log.join(',')")
            .unwrap()
            .split(',')
            .map(|n| n.parse().unwrap())
            .collect();
        assert_eq!(seen.len(), 3);
        assert!((seen[0] - 10.0).abs() < 1e-9, "{seen:?}");
        assert!((seen[1] - 15.0).abs() < 1e-9, "{seen:?}");
        assert!((seen[2] - 10.0).abs() < 1e-9, "{seen:?}");
    }

    #[rstest]
    #[case::missing("requestAnimationFrame();")]
    #[case::number("requestAnimationFrame(1);")]
    #[case::string("requestAnimationFrame('x');")]
    #[case::object("requestAnimationFrame({});")]
    fn a_callback_that_is_not_a_function_throws_a_type_error(#[case] call: &str) {
        let fx = fixture();

        assert_eq!(
            fx.thrown(call),
            "TypeError: requestAnimationFrame takes a function. pass a function as the callback"
        );
        assert!(!fx.frames.is_pending());
    }

    #[test]
    fn an_exhausted_id_space_throws_a_range_error() {
        let fx = fixture();
        fx.engine.with(|ctx| callbacks::exhaust_ids(&ctx).unwrap());

        assert_eq!(
            fx.thrown("requestAnimationFrame(() => {});"),
            "RangeError: timer ids are exhausted. restart the app"
        );
        assert!(!fx.frames.is_pending());
    }

    #[test]
    fn an_exhausted_handle_space_throws_a_range_error_and_stores_nothing() {
        let fx = fixture();
        fx.frames.state.borrow_mut().last_handle = LAST_ID;

        assert_eq!(
            fx.thrown("requestAnimationFrame(() => {});"),
            "RangeError: timer ids are exhausted. restart the app"
        );
        assert!(
            fx.engine
                .with(|ctx| callbacks::get(&ctx, 1).unwrap().is_none())
        );
    }
}
