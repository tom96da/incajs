// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `QuickJS` runtime bootstrap.
//!
//! An [`Engine`] pairs one `QuickJS` runtime with one execution context. Each
//! `Engine` is fully independent: creating a global on one has no effect on
//! any other `Engine`, since they don't share a runtime or a heap.

use std::cell::RefCell;
use std::fmt;
use std::ops::ControlFlow;
use std::path::PathBuf;

use rquickjs::{Coerced, Context, Ctx, FromJs, Function, Module, Persistent, Runtime, Value};

use crate::loader::{DiskLoader, DiskResolver};

pub type EngineResult<T> = Result<T, EngineError>;

/// A failure out of the JS engine, with the thrown value already read from
/// the context that produced it.
///
/// Reading a pending exception clears it, so it has to happen inside the
/// call that failed. That is why this is what [`Engine`]'s methods return:
/// a caller cannot get the order wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    message: String,
    stack: Option<String>,
}

impl EngineError {
    /// Reads `err`, and any exception pending on `ctx`, into an owned error.
    ///
    /// Call it inside the same [`Ctx`] borrow that produced `err`.
    #[must_use]
    pub fn capture(ctx: &Ctx<'_>, err: &rquickjs::Error) -> Self {
        if matches!(err, rquickjs::Error::Exception) {
            return Self::from_thrown(&ctx.catch());
        }
        Self::plain(err)
    }

    /// A failure with no JS value behind it: engine setup, or a conversion
    /// that never entered JS.
    fn plain(err: &rquickjs::Error) -> Self {
        Self {
            message: err.to_string(),
            stack: None,
        }
    }

    /// JS can throw any value. An `Error` gives up a name, a message and its
    /// frames; anything else is coerced to text, so a thrown object reads as
    /// `[object Object]` — nothing here walks its shape.
    fn from_thrown(thrown: &Value<'_>) -> Self {
        let Some(object) = thrown.as_object().filter(|_| thrown.is_error()) else {
            return Self {
                message: thrown
                    .get::<Coerced<String>>()
                    .map_or_else(|_| "uncaught, unreadable value".to_owned(), |text| text.0),
                stack: None,
            };
        };

        let name = object
            .get::<_, Option<String>>("name")
            .ok()
            .flatten()
            .unwrap_or_else(|| "Error".to_owned());
        let message = match object.get::<_, Option<String>>("message").ok().flatten() {
            Some(message) if !message.is_empty() => format!("{name}: {message}"),
            _ => name,
        };
        let stack = object
            .get::<_, Option<String>>("stack")
            .ok()
            .flatten()
            .filter(|stack| !stack.trim().is_empty())
            .map(|stack| stack.trim_end().to_owned());

        Self { message, stack }
    }

    /// The thrown value as text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The frames behind it, or `None` — `QuickJS` records them only for an
    /// `Error`, and only the frames: the message is not part of them.
    #[must_use]
    pub fn stack(&self) -> Option<&str> {
        self.stack.as_deref()
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        match &self.stack {
            Some(stack) => write!(f, "\n{stack}"),
            None => Ok(()),
        }
    }
}

impl std::error::Error for EngineError {}

const TRACK: &str = "__inca_track_rejection__";
const TAKE: &str = "__inca_take_rejections__";

/// Keeps each promise rejected with no handler yet, mapped to its reason.
/// The rejection tracker fills it through `TRACK`; [`Engine::run_jobs`]
/// empties it through `TAKE`.
const TRACKER_JS: &str = "{
    const pending = new Map();
    globalThis.__inca_track_rejection__ = (promise, reason, handled) =>
        handled ? pending.delete(promise) : pending.set(promise, reason);
    globalThis.__inca_take_rejections__ = () => {
        const reasons = [...pending.values()];
        pending.clear();
        return reasons;
    };
}";

pub struct Engine {
    // Kept alive for the lifetime of `context`, which internally holds a
    // reference-counted handle back to it; QuickJS ties runtime-wide state
    // (the heap, GC) to this handle rather than to the context.
    runtime: Runtime,
    context: Context,
    /// Jobs that threw while [`eval_module`](Self::eval_module) drove a
    /// module's top-level `await`. The next [`run_jobs`](Self::run_jobs)
    /// returns them first.
    startup_failures: RefCell<Vec<EngineError>>,
}

