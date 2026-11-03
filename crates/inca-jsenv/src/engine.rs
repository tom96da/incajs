// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `QuickJS` runtime bootstrap.
//!
//! An [`Engine`] pairs one `QuickJS` runtime with one execution context. Each
//! `Engine` is fully independent: creating a global on one has no effect on
//! any other `Engine`, since they don't share a runtime or a heap.

use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use rquickjs::{Coerced, Context, Ctx, FromJs, Function, Object, Runtime, Value};

use crate::limits::{Limits, Watchdog, interrupted, null_under_memory_limit, record_limits};
use crate::loader::{DiskLoader, DiskResolver};
use crate::sourcemap::{StackPaths, remap_stack};
use crate::timers::{Clock, system_clock};
use crate::vault::{create_vault, vault};

mod jobs;

use jobs::{MODULES, TRACK, TRACKER_JS};
pub use jobs::{PendingModule, report_jobs};

/// `Result<T, EngineError>`, what every fallible [`Engine`] method returns.
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
    /// `[object Object]`. A stop by the time limit and a `null` thrown under a
    /// memory limit read with the limit named.
    fn from_thrown(thrown: &Value<'_>) -> Self {
        let Some(object) = thrown.as_object().filter(|_| thrown.is_error()) else {
            let named = thrown
                .is_null()
                .then(|| null_under_memory_limit(thrown.ctx()))
                .flatten();
            return Self {
                message: named.unwrap_or_else(|| {
                    thrown
                        .get::<Coerced<String>>()
                        .map_or_else(|_| "uncaught, unreadable value".to_owned(), |text| text.0)
                }),
                stack: None,
            };
        };

        let name = object
            .get::<_, Option<String>>("name")
            .ok()
            .flatten()
            .unwrap_or_else(|| "Error".to_owned());
        let message = match object.get::<_, Option<String>>("message").ok().flatten() {
            Some(message) if !message.is_empty() => interrupted(thrown.ctx(), &name, &message)
                .unwrap_or_else(|| format!("{name}: {message}")),
            _ => name,
        };
        let stack = object
            .get::<_, Option<String>>("stack")
            .ok()
            .flatten()
            .filter(|stack| !stack.trim().is_empty())
            .map(|stack| remap_stack(thrown.ctx(), stack.trim_end()));

        Self { message, stack }
    }

    /// The thrown value as text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The frames behind it, or `None`: `QuickJS` records them only for an
    /// `Error`, and only the frames: the message is separate. A frame
    /// in a file with a source map reads as its original position.
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

/// One `QuickJS` runtime with the single context it runs scripts in.
///
/// Build one with [`Engine::new`] or [`Engine::builder`]. Run a script with
/// [`eval`](Self::eval) and a module with
/// [`eval_module`](Self::eval_module). A module whose top-level `await`
/// settles later runs with [`start_module`](Self::start_module) and
/// [`poll_module`](Self::poll_module). Run the promise jobs they queue with
/// [`run_jobs`](Self::run_jobs). Reach the raw context through
/// [`with`](Self::with). An `Engine` is neither `Send` nor `Sync`, so it
/// stays on the thread that created it. Dropping it frees the runtime and
/// its realm.
///
/// [`EngineBuilder::limits`] bounds what a script may use. `eval`, `with`,
/// `start_module`, `poll_module` and `run_jobs` are each one call with its own
/// time limit.
pub struct Engine {
    // Kept alive for the lifetime of `context`, which holds a reference-counted
    // handle back to it. QuickJS ties runtime-wide state (the heap, GC) to this handle.
    pub(crate) runtime: Runtime,
    context: Context,
    /// Jobs that threw while [`poll_module`](Self::poll_module) drove a
    /// module's top-level `await`. The next [`run_jobs`](Self::run_jobs)
    /// returns them first.
    startup_failures: RefCell<Vec<EngineError>>,
    /// Stops the call that outlives the time limit.
    watchdog: Rc<Watchdog>,
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

