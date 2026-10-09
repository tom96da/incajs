// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Binds `globalThis.__inca_dev__`, the dev-only channel a running app uses
//! to trade arbitrary-named messages with whatever is on the other end of
//! the host's own protocol — a bundler's HMR channel, say. This crate knows
//! nothing about that protocol's wire format or any particular bundler; it
//! only carries a method name and a parsed `params` value each way, exactly
//! like [`crate::dispatch`] carries a native input event into JS without
//! knowing what a caller does with it. [`install_load_signal`] adds a flag
//! the app sets to report that its load failed or recovered.

use std::cell::Cell;
use std::rc::Rc;

use rquickjs::{Ctx, Exception, Function, Object, Result as JsResult, Value as JsValue};
use serde_json::Value;

use inca_jsenv::{Engine, EngineError};

/// Called with `(method, params)` whenever JS calls
/// `__inca_dev__.send(method, paramsJson)` — `params` already parsed out of
/// `paramsJson`. What happens with it (writing a line to some transport,
/// say) is entirely up to the caller.
pub type DevSend = Rc<dyn Fn(&str, Value)>;

/// Installs `globalThis.__inca_dev__`, with the method
/// `send(method: string, paramsJson: string)`. `paramsJson` is parsed as
/// JSON before `on_send` is called; invalid JSON raises a JS `TypeError`
/// rather than panicking or silently doing nothing.
///
/// # Errors
///
/// Returns an error if defining `globalThis.__inca_dev__` or its `send`
/// method on `ctx` fails.
pub fn install_dev(ctx: &Ctx<'_>, on_send: DevSend) -> JsResult<()> {
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
    ctx.globals().set("__inca_dev__", dev)?;
    Ok(())
}

/// Adds `setLoadFailed(failed: boolean)` to `globalThis.__inca_dev__`. Each
/// call stores `failed` in `flag`, so the latest call wins and repeating a
/// value changes nothing.
///
/// An app passes `true` when its load failed and a retry may follow, and
/// `false` when a new attempt begins or the load recovered. A non-boolean
/// argument raises a JS `TypeError` and leaves `flag` as it was.
///
/// # Errors
///
/// Returns an error if `globalThis.__inca_dev__` is missing or is not an
/// object (call [`install_dev`] first), or defining `setLoadFailed` on
/// it fails.
pub fn install_load_signal(ctx: &Ctx<'_>, flag: Rc<Cell<bool>>) -> JsResult<()> {
    let dev: Object = ctx.globals().get("__inca_dev__")?;
    dev.set(
        "setLoadFailed",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'_>, failed: JsValue<'_>| -> JsResult<()> {
                let failed = failed
                    .as_bool()
                    .ok_or_else(|| Exception::throw_type(&ctx, "setLoadFailed takes a boolean"))?;
                flag.set(failed);
                Ok(())
            },
        )?,
    )
}

/// Calls `globalThis.__inca_dev__.receive?.(method, paramsJson)` inside
/// `engine`, so a message the host's own protocol doesn't recognize as one
/// of its own methods still reaches the running app. Defensive by design: a
/// missing `__inca_dev__`/`receive` is a no-op, never a fault. A throw from
/// `receive` itself is captured and returned rather than propagated — the
/// caller decides how to report it, same as any other JS fault.
#[must_use]
pub fn call_dev_receive(engine: &Engine, method: &str, params_json: &str) -> Option<EngineError> {
    engine.with(|ctx| {
        let dev = ctx.globals().get::<_, Object>("__inca_dev__").ok()?;
        let receive = dev.get::<_, Function>("receive").ok()?;
        receive
            .call::<_, ()>((method, params_json))
            .err()
            .map(|err| EngineError::capture(&ctx, &err))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
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
    fn send_raises_a_type_error_on_invalid_json() {
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

    fn flag_engine(initial: bool) -> (Engine, Rc<Cell<bool>>) {
        let engine = engine_with_dev(Rc::new(|_, _| {}));
        let flag = Rc::new(Cell::new(initial));
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
        assert!(!flag.get());
        engine
            .eval::<()>("__inca_dev__.setLoadFailed(false); __inca_dev__.setLoadFailed(true);")
            .unwrap();
        assert!(flag.get());
    }

    #[test]
    fn set_load_failed_reads_only_the_first_argument() {
        let (engine, flag) = flag_engine(false);
        engine
            .eval::<()>("__inca_dev__.setLoadFailed(true, 1);")
            .unwrap();
        assert!(flag.get());
    }

    #[test]
    fn set_load_failed_rejects_a_non_boolean_and_keeps_the_flag() {
        for arg in ["'true'", "1", "undefined", "null", ""] {
            let (engine, flag) = flag_engine(true);
            engine
                .eval::<()>(&format!(
                    "globalThis.threw = false; \
                     try {{ __inca_dev__.setLoadFailed({arg}); }} \
                     catch (e) {{ globalThis.threw = e instanceof TypeError; }}"
                ))
                .unwrap();
            assert!(engine.eval::<bool>("globalThis.threw;").unwrap(), "{arg:?}");
            assert!(flag.get(), "{arg:?}");
        }
    }

    #[test]
    fn install_load_signal_fails_before_install_dev() {
        let engine = Engine::new().unwrap();
        let result = engine.with(|ctx| install_load_signal(&ctx, Rc::new(Cell::new(false))));
        assert!(result.is_err());
    }

    #[test]
    fn call_dev_receive_invokes_the_js_side_receiver() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>(
                "globalThis.received = []; \
                 globalThis.__inca_dev__ = { \
                     receive: (m, p) => globalThis.received.push([m, p]), \
                 };",
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
    fn call_dev_receive_reports_a_throw_rather_than_propagating_it() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>(
                "globalThis.__inca_dev__ = { receive: () => { throw new Error('boom'); } };",
            )
            .unwrap();

        let err = call_dev_receive(&engine, "testEvent", "null").unwrap();
        assert_eq!(err.message(), "Error: boom");
    }
}