impl Engine {
    /// Creates a new engine with a fresh `QuickJS` runtime and context, with
    /// the standard set of built-in JS intrinsics (`Array`, `JSON`, ...)
    /// enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `QuickJS` runtime or context fails
    /// to initialize.
    pub fn new() -> EngineResult<Self> {
        Self::builder().build()
    }

    /// Starts building an [`Engine`] with one or more module roots — see
    /// [`EngineBuilder`].
    #[must_use]
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    fn from_runtime(runtime: Runtime) -> EngineResult<Self> {
        let context = Context::full(&runtime).map_err(|err| EngineError::plain(&err))?;
        let engine = Self {
            runtime,
            context,
            startup_failures: RefCell::new(Vec::new()),
        };
        engine
            .context
            .with(|ctx| ctx.eval::<(), _>(TRACKER_JS))
            .map_err(|err| EngineError::plain(&err))?;
        engine
            .runtime
            .set_host_promise_rejection_tracker(Some(Box::new(
                |ctx, promise, reason, is_handled| {
                    if let Ok(track) = ctx.globals().get::<_, Function>(TRACK) {
                        let _ = track.call::<_, ()>((promise, reason, is_handled));
                    }
                },
            )));
        Ok(engine)
    }

    /// Runs every pending promise job and returns each failure the app left
    /// unreported: a job that threw, and a promise still rejected with no
    /// handler once the queue is empty. A rejection someone handles during
    /// the same drain is not a failure. A job that threw while
    /// [`eval_module`](Self::eval_module) awaited comes first.
    ///
    /// Call it outside [`with`](Self::with), and handle the result outside
    /// it too: the handling code may re-enter the engine.
    #[must_use]
    pub fn run_jobs(&self) -> Vec<EngineError> {
        let mut failures = self.startup_failures.take();
        while self.run_job(&mut failures) {}
        let reasons = self.context.with(|ctx| {
            ctx.globals()
                .get::<_, Function>(TAKE)
                .and_then(|take| take.call::<_, Vec<Value>>(()))
                .map(|values| values.iter().map(EngineError::from_thrown).collect())
                .unwrap_or_default()
        });
        failures.extend::<Vec<EngineError>>(reasons);
        failures
    }

    /// Runs one pending promise job. A job that threw is appended to
    /// `failures`. Returns whether a job was pending.
    ///
    /// Call it outside [`with`](Self::with): it takes the context itself, and
    /// nesting that panics.
    fn run_job(&self, failures: &mut Vec<EngineError>) -> bool {
        match self.runtime.execute_pending_job() {
            Ok(ran) => ran,
            Err(job) => {
                job.0.with(|ctx| {
                    failures.push(EngineError::capture(&ctx, &rquickjs::Error::Exception));
                });
                true
            }
        }
    }

    /// Evaluates `source` as a JS script and converts its completion value
    /// to `V`. A syntax error, a thrown exception, or a value that can't
    /// convert to `V` are all returned as an `Err`, never a panic.
    ///
    /// # Errors
    ///
    /// Returns an error for a JS syntax error, an uncaught thrown exception,
    /// or a completion value that doesn't convert to `V`.
    pub fn eval<V>(&self, source: &str) -> EngineResult<V>
    where
        V: for<'js> FromJs<'js>,
    {
        self.context.with(|ctx| {
            ctx.eval(source)
                .map_err(|err| EngineError::capture(&ctx, &err))
        })
    }

    /// Runs `f` with access to this engine's [`Ctx`], for operations `eval`
    /// doesn't cover — such as registering native functions on the global
    /// object.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx<'_>) -> R,
    {
        self.context.with(f)
    }

