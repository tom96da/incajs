// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The clock of a context, read by `performance.now()` and by the host.

use rquickjs::object::Property;
use rquickjs::{Ctx, Function, Object, Result as JsResult, Value};

use crate::class::{define_accessor, define_method};
use crate::timers::Clock;
use crate::vault::vault;

/// The vault key of the function that reads the clock.
const NOW: &str = "now";

/// The vault key of the `performance` object.
const PERFORMANCE: &str = "performance";

/// Starts the clock of `ctx`'s realm at the reading of `clock` now, keeps its
/// reader in the vault and defines the accessor `performance` with that reader as `now`. An
/// assignment to `performance` replaces the accessor with a data property.
///
/// # Errors
///
/// Returns an error if the realm has no vault or the global cannot be defined.
pub(crate) fn install<'js>(ctx: &Ctx<'js>, clock: &Clock) -> JsResult<()> {
    let origin = clock();
    let clock = Clock::clone(clock);
    let now = Function::new(ctx.clone(), move || {
        clock().saturating_duration_since(origin).as_secs_f64() * 1000.0
    })?;
    vault(ctx)?.set(NOW, now.clone())?;
    let performance = Object::new(ctx.clone())?;
    define_method(&performance, NOW, now)?;
    vault(ctx)?.set(PERFORMANCE, performance)?;
    let get = Function::new(ctx.clone(), |ctx: Ctx<'js>| {
        vault(&ctx)?.get::<_, Object>(PERFORMANCE)
    })?;
    let set = Function::new(ctx.clone(), |ctx: Ctx<'js>, value: Value<'js>| {
        ctx.globals().prop(
            PERFORMANCE,
            Property::from(value).writable().enumerable().configurable(),
        )
    })?;
    define_accessor(&ctx.globals(), PERFORMANCE, get, set)
}

/// The milliseconds from the creation of `ctx`'s [`Engine`](crate::Engine) to
/// now, on the clock `performance.now()` reads. A script that replaces
/// `performance` does not change it.
///
/// # Errors
///
/// Returns an error if the realm has no vault, as in a context that no
/// `Engine` created.
pub fn now(ctx: &Ctx<'_>) -> JsResult<f64> {
    vault(ctx)?.get::<_, Function>(NOW)?.call(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use rstest::rstest;

    use super::*;
    use crate::Engine;
    use crate::tests::with_realm;

    /// An engine on a clock that `at` moves.
    fn engine_on_test_clock() -> (Engine, Rc<Cell<Instant>>) {
        let at = Rc::new(Cell::new(Instant::now()));
        let clock: Clock = {
            let at = Rc::clone(&at);
            Rc::new(move || at.get())
        };
        let engine = Engine::builder().clock(clock).build().unwrap();
        (engine, at)
    }

    fn advance(at: &Cell<Instant>, by: Duration) {
        at.set(at.get() + by);
    }

    /// The clock counts seconds as `f64`, so whole milliseconds differ by float rounding.
    fn assert_near(got: f64, want: f64) {
        assert!((got - want).abs() < 1e-9, "{got} is not {want}");
    }

    #[test]
    fn performance_now_reads_the_injected_clock() {
        let (engine, at) = engine_on_test_clock();
        let before: f64 = engine.eval("performance.now()").unwrap();

        advance(&at, Duration::from_millis(25));
        let after: f64 = engine.eval("performance.now()").unwrap();

        assert_near(after - before, 25.0);
    }

    #[test]
    fn performance_now_counts_from_the_creation_of_the_context() {
        let (engine, at) = engine_on_test_clock();

        advance(&at, Duration::from_millis(10));

        assert_near(engine.eval("performance.now()").unwrap(), 10.0);
    }

    #[test]
    fn now_keeps_reading_the_clock_after_a_script_replaces_performance() {
        let (engine, at) = engine_on_test_clock();
        engine
            .eval::<()>("globalThis.performance = { now: () => -1 };")
            .unwrap();

        advance(&at, Duration::from_millis(7));

        assert_near(engine.with(|ctx| now(&ctx).unwrap()), 7.0);
    }

    #[rstest]
    #[case::enumerable("d.enumerable")]
    #[case::configurable("d.configurable")]
    #[case::getter("typeof d.get === 'function'")]
    #[case::setter("typeof d.set === 'function'")]
    fn performance_descriptor_has(#[case] field: &str) {
        let (engine, _) = engine_on_test_clock();

        let got: bool = engine
            .eval(&format!(
                r"
                const d = Object.getOwnPropertyDescriptor(globalThis, 'performance');
                {field};
                "
            ))
            .unwrap();

        assert!(got);
    }

    #[test]
    fn performance_assignment_replaces_the_accessor_with_a_data_property() {
        let (engine, _) = engine_on_test_clock();

        let attributes: String = engine
            .eval(
                r"
                performance = 1;
                const d = Object.getOwnPropertyDescriptor(globalThis, 'performance');
                `${d.value} ${d.writable} ${d.enumerable} ${d.configurable}`;
                ",
            )
            .unwrap();

        assert_eq!(attributes, "1 true true true");
    }

    #[test]
    fn performance_now_is_named_after_itself() {
        let (engine, _) = engine_on_test_clock();

        assert_eq!(
            engine.eval::<String>("performance.now.name").unwrap(),
            "now"
        );
    }

    #[test]
    fn now_fails_in_a_context_no_engine_made() {
        with_realm(|ctx| assert!(now(&ctx).is_err()));
    }
}
