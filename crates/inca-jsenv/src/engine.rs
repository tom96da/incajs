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
use std::sync::atomic::{AtomicU32, Ordering};

use rquickjs::function::Rest;
use rquickjs::{Coerced, Context, Ctx, FromJs, Function, Module, Object, Promise, Runtime, Value};

use crate::limits::{Limits, Watchdog, interrupted, null_under_memory_limit, record_limits};
use crate::loader::{DiskLoader, DiskResolver, canonical_name};
use crate::sourcemap::{StackPaths, remap_stack};
use crate::vault::{create_vault, vault};

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

/// The vault key of the function that fills the rejection tracker's map.
const TRACK: &str = "trackRejection";
/// The vault key of the function [`Engine::run_jobs`] empties the map with.
const TAKE: &str = "takeRejections";

/// Puts the rejection tracker's functions into the vault it is called with.
/// They keep each promise rejected with no handler yet, mapped to its reason,
/// and read the `Map` methods they were made with, so a script that replaces
/// `Map.prototype` members changes nothing. `takeRejections(each)` calls
/// `each(reason)` per entry, then empties the map.
const TRACKER_JS: &str = r"
    (vault) => {
        const { apply } = Reflect;
        const { delete: remove, set, forEach, clear } = Map.prototype;
        const pending = new Map();
        vault.trackRejection = (promise, reason, handled) => {
            if (handled) {
                apply(remove, pending, [promise]);
            } else {
                apply(set, pending, [promise, reason]);
            }
        };
        vault.takeRejections = (each) => {
            apply(forEach, pending, [each]);
            apply(clear, pending, []);
        };
    }
    ";

/// The vault key of the null-prototype object that keeps the promise of each
/// started module under its id.
const MODULES: &str = "modules";