    /// Starts building an [`Engine`] with one or more module roots. See
    /// [`EngineBuilder`].
    #[must_use]
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    fn from_runtime(runtime: Runtime, limits: Limits, clock: &Clock) -> EngineResult<Self> {
        let context = Context::full(&runtime).map_err(|err| EngineError::plain(&err))?;
        let engine = Self {
            runtime,
            context,
            startup_failures: RefCell::new(Vec::new()),
            watchdog: Rc::new(Watchdog::new(limits.time)),
        };
        engine
            .context
            .with(|ctx| {
                create_vault(&ctx)?;
                crate::time::install(&ctx, clock)?;
                crate::encoding::install(&ctx)?;
                let modules = Object::new(ctx.clone())?;
                modules.set_prototype(None)?;
                let vault = vault(&ctx)?;
                vault.set(MODULES, modules)?;
                ctx.eval::<Function, _>(TRACKER_JS)?
                    .call::<_, ()>((vault,))?;
                record_limits(&ctx, limits)
            })
            .map_err(|err| EngineError::plain(&err))?;
        engine
            .runtime
            .set_host_promise_rejection_tracker(Some(Box::new(
                |ctx, promise, reason, is_handled| {
                    if let Ok(track) = vault(&ctx).and_then(|vault| vault.get::<_, Function>(TRACK))
                    {
                        let _ = track.call::<_, ()>((promise, reason, is_handled));
                    }
                },
            )));
        // Set after the bootstrap, so a small limit cannot make `build` fail.
        if let Some(stack) = limits.stack_limit() {
            engine.runtime.set_max_stack_size(stack);
        }
        if limits.memory != 0 {
            engine.runtime.set_memory_limit(limits.memory);
        }
        if !limits.time.is_zero() {
            let watchdog = Rc::clone(&engine.watchdog);
            engine
                .runtime
                .set_interrupt_handler(Some(Box::new(move || watchdog.expired())));
        }
        Ok(engine)
    }

    /// Evaluates `source` as a JS script and converts its completion value
    /// to `V`. A syntax error, a thrown exception, or a value that can't
    /// convert to `V` are all returned as an `Err`.
    ///
    /// # Errors
    ///
    /// Returns an error for a JS syntax error, an uncaught thrown exception,
    /// or a completion value that doesn't convert to `V`.
    pub fn eval<V>(&self, source: &str) -> EngineResult<V>
    where
        V: for<'js> FromJs<'js>,
    {
        self.watchdog.arm();
        self.context.with(|ctx| {
            ctx.eval(source)
                .map_err(|err| EngineError::capture(&ctx, &err))
        })
    }

    /// Runs `f` with access to this engine's [`Ctx`], for operations `eval`
    /// doesn't cover, such as registering native functions on the global
    /// object. The JS that `f` runs shares one time limit.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx<'_>) -> R,
    {
        self.watchdog.arm();
        self.context.with(f)
    }

    /// Declares and evaluates `source` as an ES module named `name`, driving
    /// its top-level evaluation to completion before returning.
    ///
    /// An unresolved `import` in `source` resolves against this engine's
    /// module roots (see [`EngineBuilder::module_root`]), or against nothing
    /// if there are none, so `source` then has to be fully self-contained.
    /// `name` also doubles as the base a relative `import` in `source`
    /// resolves against, so pass the source's real path on disk if it has
    /// one; it is canonicalized when it names a file.
    ///
    /// A module's own completion value is always `undefined` per spec,
    /// so unlike [`eval`](Self::eval) there's nothing meaningful to convert
    /// to a caller-chosen type; state comes back out the same way a plain
    /// script's does: the module's top-level code writes to `globalThis`,
    /// and a separate `eval` call reads it back afterward.
    ///
    /// This is [`start_module`](Self::start_module) followed by one
    /// [`poll_module`](Self::poll_module). A module that is still pending
    /// after that poll is an error. The failure of a job that throws while
    /// the module awaits goes to the next [`run_jobs`](Self::run_jobs), and
    /// the module's result is unaffected.
    ///
    /// # Errors
    ///
    /// Returns an error for a syntax/link error, an uncaught exception
    /// thrown during evaluation, or an unsettled promise if the module
    /// awaits something with no pending job left to drive it (not expected
    /// for a self-contained module with no top-level `await`).
    pub fn eval_module(&self, name: &str, source: &str) -> EngineResult<()> {
        let module = self.start_module(name, source)?;
        self.poll_module(&module)
            .unwrap_or_else(|| Err(EngineError::plain(&rquickjs::Error::WouldBlock)))
    }
}

/// Builds an [`Engine`], configuring where its `import`s resolve against.
///
/// [`Engine::new`] is [`Engine::builder().build()`](Self::build) with no
/// module root. Every `import` in evaluated source is then unresolved.
/// [`limits`](Self::limits) bounds memory, stack and time.
#[derive(Default)]
pub struct EngineBuilder {
    module_roots: Vec<PathBuf>,
    paths: StackPaths,
    limits: Limits,
    clock: Option<Clock>,
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

