// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The JS side of the development protocol. Binds `globalThis.__inca_dev__`, the channel a running
//! app in the development window uses to trade arbitrary-named messages with whatever is on the
//! other end of the host's own protocol, a bundler's HMR channel say. The host knows nothing about
//! that protocol's wire format or any particular bundler; the relay carries a method name and a
//! parsed `params` value each way, as [`inca_bridge::EventDispatcher`] carries a native input event
//! into JS. [`install_dev`] also adds `compile`, which builds app source into an async function.
//! [`install_load_signal`] adds a flag the app sets to report that its load failed or recovered.

use std::cell::Cell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Function, Object, Result as JsResult, Value as JsValue};
use serde_json::Value;

use inca_jsenv::{Engine, EngineError, compile_async_function};

/// Called with `(method, params)` whenever JS calls
/// `__inca_dev__.send(method, paramsJson)`, with `params` already parsed out of
/// `paramsJson`. What happens with it (writing a line to some transport,
/// say) is entirely up to the caller.
pub(crate) type DevSend = Rc<dyn Fn(&str, Value)>;

/// Installs `globalThis.__inca_dev__`, with the methods
/// `send(method: string, paramsJson: string)` and
/// `compile(params: string[], body: string, filename: string)`.
///
/// `paramsJson` is parsed as JSON before `on_send` is called; invalid JSON
/// raises a JS `TypeError`. `compile` returns an async function whose body is
/// `body` and whose parameters are named by `params`, built by
/// [`inca_jsenv::compile_async_function`]. Its stack frames name `filename`,
/// and a frame on line N of `body` reports line N + 1. An inline source map
/// in `body` points the frames the host reports at the original source. A
/// syntax error raises a JS `SyntaxError` whose stack names `filename` and the
/// line.
///
/// # Errors
///
/// Returns an error if defining `globalThis.__inca_dev__` or its methods on
/// `ctx` fails.
pub(crate) fn install_dev(ctx: &Ctx<'_>, on_send: DevSend) -> JsResult<()> {
    let dev = Object::new(ctx.clone())?;
    dev.set(
        "send",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'_>, method: String, params_json: String| -> JsResult<()> {
                let params: Value = serde_json::from_str(&params_json).map_err(|err| {
                    Exception::throw_type(&ctx, &format!("invalid JSON in params: {err}"))
                })?;
                on_send(&method, params);
                Ok(())
            },
        )?,
    )?;
    dev.set("compile", Function::new(ctx.clone(), compile)?)?;
    ctx.globals().set("__inca_dev__", dev)?;
    Ok(())
}

/// The `compile` method of [`install_dev`]. An argument of the wrong type, or
/// a NUL byte in any string, raises a JS `TypeError`. A syntax error raises
/// the `SyntaxError` that [`compile_async_function`] left pending.
#[allow(clippy::needless_pass_by_value)]
fn compile(
    ctx: Ctx<'_>,
    params: Vec<String>,
    body: String,
    filename: String,
) -> JsResult<Function<'_>> {
    let params: Vec<&str> = params.iter().map(String::as_str).collect();
    compile_async_function(&ctx, &params, &body, &filename)
}

/// What `setLoadFailed` reports: the latest value, and how many times the app
/// reported `true`.
#[derive(Debug, Default)]
pub(crate) struct LoadSignal {
    failed: Cell<bool>,
    failures: Cell<u32>,
}

impl LoadSignal {
    /// The value of the latest `setLoadFailed` call, `false` before any.
    #[must_use]
    pub(crate) fn failed(&self) -> bool {
        self.failed.get()
    }

    /// How many `setLoadFailed(true)` calls have happened. Compare two reads
    /// to detect a `true` that a later `false` cleared.
    #[must_use]
    pub(crate) fn failures(&self) -> u32 {
        self.failures.get()
    }
}