/// The id of the next started module. One counter serves every engine, so an
/// id names one module in the process.
static NEXT_MODULE: AtomicU32 = AtomicU32::new(0);

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
    runtime: Runtime,
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

    fn from_runtime(runtime: Runtime, limits: Limits) -> EngineResult<Self> {
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

    /// Runs every pending promise job and returns each failure the app left
    /// unreported: a job that threw, and a promise still rejected with no
    /// handler once the queue is empty. A rejection someone handles during
    /// the same drain is not a failure. A job that threw while
    /// [`eval_module`](Self::eval_module) or [`poll_module`](Self::poll_module)
    /// ran comes first.
    ///
    /// The drain is one call with one time limit. A job that outlives it fails,
    /// and the jobs that are left wait for the next call.
    ///
    /// Call it outside [`with`](Self::with), and handle the result outside
    /// it too: the handling code may re-enter the engine.
    #[must_use]
    pub fn run_jobs(&self) -> Vec<EngineError> {
        self.watchdog.arm();
        let mut failures = self.startup_failures.take();
        while self.run_job(&mut failures) && !self.watchdog.tripped() {}
        failures.extend(self.context.with(|ctx| take_rejections(&ctx)));
        failures
    }

    /// Takes the failures of jobs that threw while
    /// [`eval_module`](Self::eval_module) or [`poll_module`](Self::poll_module)
    /// ran, oldest first. The next [`run_jobs`](Self::run_jobs) starts from an
    /// empty list. Call it to report the failures when the module's own error
    /// is reported separately.
    #[must_use]
    pub fn take_startup_failures(&self) -> Vec<EngineError> {
        self.startup_failures.take()
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

    /// Declares `source` as an ES module named `name` and evaluates it up to
    /// its first top-level `await`. Drive it with
    /// [`poll_module`](Self::poll_module).
    ///
    /// `name` and `import` resolution follow
    /// [`eval_module`](Self::eval_module). A frame of a module whose source
    /// ends in a `//# sourceMappingURL=` line names the original source.
    ///
    /// # Errors
    ///
    /// Returns an error for a syntax or link error. An exception the module
    /// throws while evaluating comes from [`poll_module`](Self::poll_module).
    pub fn start_module(&self, name: &str, source: &str) -> EngineResult<PendingModule> {
        self.watchdog.arm();
        let name = canonical_name(name);
        let id = NEXT_MODULE.fetch_add(1, Ordering::Relaxed);
        self.context.with(|ctx| {
            crate::sourcemap::register(&ctx, &name, source);
            Module::declare(ctx.clone(), name, source)
                .and_then(rquickjs::Module::eval)
                .and_then(|(_module, promise)| {
                    vault(&ctx)?.get::<_, Object>(MODULES)?.set(id, promise)
                })
                .map(|()| PendingModule { id })
                .map_err(|err| EngineError::capture(&ctx, &err))
        })
    }

    /// Runs promise jobs until `module` settles or the job queue is empty.
    ///
    /// Returns `None` while the module is pending. Settle it by evaluating
    /// code that resolves what it awaits, then poll again. Returns
    /// `Some(Ok(()))` once the module has completed, and `Some(Err(_))` once
    /// it has thrown. Every later poll returns the same result.
    ///
    /// The next [`run_jobs`](Self::run_jobs) returns the failure of a job
    /// that throws during a poll, whether the module is pending or settled,
    /// and the error of a module that settled with `Some(Err(_))`. The
    /// module's result is unaffected.
    ///
    /// The poll is one call with one time limit, and a job that outlives it ends
    /// the poll with `None`.
    ///
    /// Call it outside [`with`](Self::with).
    ///
    /// # Errors
    ///
    /// Returns `Some(Err(_))` for an exception the module threw while
    /// evaluating, or for a module that another engine started.
    pub fn poll_module(&self, module: &PendingModule) -> Option<EngineResult<()>> {
        // A module that start_module stopped reads its error before the arm forgets the stop.
        if let Some(result) = self.settled(module) {
            return Some(result);
        }
        self.watchdog.arm();
        loop {
            if let Some(result) = self.settled(module) {
                return Some(result);
            }
            let mut failures = Vec::new();
            let ran = self.run_job(&mut failures);
            self.startup_failures.borrow_mut().extend(failures);
            if !ran || self.watchdog.tripped() {
                return None;
            }
        }
    }

    /// The module's result, or `None` while its promise is pending.
    fn settled(&self, module: &PendingModule) -> Option<EngineResult<()>> {
        self.context.with(|ctx| {
            let promise = vault(&ctx)
                .and_then(|vault| vault.get::<_, Object>(MODULES))
                .and_then(|modules| modules.get::<_, Promise>(module.id));
            match promise {
                Ok(promise) => promise
                    .result::<()>()
                    .map(|result| result.map_err(|err| EngineError::capture(&ctx, &err))),
                Err(err) => Some(Err(EngineError::capture(&ctx, &err))),
            }
        })
    }
}

/// The reasons of the promises still rejected with no handler, which this
/// forgets. A failed take reports what it collected.
fn take_rejections<'js>(ctx: &Ctx<'js>) -> Vec<EngineError> {
    let reasons = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&reasons);
    let _ = Function::new(ctx.clone(), move |args: Rest<Value<'js>>| {
        if let Some(reason) = args.0.into_iter().next() {
            sink.borrow_mut().push(reason);
        }
    })
    .and_then(|each| vault(ctx)?.get::<_, Function>(TAKE)?.call::<_, ()>((each,)));
    reasons
        .take()
        .iter()
        .map(EngineError::from_thrown)
        .collect()
}

/// A module started by [`Engine::start_module`] and polled with
/// [`Engine::poll_module`]. The engine that started it keeps its promise until
/// the engine drops.
pub struct PendingModule {
    /// The key of the module's promise in the engine's module table. Another
    /// engine holds no promise under it.
    id: u32,
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
        let engine = Engine::from_runtime(runtime, self.limits)?;
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

/// Runs `engine`'s pending promise jobs and sends each failure to `reporter`:
/// a job that threw, including during module startup, and a promise rejected
/// with no handler. Use it where no window is open yet.
///
/// Call it outside any [`Engine::with`]: it takes the context itself, and
/// nesting that panics with "`RefCell` already borrowed".
pub fn report_jobs(engine: &Engine, reporter: &ErrorReporter) {
    for failure in engine.run_jobs() {
        reporter(&failure);
    }
}

#[cfg(test)]
mod tests;
