// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Web timer functions, defined on the global object.

use std::rc::Rc;

use rquickjs::function::{Args, Opt, Rest};
use rquickjs::{
    Array, Coerced, Ctx, Error, Exception, FromJs, Function, Result as JsResult, Value,
};

use super::Timers;
use crate::callbacks;

/// Defines `setTimeout`, `setInterval`, `clearTimeout` and `clearInterval` on the global object
/// of `ctx`, over `timers`. The handler and its arguments stay in the
/// [callback table](crate::callbacks).
///
/// # Errors
///
/// Returns an error if a global cannot be defined.
pub fn install<'js>(ctx: &Ctx<'js>, timers: &Rc<Timers>) -> JsResult<()> {
    let globals = ctx.globals();
    for (name, repeat) in [("setTimeout", false), ("setInterval", true)] {
        let timers = Rc::clone(timers);
        let start = Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>,
                  handler: Value<'js>,
                  delay: Opt<Value<'js>>,
                  args: Rest<Value<'js>>| {
                start(&ctx, &timers, name, handler, delay.0, args.0, repeat)
            },
        )?
        .with_name(name)?;
        globals.set(name, start)?;
    }
    for name in ["clearTimeout", "clearInterval"] {
        let timers = Rc::clone(timers);
        let clear = Function::new(ctx.clone(), move |ctx: Ctx<'js>, id: Opt<Coerced<i32>>| {
            clear(&ctx, &timers, id.0.map_or(0, |id| id.0))
        })?
        .with_name(name)?;
        globals.set(name, clear)?;
    }
    Ok(())
}

/// Queues `handler` and returns the id of the timer.
fn start<'js>(
    ctx: &Ctx<'js>,
    timers: &Timers,
    name: &str,
    handler: Value<'js>,
    delay: Option<Value<'js>>,
    args: Vec<Value<'js>>,
    repeat: bool,
) -> JsResult<u32> {
    if !handler.is_function() {
        return Err(Exception::throw_type(
            ctx,
            &format!("{name} takes a function. pass a function as the handler"),
        ));
    }
    let delay = match delay {
        Some(delay) => Coerced::<i32>::from_js(ctx, delay)?.0,
        None => 0,
    };
    let entry = Array::new(ctx.clone())?;
    entry.set(0, handler)?;
    for (at, arg) in args.into_iter().enumerate() {
        entry.set(at + 1, arg)?;
    }
    let id = callbacks::store(ctx, entry.into_value())?
        .ok_or_else(|| Exception::throw_range(ctx, "timer ids are exhausted. restart the app"))?;
    timers.start(id, u32::try_from(delay).unwrap_or(0), repeat);
    Ok(id)
}

/// Drops timer `id`. An id the queue does not hold does nothing, so the id of a
/// listener or a frame callback stays in the table.
fn clear(ctx: &Ctx<'_>, timers: &Timers, id: i32) -> JsResult<()> {
    let Ok(id) = u32::try_from(id) else {
        return Ok(());
    };
    if timers.contains(id) {
        callbacks::release(ctx, &[id])?;
        timers.stop(id);
    }
    Ok(())
}