/// Adds `setLoadFailed(failed: boolean)` to `globalThis.__inca_dev__`. Each
/// call updates `signal`: the latest call sets [`LoadSignal::failed`], and a
/// `true` adds one to [`LoadSignal::failures`].
///
/// An app passes `true` when its load failed and a retry may follow, and
/// `false` when a new attempt begins or the load recovered. A non-boolean
/// argument raises a JS `TypeError` and leaves `signal` as it was.
///
/// # Errors
///
/// Returns an error if `globalThis.__inca_dev__` is missing or is not an
/// object (call [`install_dev`] first), or defining `setLoadFailed` on
/// it fails.
pub(crate) fn install_load_signal(ctx: &Ctx<'_>, signal: Rc<LoadSignal>) -> JsResult<()> {
    let dev: Object = ctx.globals().get("__inca_dev__")?;
    dev.set(
        "setLoadFailed",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'_>, failed: JsValue<'_>| -> JsResult<()> {
                let failed = failed
                    .as_bool()
                    .ok_or_else(|| Exception::throw_type(&ctx, "setLoadFailed takes a boolean"))?;
                signal.failed.set(failed);
                if failed {
                    signal.failures.set(signal.failures.get().wrapping_add(1));
                }
                Ok(())
            },
        )?,
    )
}