    /// Sets how a stack names a file. Defaults to [`StackPaths::Absolute`].
    #[must_use]
    pub fn stack_paths(mut self, paths: StackPaths) -> Self {
        self.paths = paths;
        self
    }

    /// Sets what a script may use. Defaults to [`Limits::default`], which sets
    /// none.
    #[must_use]
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Sets the clock `performance.now()` reads, counted from the build.
    /// Defaults to [`system_clock`].
    #[must_use]
    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = Some(clock);
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
        let engine = Engine::from_runtime(
            runtime,
            self.limits,
            &self.clock.unwrap_or_else(system_clock),
        )?;
        engine
            .with(|ctx| crate::sourcemap::set_paths(&ctx, self.paths))
            .map_err(|err| EngineError::plain(&err))?;
        Ok(engine)
    }
}

/// Where a failure inside the running app goes. Nothing asked for it, so
/// there is no caller to return it to and no `Result` to put it in.
pub type ErrorReporter = Rc<dyn Fn(&EngineError)>;

/// The default [`ErrorReporter`]: one line per failure on stderr.
#[must_use]
pub fn stderr_reporter() -> ErrorReporter {
    Rc::new(|err| eprintln!("{err}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs;

    use super::*;
    use crate::tests::ScratchDir;

    #[test]
    fn a_syntax_error_is_returned_as_an_err() {
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
    fn a_frame_of_a_compiled_function_reads_at_its_source() {
        let engine = Engine::new().unwrap();
        let body = crate::sourcemap::mapped_body("throw new Error('bad edit');");
        engine.with(|ctx| {
            let f = crate::compile_async_function(&ctx, &[], &body, "virtual.js").unwrap();
            ctx.globals().set("f", f).unwrap();
        });
        engine.eval::<()>("f();").unwrap();
        let errors = engine.run_jobs();

        let stack = errors[0].stack().unwrap();
        assert!(stack.contains("src/app.ts:12:5"), "{stack}");
        assert!(!stack.contains("virtual.js:"), "{stack}");
    }

    #[test]
    fn an_engine_without_stack_paths_names_files_in_full() {
        let engine = Engine::new().unwrap();
        let err = engine
            .eval_module("/app/dist/probe.mjs", "throw new Error('x');")
            .unwrap_err();

        let stack = err.stack().unwrap();
        assert!(stack.contains("/app/dist/probe.mjs:1:"), "{stack}");
    }

    #[test]
    fn a_frame_of_a_module_reads_through_its_sidecar_map() {
        let dir = ScratchDir::new("module-sidecar-map");
        fs::write(
            dir.0.join("bundle.js.map"),
            r#"{"version":3,"sources":["../src/app.ts"],"mappings":";AAWI"}"#,
        )
        .unwrap();
        let entry = dir.0.join("bundle.js");
        let source = "globalThis.ready = true;\nthrow new Error('bad edit');\n//# sourceMappingURL=bundle.js.map";

        let engine = Engine::new().unwrap();
        let err = engine
            .eval_module(&entry.to_string_lossy(), source)
            .unwrap_err();

        let stack = err.stack().unwrap();
        assert!(stack.contains("src/app.ts:12:5"), "{stack}");
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

        // Both entries import the same `counter.js`. One shared module
        // instance means its call counter keeps incrementing, so the second entry
        // sees `2`.
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
    fn a_module_importing_its_entry_back_reaches_the_entry_itself() {
        let dir = ScratchDir::new("import-entry-back");
        fs::create_dir_all(dir.0.join("sub")).unwrap();
        let entry = dir.0.join("sub/../entry.js");
        let source = "globalThis.runs = (globalThis.runs ?? 0) + 1; import('./back.js');";
        fs::write(dir.0.join("entry.js"), source).unwrap();
        fs::write(dir.0.join("back.js"), "import './entry.js';").unwrap();

        let engine = Engine::builder().module_root(&dir.0).build().unwrap();
        engine
            .eval_module(&entry.to_string_lossy(), source)
            .unwrap();
        let _ = engine.run_jobs();

        let runs: i32 = engine.eval("globalThis.runs").unwrap();
        assert_eq!(runs, 1);
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
}
