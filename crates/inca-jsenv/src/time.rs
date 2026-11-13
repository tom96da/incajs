// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The clock of a context, read by `performance.now()` and by the host, and the `Performance`
//! class.

use std::time::{SystemTime, UNIX_EPOCH};

// `PerformanceInstance` derives `JsLifetime`, and the class holds a number and no JS value.
#[allow(clippy::disallowed_types)]
use rquickjs::JsLifetime;
use rquickjs::object::Property;
use rquickjs::{Class, Ctx, Function, Object, Result as JsResult, Value};

use crate::class::{
    define_accessor, define_class, define_global, define_this_getter, define_this_method,
    illegal_constructor, plain_class, receiver,
};
use crate::timeline;
use crate::timers::Clock;
use crate::vault::vault;

/// The vault key of the function that reads the clock.
const NOW: &str = "now";

/// The vault key of the `performance` object.
const PERFORMANCE: &str = "performance";

/// The `Performance` instance. `time_origin` is the epoch milliseconds of the moment
/// `performance.now()` is 0.
// `PerformanceInstance` holds a number and no JS value, so the derive is sound.
#[allow(clippy::disallowed_macros)]
#[derive(JsLifetime)]
pub(crate) struct PerformanceInstance {
    time_origin: f64,
}

plain_class!(PerformanceInstance, "Performance");

/// Starts the clock of `ctx`'s realm at the reading of `clock` now and keeps its reader in the
/// vault. Defines the global class `Performance` and the accessor `performance`, whose value is
/// the one instance. An assignment to `performance` replaces the accessor with a data property.
///
/// # Errors
///
/// Returns an error if the realm has no vault or a global cannot be defined.
pub(crate) fn install(ctx: &Ctx<'_>, clock: &Clock) -> JsResult<()> {
    let time_origin = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0;
    install_at(ctx, clock, time_origin)
}

/// [`install`] with the epoch milliseconds of the origin given.
fn install_at<'js>(ctx: &Ctx<'js>, clock: &Clock, time_origin: f64) -> JsResult<()> {
    let origin = clock();
    let clock = Clock::clone(clock);
    let now = Function::new(ctx.clone(), move || {
        clock().saturating_duration_since(origin).as_secs_f64() * 1000.0
    })?;
    vault(ctx)?.set(NOW, now)?;
    let proto = Object::new(ctx.clone())?;
    let class = define_class(ctx, &proto, "Performance", 0, illegal_constructor())?;
    define_this_method(ctx, &proto, NOW, 0, |ctx, this, _| {
        receiver::<PerformanceInstance>(ctx, &this)?;
        self::now(ctx)
    })?;
    define_this_getter(ctx, &proto, "timeOrigin", |ctx, this| {
        Ok(receiver::<PerformanceInstance>(ctx, &this)?
            .borrow()
            .time_origin)
    })?;
    define_this_method(ctx, &proto, "toJSON", 0, |ctx, this, _| {
        let json = Object::new(ctx.clone())?;
        json.set(
            "timeOrigin",
            receiver::<PerformanceInstance>(ctx, &this)?
                .borrow()
                .time_origin,
        )?;
        Ok(json)
    })?;
    timeline::install(ctx, &proto)?;
    define_global(ctx, "Performance", class)?;
    let performance = Class::instance_proto(PerformanceInstance { time_origin }, proto)?;
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

    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use rstest::rstest;

    use super::*;
    use crate::tests::{advance, engine_on_test_clock, thrown, with_realm};

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

    #[test]
    fn time_origin_is_epoch_milliseconds() {
        let (engine, _) = engine_on_test_clock();

        let got: bool = engine
            .eval(
                r"
                const origin = performance.timeOrigin;
                origin > 1e12 && Math.abs(Date.now() - (origin + performance.now())) < 1000;
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[test]
    fn time_origin_is_the_given_epoch_milliseconds() {
        with_realm(|ctx| {
            crate::vault::create_vault(&ctx).unwrap();
            let clock: Clock = Rc::new(Instant::now);
            install_at(&ctx, &clock, 1_700_000_000_123.5).unwrap();

            assert!(
                ctx.eval::<bool, _>("performance.timeOrigin === 1700000000123.5")
                    .unwrap()
            );
        });
    }

    #[test]
    fn time_origin_is_a_getter_of_the_prototype() {
        let (engine, _) = engine_on_test_clock();

        let got: bool = engine
            .eval(
                r"
                const d = Object.getOwnPropertyDescriptor(Performance.prototype, 'timeOrigin');
                typeof d.get === 'function' && d.set === undefined;
                ",
            )
            .unwrap();

        assert!(got);
    }

    #[test]
    fn to_json_holds_the_time_origin() {
        let (engine, _) = engine_on_test_clock();

        let got: bool = engine
            .eval("performance.toJSON().timeOrigin === performance.timeOrigin")
            .unwrap();

        assert!(got);
    }

    #[test]
    fn json_stringify_gives_the_time_origin() {
        let (engine, _) = engine_on_test_clock();

        let got: bool = engine
            .eval(
                r#"
                JSON.stringify(performance) === `{"timeOrigin":${performance.timeOrigin}}`;
                "#,
            )
            .unwrap();

        assert!(got);
    }

    #[test]
    fn performance_has_no_own_property() {
        let (engine, _) = engine_on_test_clock();

        assert_eq!(
            engine
                .eval::<String>("JSON.stringify(Object.getOwnPropertyNames(performance))")
                .unwrap(),
            "[]"
        );
    }

    #[rstest]
    #[case::now("Performance.prototype.now.call({})")]
    #[case::time_origin(
        "Object.getOwnPropertyDescriptor(Performance.prototype, 'timeOrigin').get.call({})"
    )]
    #[case::to_json("Performance.prototype.toJSON.call({})")]
    fn performance_member_on_a_foreign_receiver_throws(#[case] call: &str) {
        assert_eq!(
            thrown(call),
            "TypeError: the receiver is not a Performance. call the method on an instance"
        );
    }
}