/// Calls `globalThis.__inca_dev__.receive?.(method, paramsJson)` inside
/// `engine`, so a message the host's own protocol doesn't recognize as one
/// of its own methods still reaches the running app. An undefined or null
/// `__inca_dev__` or `receive` does nothing. Any other failure, a throw from
/// `receive` or from a getter on the way to it included, is returned as an
/// [`EngineError`], and the caller decides how to report it.
#[must_use]
pub(crate) fn call_dev_receive(
    engine: &Engine,
    method: &str,
    params_json: &str,
) -> Option<EngineError> {
    engine.with(|ctx| {
        let call = || -> rquickjs::Result<()> {
            let Some(dev) = ctx.globals().get::<_, Option<Object>>("__inca_dev__")? else {
                return Ok(());
            };
            let Some(receive) = dev.get::<_, Option<Function>>("receive")? else {
                return Ok(());
            };
            receive.call((method, params_json))
        };
        call().err().map(|err| EngineError::capture(&ctx, &err))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use rstest::rstest;
    use std::cell::RefCell;

    fn engine_with_dev(on_send: DevSend) -> Engine {
        let engine = Engine::new().unwrap();
        engine.with(|ctx| install_dev(&ctx, on_send)).unwrap();
        engine
    }

    #[test]
    fn send_parses_params_and_calls_the_callback() {
        let calls: Rc<RefCell<Vec<(String, Value)>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&calls);
        let engine = engine_with_dev(Rc::new(move |method: &str, params: Value| {
            sink.borrow_mut().push((method.to_owned(), params));
        }));

        engine
            .eval::<()>(r"__inca_dev__.send('testEvent', JSON.stringify({ a: 1 }));")
            .unwrap();

        let calls = calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            ("testEvent".to_owned(), serde_json::json!({"a": 1}))
        );
    }

    #[test]
    fn send_throws_a_type_error_on_invalid_json() {
        let engine = engine_with_dev(Rc::new(|_, _| {}));

        engine
            .eval::<()>(
                r"
                    globalThis.threw = false;
                    try {
                        __inca_dev__.send('testEvent', 'not json');
                    } catch (e) {
                        globalThis.threw = e instanceof TypeError;
                    }
                ",
            )
            .unwrap();

        assert!(engine.eval::<bool>("globalThis.threw;").unwrap());
    }

    /// Calls `f(args)` and returns what its promise settled with: the value as
    /// text, or the name and stack of a rejection.
    fn settle(engine: &Engine, args: &str) -> String {
        engine
            .eval::<()>(&format!(
                r"
                f({args}).then((v) => {{ globalThis.out = String(v); }},
                (e) => {{ globalThis.out = e.name + '\n' + e.stack; }});
                "
            ))
            .unwrap();
        assert!(engine.run_jobs().is_empty());
        engine.eval("globalThis.out").unwrap()
    }

    /// Runs `__inca_dev__.compile(params, body, filename)` into the global `f`.
    fn compile_f(engine: &Engine, params: &str, body: &str, filename: &str) {
        engine
            .eval::<()>(&format!(
                "globalThis.f = __inca_dev__.compile({params}, {}, {});",
                serde_json::to_string(body).unwrap(),
                serde_json::to_string(filename).unwrap(),
            ))
            .unwrap();
    }

    /// The constructor name and message of what `source` throws, or
    /// `no throw`.
    fn thrown_by(engine: &Engine, source: &str) -> String {
        engine
            .eval::<String>(&format!(
                r"
                (() => {{
                    try {{ {source}; return 'no throw'; }}
                    catch (e) {{ return e.constructor.name + ': ' + e.message; }}
                }})()
                "
            ))
            .unwrap()
    }

    #[test]
    fn compile_binds_params_in_the_order_given() {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        compile_f(&engine, "['a', 'b']", "return a - b;", "order.js");

        assert_eq!(settle(&engine, "5, 2"), "3");
    }

    #[test]
    fn compile_frames_name_the_filename_and_add_one_to_the_body_line() {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        compile_f(
            &engine,
            "[]",
            "const a = 1;\nthrow new Error('x');",
            "app/counter.ts",
        );

        assert!(settle(&engine, "").contains("app/counter.ts:3:"));
    }

    #[rstest]
    #[case::string("'a'")]
    #[case::null("null")]
    #[case::number_item("[1]")]
    #[case::null_item("[null]")]
    fn compile_throws_type_error_for_non_string_array_params(#[case] params: &str) {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        let thrown = thrown_by(
            &engine,
            &format!("__inca_dev__.compile({params}, 'return 1;', 'f.js')"),
        );
        assert!(thrown.starts_with("TypeError"), "{params}: {thrown}");
    }

    #[rstest]
    #[case::in_params(r"['a\u0000b'], 'return 1;', 'f.js'")]
    #[case::in_body(r"[], 'return\u00001;', 'f.js'")]
    #[case::in_filename(r"[], 'return 1;', 'f\u0000.js'")]
    fn compile_throws_type_error_for_a_nul_byte_in_params_body_or_filename(#[case] args: &str) {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        let thrown = thrown_by(&engine, &format!("__inca_dev__.compile({args})"));
        assert!(thrown.starts_with("TypeError"), "{args}: {thrown}");
    }

    #[rstest]
    #[case::body("[], 1, 'f.js'")]
    #[case::filename("[], 'return 1;', 1")]
    fn compile_throws_type_error_for_a_non_string_body_or_filename(#[case] args: &str) {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        let thrown = thrown_by(&engine, &format!("__inca_dev__.compile({args})"));
        assert!(thrown.starts_with("TypeError"), "{args}: {thrown}");
    }

    #[test]
    fn compile_throws_a_syntax_error_whose_stack_names_the_file_and_line() {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        let thrown: String = engine
            .eval(
                r"
                (() => {
                    try {
                        __inca_dev__.compile([], 'const a = 1;\nconst = ;', 'broken.js');
                        return 'no throw';
                    } catch (e) { return e.name + '\n' + e.stack; }
                })()
                ",
            )
            .unwrap();

        assert!(
            thrown.starts_with("SyntaxError\n") && thrown.contains("broken.js:3"),
            "{thrown}"
        );
    }

    fn flag_engine(initial: bool) -> (Engine, Rc<LoadSignal>) {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        let flag = Rc::new(LoadSignal::default());
        flag.failed.set(initial);
        let handle = Rc::clone(&flag);
        engine
            .with(|ctx| install_load_signal(&ctx, handle))
            .unwrap();
        (engine, flag)
    }

    #[test]
    fn set_load_failed_keeps_the_latest_call() {
        let (engine, flag) = flag_engine(false);
        engine
            .eval::<()>("__inca_dev__.setLoadFailed(true); __inca_dev__.setLoadFailed(false);")
            .unwrap();
        assert!(!flag.failed());
        engine
            .eval::<()>("__inca_dev__.setLoadFailed(false); __inca_dev__.setLoadFailed(true);")
            .unwrap();
        assert!(flag.failed());
    }

    #[test]
    fn set_load_failed_counts_each_true_even_when_false_follows() {
        let (engine, flag) = flag_engine(false);
        engine
            .eval::<()>("__inca_dev__.setLoadFailed(true); __inca_dev__.setLoadFailed(false);")
            .unwrap();
        assert_eq!(flag.failures(), 1);
        engine
            .eval::<()>("__inca_dev__.setLoadFailed(false); __inca_dev__.setLoadFailed(true);")
            .unwrap();
        assert_eq!(flag.failures(), 2);
    }

    #[test]
    fn set_load_failed_reads_only_the_first_argument() {
        let (engine, flag) = flag_engine(false);
        engine
            .eval::<()>("__inca_dev__.setLoadFailed(true, 1);")
            .unwrap();
        assert!(flag.failed());
    }

    #[rstest]
    #[case::string("'true'")]
    #[case::number("1")]
    #[case::undefined("undefined")]
    #[case::null("null")]
    #[case::no_argument("")]
    fn set_load_failed_throws_for_a_non_boolean_and_keeps_the_signal(#[case] arg: &str) {
        let (engine, flag) = flag_engine(true);
        engine
            .eval::<()>(&format!(
                r"
                globalThis.threw = false;
                try {{ __inca_dev__.setLoadFailed({arg}); }}
                catch (e) {{ globalThis.threw = e instanceof TypeError; }}
                "
            ))
            .unwrap();
        assert!(engine.eval::<bool>("globalThis.threw;").unwrap(), "{arg:?}");
        assert!(flag.failed(), "{arg:?}");
        assert_eq!(flag.failures(), 0, "{arg:?}");
    }

    #[test]
    fn install_load_signal_fails_before_install_dev() {
        let engine = Engine::new().unwrap();
        let result = engine.with(|ctx| install_load_signal(&ctx, Rc::new(LoadSignal::default())));
        assert!(result.is_err());
    }

    #[test]
    fn call_dev_receive_invokes_the_js_side_receiver() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>(
                r"
                globalThis.received = [];
                globalThis.__inca_dev__ = {
                    receive: (m, p) => globalThis.received.push([m, p]),
                };
                ",
            )
            .unwrap();

        assert!(call_dev_receive(&engine, "testEvent", r#"{"a":1}"#).is_none());
        assert_eq!(
            engine
                .eval::<String>("JSON.stringify(globalThis.received);")
                .unwrap(),
            r#"[["testEvent","{\"a\":1}"]]"#
        );
    }

    #[test]
    fn call_dev_receive_is_a_no_op_when_nothing_is_listening() {
        let engine = Engine::new().unwrap();
        assert!(call_dev_receive(&engine, "testEvent", "null").is_none());
    }

    #[test]
    fn call_dev_receive_returns_a_throw_as_an_engine_error() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>(
                "globalThis.__inca_dev__ = { receive: () => { throw new Error('boom'); } };",
            )
            .unwrap();

        let err = call_dev_receive(&engine, "testEvent", "null").unwrap();
        assert_eq!(err.message(), "Error: boom");
    }

    #[rstest]
    #[case::receive_getter(
        r"
        globalThis.__inca_dev__ = {
            get receive() {
                throw new Error('boom');
            },
        };
        "
    )]
    #[case::dev_getter(
        r"
        Object.defineProperty(globalThis, '__inca_dev__', {
            get() {
                throw new Error('boom');
            },
        });
        "
    )]
    fn call_dev_receive_returns_a_throwing_getter_as_an_engine_error(#[case] setup: &str) {
        let engine = Engine::new().unwrap();
        engine.eval::<()>(setup).unwrap();

        let err = call_dev_receive(&engine, "testEvent", "null").unwrap();

        assert_eq!(err.message(), "Error: boom");
        assert_eq!(engine.eval::<String>("'ok'").unwrap(), "ok");
    }

    #[rstest]
    #[case::receive_undefined("globalThis.__inca_dev__ = { receive: undefined };")]
    #[case::receive_null("globalThis.__inca_dev__ = { receive: null };")]
    #[case::dev_null("globalThis.__inca_dev__ = null;")]
    fn call_dev_receive_does_nothing_for_a_null_or_undefined_target(#[case] setup: &str) {
        let engine = Engine::new().unwrap();
        engine.eval::<()>(setup).unwrap();

        assert!(call_dev_receive(&engine, "testEvent", "null").is_none());
    }

    #[rstest]
    #[case::receive("globalThis.__inca_dev__ = { receive: 5 };", "function")]
    #[case::dev("globalThis.__inca_dev__ = 5;", "object")]
    fn call_dev_receive_returns_a_non_function_or_non_object_as_an_engine_error(
        #[case] setup: &str,
        #[case] type_name: &str,
    ) {
        let engine = Engine::new().unwrap();
        engine.eval::<()>(setup).unwrap();

        let err = call_dev_receive(&engine, "testEvent", "null").unwrap();

        assert!(err.message().contains(type_name));
    }
}
