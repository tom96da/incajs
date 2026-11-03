// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The promise jobs of an [`Engine`] and the modules that wait on them.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use rquickjs::function::Rest;
use rquickjs::{Ctx, Function, Module, Object, Promise, Value};

use super::{Engine, EngineError, EngineResult, ErrorReporter};
use crate::loader::canonical_name;
use crate::vault::vault;

/// The vault key of the function that fills the rejection tracker's map.
pub(super) const TRACK: &str = "trackRejection";
/// The vault key of the function [`Engine::run_jobs`] empties the map with.
pub(super) const TAKE: &str = "takeRejections";

/// Puts the rejection tracker's functions into the vault it is called with.
/// They keep each promise rejected with no handler yet, mapped to its reason,
/// and read the `Map` methods they were made with, so a script that replaces
/// `Map.prototype` members changes nothing. `takeRejections(each)` calls
/// `each(reason)` per entry, then empties the map.
pub(super) const TRACKER_JS: &str = r"
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
pub(super) const MODULES: &str = "modules";

/// The id of the next started module. One counter serves every engine, so an
/// id names one module in the process.
static NEXT_MODULE: AtomicU32 = AtomicU32::new(0);

impl Engine {
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
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;

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
                r"
                const p = Promise.reject(new Error('late'));
                Promise.resolve().then(() => p.catch(() => {}));
                ",
            )
            .unwrap();

        assert!(engine.run_jobs().is_empty());
    }

    #[rstest]
    #[case::map_set(
        r"
        Map.prototype.set = function () {
            return this;
        };
        "
    )]
    #[case::map_iterator(
        r"
        Object.getPrototypeOf(new Map().values()).next = () => ({ done: true });
        "
    )]
    #[case::map_for_each(
        r"
        Map.prototype.forEach = function () {};
        "
    )]
    fn a_script_cannot_silence_an_unhandled_rejection(#[case] tamper: &str) {
        let engine = Engine::new().unwrap();
        engine.eval::<()>(tamper).unwrap();
        engine
            .eval::<()>("Promise.reject(new Error('lost'));")
            .unwrap();

        assert_eq!(engine.run_jobs().len(), 1);
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
    fn a_pending_module_settles_once_a_later_eval_resolves_it() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module(
                "pending.mjs",
                "await new Promise((r) => { globalThis.release = r; }); globalThis.seen = 42;",
            )
            .unwrap();

        assert!(engine.poll_module(&module).is_none());
        engine.eval::<()>("release();").unwrap();
        assert_eq!(engine.poll_module(&module), Some(Ok(())));

        let seen: i32 = engine.eval("globalThis.seen").unwrap();
        assert_eq!(seen, 42);
    }

    #[test]
    fn a_module_cannot_replace_its_own_promise() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module(
                "steals.mjs",
                r"
                for (let key = 0; key < 65536; key++) {
                    Object.defineProperty(Object.prototype, key, {
                        get() {
                            return Promise.resolve();
                        },
                        set(promise) {
                            globalThis.stolen = promise;
                        },
                    });
                }
                await new Promise(() => {});
                ",
            )
            .unwrap();

        assert!(engine.poll_module(&module).is_none());
        let stolen: String = engine.eval("typeof globalThis.stolen").unwrap();
        assert_eq!(stolen, "undefined");
    }

    #[test]
    fn a_pending_module_returns_a_later_rejection_as_its_own_error() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module(
                "pending-rejects.mjs",
                "await new Promise((_, reject) => { globalThis.fail = reject; });",
            )
            .unwrap();

        assert!(engine.poll_module(&module).is_none());
        engine.eval::<()>("fail(new Error('later'));").unwrap();
        let err = engine.poll_module(&module).unwrap().unwrap_err();
        assert_eq!(err.message(), "Error: later");
    }

    #[test]
    fn a_job_that_throws_during_a_poll_is_returned_by_run_jobs() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module(
                "poll-job.mjs",
                r"
                queueMicrotask(() => { throw new Error('poll job'); });
                await new Promise((r) => { globalThis.release = r; });
                ",
            )
            .unwrap();

        assert!(engine.poll_module(&module).is_none());
        let failures = engine.run_jobs();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].message(), "Error: poll job");
        assert!(engine.run_jobs().is_empty());

        engine.eval::<()>("release();").unwrap();
        assert_eq!(engine.poll_module(&module), Some(Ok(())));
    }

    #[test]
    fn polling_a_completed_module_again_returns_ok() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module("done.mjs", "await Promise.resolve();")
            .unwrap();

        assert_eq!(engine.poll_module(&module), Some(Ok(())));
        assert_eq!(engine.poll_module(&module), Some(Ok(())));
    }

    #[test]
    fn polling_a_rejected_module_again_returns_the_same_error() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module(
                "rejected.mjs",
                "await Promise.resolve(); throw new Error('again');",
            )
            .unwrap();

        let first = engine.poll_module(&module).unwrap().unwrap_err();
        let second = engine.poll_module(&module).unwrap().unwrap_err();
        assert_eq!(first.message(), "Error: again");
        assert_eq!(first, second);
    }

    #[test]
    fn a_module_from_another_engine_is_an_error() {
        let other = Engine::new().unwrap();
        let module = other
            .start_module("foreign.mjs", "await new Promise(() => {});")
            .unwrap();
        let engine = Engine::new().unwrap();
        let _own = engine
            .start_module("own.mjs", "await new Promise(() => {});")
            .unwrap();

        let err = engine.poll_module(&module).unwrap().unwrap_err();
        assert!(!err.message().is_empty());
        assert_eq!(err.stack(), None);
    }

    #[test]
    fn a_settled_rejection_is_also_returned_by_run_jobs() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module(
                "settled.mjs",
                "await Promise.resolve(); throw new Error('settled');",
            )
            .unwrap();

        let err = engine.poll_module(&module).unwrap().unwrap_err();
        let failures = engine.run_jobs();
        assert_eq!(failures, vec![err]);
        assert!(engine.run_jobs().is_empty());
    }

    #[test]
    fn run_jobs_returns_the_jobs_that_threw_during_the_poll_before_the_entry_error() {
        let engine = Engine::new().unwrap();
        let module = engine
            .start_module(
                "taken.mjs",
                r"
                queueMicrotask(() => { throw new Error('side'); });
                await Promise.resolve(); throw new Error('entry');
                ",
            )
            .unwrap();

        let entry = engine.poll_module(&module).unwrap().unwrap_err();
        assert_eq!(entry.message(), "Error: entry");
        let failures = engine.run_jobs();
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].message(), "Error: side");
        assert_eq!(failures[1], entry);
        assert!(engine.run_jobs().is_empty());
    }

    #[test]
    fn no_module_root_leaves_an_import_unresolved() {
        let engine = Engine::new().unwrap();
        let result = engine.eval_module("entry.js", "import './dep.js';");
        assert!(result.is_err());
    }

    #[test]
    fn report_jobs_delivers_a_failed_job_once() {
        let engine = Engine::new().unwrap();
        engine
            .eval::<()>("Promise.resolve().then(() => { throw new Error('boom'); });")
            .unwrap();
        let reported: Rc<RefCell<Vec<EngineError>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&reported);
        let report: ErrorReporter = Rc::new(move |err| sink.borrow_mut().push(err.clone()));

        report_jobs(&engine, &report);
        assert_eq!(reported.borrow().len(), 1);
        assert_eq!(reported.borrow()[0].message(), "Error: boom");

        report_jobs(&engine, &report);
        assert_eq!(reported.borrow().len(), 1);
    }
}