    /// Declares and evaluates `source` as an ES module named `name`, driving
    /// its top-level evaluation to completion before returning.
    ///
    /// An unresolved `import` in `source` resolves against this engine's
    /// module roots (see [`EngineBuilder::module_root`]) — against nothing,
    /// if there are none, so `source` then has to be fully self-contained.
    /// `name` also doubles as the base a relative `import` in `source`
    /// resolves against, so pass the source's real path on disk if it has
    /// one. A module's own completion value is always `undefined` per spec,
    /// so unlike [`eval`](Self::eval) there's nothing meaningful to convert
    /// to a caller-chosen type; state comes back out the same way a plain
    /// script's does — the module's top-level code writes to `globalThis`,
    /// and a separate `eval` call reads it back afterward.
    ///
    /// # Errors
    ///
    /// Returns an error for a syntax/link error, an uncaught exception
    /// thrown during evaluation, or an unsettled promise if the module
    /// awaits something with no pending job left to drive it (not expected
    /// for a self-contained module with no top-level `await`).
    ///
    /// A job that throws while the module awaits does not fail the module.
    /// The next [`run_jobs`](Self::run_jobs) returns it.
    pub fn eval_module(&self, name: &str, source: &str) -> EngineResult<()> {
        let mut pending = self.context.with(|ctx| {
            Module::declare(ctx.clone(), name, source)
                .and_then(rquickjs::Module::eval)
                .map(|(_module, promise)| Persistent::save(&ctx, promise))
                .map_err(|err| EngineError::capture(&ctx, &err))
        })?;
        loop {
            let step = self.context.with(|ctx| {
                let promise = pending
                    .restore(&ctx)
                    .map_err(|err| EngineError::plain(&err))?;
                match promise.result::<()>() {
                    Some(result) => Ok(ControlFlow::Break(
                        result.map_err(|err| EngineError::capture(&ctx, &err)),
                    )),
                    None => Ok(ControlFlow::Continue(Persistent::save(&ctx, promise))),
                }
            });
            match step? {
                ControlFlow::Break(result) => return result,
                ControlFlow::Continue(promise) => pending = promise,
            }
            let mut failures = Vec::new();
            let ran = self.run_job(&mut failures);
            self.startup_failures.borrow_mut().extend(failures);
            if !ran {
                return self.context.with(|ctx| {
                    drop(pending.restore(&ctx));
                    Err(EngineError::plain(&rquickjs::Error::WouldBlock))
                });
            }
        }
    }
}

/// Builds an [`Engine`], configuring where its `import`s resolve against.
///
/// [`Engine::new`] is [`Engine::builder().build()`](Self::build) with no
/// module root — every `import` in evaluated source is then unresolved.
#[derive(Default)]
pub struct EngineBuilder {
    module_roots: Vec<PathBuf>,
}