/// Calls the handler of timer `id` with the arguments it was started with. A one-shot timer
/// leaves the table first.
pub(super) fn fire(ctx: &Ctx<'_>, id: u32, repeat: bool) -> JsResult<()> {
    let Some(entry) = callbacks::get(ctx, id)? else {
        return Ok(());
    };
    if !repeat {
        callbacks::release(ctx, &[id])?;
    }
    let entry = entry
        .into_array()
        .ok_or_else(|| Error::new_from_js("value", "array"))?;
    let handler: Function = entry.get(0)?;
    let mut call = Args::new(ctx.clone(), entry.len() - 1);
    call.this(ctx.globals())?;
    for at in 1..entry.len() {
        call.push_arg(entry.get::<Value>(at)?)?;
    }
    call.apply::<Value>(&handler)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::time::{Duration, Instant};

    use rstest::rstest;

    use super::*;
    use crate::callbacks::{LAST_ID, NEXT};
    use crate::{Clock, Engine, EngineError, ErrorReporter, run_timers, vault};

    /// An engine with the timer globals, over a clock that stands still, and the failures the
    /// reporter received.
    struct Fixture {
        engine: Engine,
        timers: Rc<Timers>,
        start: Instant,
        reported: Rc<RefCell<Vec<String>>>,
        reporter: ErrorReporter,
    }

    fn fixture() -> Fixture {
        let start = Instant::now();
        let clock: Clock = Rc::new(move || start);
        let timers = Timers::new(clock, Rc::new(|| {}));
        let engine = Engine::new().unwrap();
        engine.with(|ctx| install(&ctx, &timers)).unwrap();
        engine
            .with(|ctx| {
                let peek = Function::new(ctx.clone(), |ctx: Ctx<'_>, id: u32| {
                    callbacks::get(&ctx, id).unwrap().is_some()
                })?;
                ctx.globals().set("peek", peek)
            })
            .unwrap();
        let reported = Rc::new(RefCell::new(Vec::new()));
        let report: ErrorReporter = {
            let reported = Rc::clone(&reported);
            Rc::new(move |err: &EngineError| reported.borrow_mut().push(err.message().to_owned()))
        };
        Fixture {
            engine,
            timers,
            start,
            reported,
            reporter: report,
        }
    }

    impl Fixture {
        fn run(&self) -> usize {
            run_timers(&self.engine, &self.timers, &self.reporter)
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

    #[rstest]
    #[case::set_timeout("setTimeout")]
    #[case::set_interval("setInterval")]
    #[case::clear_timeout("clearTimeout")]
    #[case::clear_interval("clearInterval")]
    fn the_globals_exist_before_any_script_runs(#[case] name: &str) {
        let fx = fixture();

        let kind: String = fx.engine.eval(&format!("typeof {name}")).unwrap();

        assert_eq!(kind, "function");
    }

    #[rstest]
    #[case::set_timeout("setTimeout")]
    #[case::set_interval("setInterval")]
    fn a_handler_that_is_not_a_function_throws_a_type_error(#[case] name: &str) {
        let fx = fixture();

        assert_eq!(
            fx.thrown(&format!("{name}('x', 0);")),
            format!("TypeError: {name} takes a function. pass a function as the handler")
        );
        assert_eq!(fx.timers.next_due(), None);
    }

    #[rstest]
    #[case::negative(", -5", 0)]
    #[case::numeric_string(", '10'", 10)]
    #[case::wrapped_past_32_bits(", 2 ** 32 + 10", 10)]
    #[case::fraction(", 2.9", 2)]
    #[case::missing("", 0)]
    #[case::undefined(", undefined", 0)]
    #[case::value_of(", { valueOf() { return 7; } }", 7)]
    fn the_delay_converts_to_a_32_bit_integer_raised_to_zero(#[case] delay: &str, #[case] ms: u64) {
        let fx = fixture();
        fx.engine
            .eval::<()>(&format!("setTimeout(() => {{}}{delay});"))
            .unwrap();

        assert_eq!(
            fx.timers.next_due(),
            Some(fx.start + Duration::from_millis(ms))
        );
    }

    #[test]
    fn the_handler_receives_the_extra_arguments() {
        let fx = fixture();
        fx.engine
            .eval::<()>(
                r"
                setTimeout(
                    (a, b) => {
                        globalThis.seen = [a, b];
                    },
                    0,
                    1,
                    'x',
                );
                ",
            )
            .unwrap();

        fx.run();

        let seen: String = fx.engine.eval("seen.join(',')").unwrap();
        assert_eq!(seen, "1,x");
    }

    #[test]
    fn the_handler_runs_with_the_global_object_as_this() {
        let fx = fixture();
        fx.engine
            .eval::<()>(
                r"
                setTimeout(function () {
                    'use strict';
                    globalThis.same = this === globalThis;
                }, 0);
                ",
            )
            .unwrap();

        fx.run();

        assert!(fx.engine.eval::<bool>("same").unwrap());
    }

    #[test]
    fn a_one_shot_entry_leaves_the_table_before_its_handler_runs() {
        let fx = fixture();
        fx.engine
            .eval::<()>(
                r"
                globalThis.id = setTimeout(() => {
                    globalThis.during = peek(id);
                }, 0);
                ",
            )
            .unwrap();

        fx.run();

        assert!(!fx.engine.eval::<bool>("during").unwrap());
    }

    #[test]
    fn a_throwing_one_shot_handler_leaves_no_entry() {
        let fx = fixture();
        fx.engine
            .eval::<()>(
                r"
                globalThis.id = setTimeout(() => {
                    throw new Error('boom');
                }, 0);
                ",
            )
            .unwrap();

        fx.run();

        assert!(!fx.engine.eval::<bool>("peek(id)").unwrap());
        assert_eq!(*fx.reported.borrow(), ["Error: boom"]);
    }

    #[rstest]
    #[case::clear_timeout_stops_an_interval("setInterval", "clearTimeout")]
    #[case::clear_interval_stops_a_timeout("setTimeout", "clearInterval")]
    fn either_clear_function_stops_either_kind_of_timer(#[case] start: &str, #[case] clear: &str) {
        let fx = fixture();

        fx.engine
            .eval::<()>(&format!("{clear}({start}(() => {{}}, 0));"))
            .unwrap();

        assert_eq!(fx.run(), 0);
        assert_eq!(fx.timers.next_due(), None);
        assert!(!fx.engine.eval::<bool>("peek(1)").unwrap());
    }

    #[rstest]
    #[case::unknown("clearTimeout(99);")]
    #[case::missing("clearTimeout();")]
    #[case::not_a_number("clearTimeout('abc');")]
    #[case::an_object("clearTimeout({});")]
    #[case::negative("clearTimeout(-1);")]
    fn an_id_that_names_no_timer_clears_nothing(#[case] call: &str) {
        let fx = fixture();
        fx.engine.eval::<()>("setTimeout(() => {}, 0);").unwrap();

        assert_eq!(fx.thrown(call), "none");

        assert!(fx.timers.contains(1), "{call}");
        assert!(fx.engine.eval::<bool>("peek(1)").unwrap(), "{call}");
    }

    #[rstest]
    #[case::numeric_string("'1'")]
    #[case::fraction("1.9")]
    #[case::wrapped_past_32_bits("2 ** 32 + 1")]
    fn the_id_is_read_as_a_32_bit_integer(#[case] id: &str) {
        let fx = fixture();
        fx.engine.eval::<()>("setTimeout(() => {}, 0);").unwrap();

        fx.engine
            .eval::<()>(&format!("clearTimeout({id});"))
            .unwrap();

        assert!(!fx.timers.contains(1), "{id}");
    }

    #[rstest]
    #[case::set_timeout("setTimeout")]
    #[case::set_interval("setInterval")]
    fn an_exhausted_id_space_throws_a_range_error(#[case] name: &str) {
        let fx = fixture();
        fx.engine
            .with(|ctx| vault(&ctx).unwrap().set(NEXT, LAST_ID + 1).unwrap());

        assert_eq!(
            fx.thrown(&format!("{name}(() => {{}}, 0);")),
            "RangeError: timer ids are exhausted. restart the app"
        );
        assert_eq!(fx.timers.next_due(), None);
    }

    #[test]
    fn clearing_the_id_of_another_kind_of_callback_leaves_it_in_the_table() {
        let fx = fixture();
        let id = fx
            .engine
            .with(|ctx| callbacks::store(&ctx, Value::new_int(ctx.clone(), 1)).unwrap())
            .unwrap();

        fx.engine
            .eval::<()>(&format!("clearTimeout({id});"))
            .unwrap();

        assert!(
            fx.engine
                .with(|ctx| callbacks::get(&ctx, id).unwrap().is_some())
        );
    }
}