impl EngineBuilder {
    /// Adds a directory a bare `import` specifier is searched in, and that a
    /// `./`- or `../`-relative specifier resolves against (via the
    /// importing module's own path, not this root directly). Call it more
    /// than once to add more than one root; each is tried in the order
    /// added.
    #[must_use]
    pub fn module_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.module_roots.push(root.into());
        self
    }

    /// Builds the configured [`Engine`].
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `QuickJS` runtime or context fails
    /// to initialize.
    pub fn build(self) -> EngineResult<Engine> {
        let runtime = Runtime::new().map_err(|err| EngineError::plain(&err))?;
        if !self.module_roots.is_empty() {
            runtime.set_loader(DiskResolver::new(self.module_roots), DiskLoader);
        }
        Engine::from_runtime(runtime)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::{env, fs};

    use super::*;

    /// A directory under the OS temp root, unique per test invocation, torn
    /// down on drop.
    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "inca-jsenv-test-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn eval_smoke_test() {
        let engine = Engine::new().unwrap();
        let result: i32 = engine.eval("1 + 2").unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn syntax_error_propagates_as_err() {
        let engine = Engine::new().unwrap();
        let result: EngineResult<i32> = engine.eval("1 +");
        assert!(result.is_err());
    }

    #[test]
    fn two_engines_do_not_share_globals() {
        let a = Engine::new().unwrap();
        let b = Engine::new().unwrap();

        a.eval::<()>("globalThis.probe = 42;").unwrap();

        let seen_by_a: i32 = a.eval("probe").unwrap();
        assert_eq!(seen_by_a, 42);

        let seen_by_b: String = b.eval("typeof probe").unwrap();
        assert_eq!(seen_by_b, "undefined");
    }

    #[test]
    fn eval_module_runs_top_level_code_to_completion() {
        let engine = Engine::new().unwrap();
        engine
            .eval_module(
                "probe.mjs",
                "export const answer = 41; globalThis.seen = answer + 1;",
            )
            .unwrap();

        let seen: i32 = engine.eval("globalThis.seen").unwrap();
        assert_eq!(seen, 42);
    }

    #[test]
    fn eval_module_uncaught_exception_propagates_as_err() {
        let engine = Engine::new().unwrap();
        let result = engine.eval_module("throws.mjs", "throw new Error('boom');");
        assert!(result.is_err());
    }

    #[test]
    fn an_error_gives_up_its_message_and_its_frames() {
        let engine = Engine::new().unwrap();
        let err = engine
            .eval_module("throws.mjs", "throw new Error('bad edit');")
            .unwrap_err();

        assert_eq!(err.message(), "Error: bad edit");
        assert!(
            err.stack().unwrap_or_default().contains("at "),
            "{:?}",
            err.stack()
        );
    }

    #[test]
    fn a_thrown_non_error_still_reads_as_something() {
        let engine = Engine::new().unwrap();
        let err = engine.eval::<()>("throw 'just a string';").unwrap_err();

        assert_eq!(err.message(), "just a string");
        assert_eq!(err.stack(), None, "only an Error carries frames");
    }

    #[test]
    fn a_conversion_failure_is_not_a_thrown_value() {
        let engine = Engine::new().unwrap();
        let err = engine.eval::<i32>("'not a number'").unwrap_err();

        assert!(!err.message().is_empty());
        assert_eq!(err.stack(), None);
    }

    #[test]
    fn display_leads_with_the_message() {
        let engine = Engine::new().unwrap();
        let err = engine.eval::<()>("throw new Error('boom');").unwrap_err();

        let shown = err.to_string();
        assert!(shown.starts_with("Error: boom"), "{shown}");
    }

    #[test]
    fn reading_one_failure_does_not_leak_into_the_next() {
        let engine = Engine::new().unwrap();

        let first = engine.eval::<()>("throw new Error('first');").unwrap_err();
        let second = engine.eval::<()>("throw new Error('second');").unwrap_err();

        assert_eq!(first.message(), "Error: first");
        assert_eq!(second.message(), "Error: second");
    }

    #[test]
    fn a_module_root_resolves_a_relative_import() {
        let dir = ScratchDir::new("relative-import");
        fs::write(dir.0.join("dep.js"), "export const value = 41;").unwrap();
        let entry = dir.0.join("entry.js");
        fs::write(
            &entry,
            "import { value } from './dep.js'; globalThis.seen = value + 1;",
        )
        .unwrap();

        let engine = Engine::builder().module_root(&dir.0).build().unwrap();
        engine
            .eval_module(
                &entry.to_string_lossy(),
                &fs::read_to_string(&entry).unwrap(),
            )
            .unwrap();

        let seen: i32 = engine.eval("globalThis.seen").unwrap();
        assert_eq!(seen, 42);
    }

    #[test]
    fn a_shared_dependency_is_evaluated_once_across_two_entries() {
        let dir = ScratchDir::new("shared-dependency");
        fs::write(
            dir.0.join("counter.js"),
            "let calls = 0; export function next() { calls += 1; return calls; }",
        )
        .unwrap();
        let first = dir.0.join("first.js");
        fs::write(
            &first,
            "import { next } from './counter.js'; globalThis.firstSeen = next();",
        )
        .unwrap();
        let second = dir.0.join("second.js");
        fs::write(
            &second,
            "import { next } from './counter.js'; globalThis.secondSeen = next();",
        )
        .unwrap();

        let engine = Engine::builder().module_root(&dir.0).build().unwrap();
        engine
            .eval_module(
                &first.to_string_lossy(),
                &fs::read_to_string(&first).unwrap(),
            )
            .unwrap();
        engine
            .eval_module(
                &second.to_string_lossy(),
                &fs::read_to_string(&second).unwrap(),
            )
            .unwrap();

        // Both entries import the same `counter.js` — one shared module
        // instance means its call counter keeps incrementing rather than
        // restarting, so the second entry sees `2`, not `1`.
        let first_seen: i32 = engine.eval("globalThis.firstSeen").unwrap();
        let second_seen: i32 = engine.eval("globalThis.secondSeen").unwrap();
        assert_eq!((first_seen, second_seen), (1, 2));
    }

    #[test]
    fn a_shared_dependency_reached_by_two_different_relative_paths_is_one_module() {
        let dir = ScratchDir::new("shared-dependency-path-variants");
        fs::write(
            dir.0.join("shared.js"),
            "let calls = 0; export function next() { calls += 1; return calls; }",
        )
        .unwrap();

        let a_dir = dir.0.join("a");
        fs::create_dir_all(&a_dir).unwrap();
        let first = a_dir.join("x.js");
        fs::write(
            &first,
            "import { next } from '../shared.js'; globalThis.firstSeen = next();",
        )
        .unwrap();

        let b_dir = dir.0.join("b");
        fs::create_dir_all(&b_dir).unwrap();
        let second = b_dir.join("y.js");
        fs::write(
            &second,
            "import { next } from '../shared.js'; globalThis.secondSeen = next();",
        )
        .unwrap();

        let engine = Engine::builder().module_root(&dir.0).build().unwrap();
        engine
            .eval_module(
                &first.to_string_lossy(),
                &fs::read_to_string(&first).unwrap(),
            )
            .unwrap();
        engine
            .eval_module(
                &second.to_string_lossy(),
                &fs::read_to_string(&second).unwrap(),
            )
            .unwrap();

        // `a/x.js` and `b/y.js` both resolve `../shared.js` to the same
        // file, but through textually different paths (`a/../shared.js`
        // vs. `b/../shared.js`). Canonicalizing before caching means one
        // module instance, so the second entry sees `2`, not `1`.
        let first_seen: i32 = engine.eval("globalThis.firstSeen").unwrap();
        let second_seen: i32 = engine.eval("globalThis.secondSeen").unwrap();
        assert_eq!((first_seen, second_seen), (1, 2));
    }

    #[test]
    fn a_dynamic_import_settles_once_pending_jobs_are_drained() {
        let dir = ScratchDir::new("dynamic-import");
        fs::write(dir.0.join("dep.js"), "export const value = 41;").unwrap();
        let entry = dir.0.join("entry.js");
        fs::write(
            &entry,
            "import('./dep.js').then((m) => { globalThis.seen = m.value + 1; });",
        )
        .unwrap();

        let engine = Engine::builder().module_root(&dir.0).build().unwrap();
        engine
            .eval_module(
                &entry.to_string_lossy(),
                &fs::read_to_string(&entry).unwrap(),
            )
            .unwrap();
        let _ = engine.run_jobs();

        let seen: i32 = engine.eval("globalThis.seen").unwrap();
        assert_eq!(seen, 42);
    }

    #[test]
    fn an_unhandled_rejection_is_returned_once() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>("Promise.reject(new Error('lost'));")
            .unwrap();

        let failures = engine.run_jobs();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].message(), "Error: lost");
        assert!(failures[0].stack().is_some());
        assert!(engine.run_jobs().is_empty());
    }

    #[test]
    fn a_rejection_handled_later_in_the_drain_is_not_returned() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>(
                "const p = Promise.reject(new Error('late'));\n\
                 Promise.resolve().then(() => p.catch(() => {}));",
            )
            .unwrap();

        assert!(engine.run_jobs().is_empty());
    }

    #[test]
    fn an_async_function_that_throws_is_returned() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>("(async () => { throw new Error('async boom'); })();")
            .unwrap();

        let failures = engine.run_jobs();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].message(), "Error: async boom");
    }

    #[test]
    fn a_job_that_throws_is_returned() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>("queueMicrotask(() => { throw new Error('job'); });")
            .unwrap();

        let failures = engine.run_jobs();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].message(), "Error: job");
        assert!(failures[0].stack().is_some());
    }

    #[test]
    fn a_job_that_throws_during_top_level_await_is_returned_by_run_jobs() {
        let engine = Engine::new().unwrap();
        engine
            .eval_module(
                "startup.mjs",
                "queueMicrotask(() => { throw new Error('startup job'); }); await null;",
            )
            .unwrap();

        let failures = engine.run_jobs();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].message().contains("startup job"));
        assert!(engine.run_jobs().is_empty());
    }

    #[test]
    fn a_top_level_await_that_resolves_completes_the_module() {
        let engine = Engine::new().unwrap();
        engine
            .eval_module(
                "awaits.mjs",
                "await Promise.resolve(); globalThis.seen = 42;",
            )
            .unwrap();

        let seen: i32 = engine.eval("globalThis.seen").unwrap();
        assert_eq!(seen, 42);
        assert!(engine.run_jobs().is_empty());
    }

    #[test]
    fn a_top_level_await_that_rejects_returns_its_own_error() {
        let engine = Engine::new().unwrap();
        let err = engine
            .eval_module(
                "rejects.mjs",
                "await Promise.resolve(); throw new Error('late boom');",
            )
            .unwrap_err();

        assert_eq!(err.message(), "Error: late boom");
    }

    #[test]
    fn a_top_level_await_that_never_settles_would_block() {
        let engine = Engine::new().unwrap();
        let err = engine
            .eval_module("stuck.mjs", "await new Promise(() => {});")
            .unwrap_err();

        assert!(!err.message().is_empty());
        assert_eq!(err.stack(), None);
    }

    #[test]
    fn no_module_root_leaves_an_import_unresolved() {
        let engine = Engine::new().unwrap();
        let result = engine.eval_module("entry.js", "import './dep.js';");
        assert!(result.is_err());
    }
}
