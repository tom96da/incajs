// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `QuickJS` runtime bootstrap.
//!
//! An [`Engine`] pairs one `QuickJS` runtime with one execution context. Each
//! `Engine` is fully independent: creating a global on one has no effect on
//! any other `Engine`, since they don't share a runtime or a heap.

use std::fmt;
use std::path::PathBuf;

use rquickjs::{Coerced, Context, Ctx, FromJs, Module, Runtime, Value};

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

pub struct Engine {
    // Kept alive for the lifetime of `context`, which internally holds a
    // reference-counted handle back to it; QuickJS ties runtime-wide state
    // (the heap, GC) to this handle rather than to the context.
    _runtime: Runtime,
    context: Context,
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
        Ok(Self {
            _runtime: runtime,
            context,
        })
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
    pub fn eval_module(&self, name: &str, source: &str) -> EngineResult<()> {
        self.context.with(|ctx| {
            Module::declare(ctx.clone(), name, source)
                .and_then(rquickjs::Module::eval)
                .and_then(|(_module, promise)| promise.finish())
                .map_err(|err| EngineError::capture(&ctx, &err))
        })
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
        engine.with(|ctx| while ctx.execute_pending_job() {});

        let seen: i32 = engine.eval("globalThis.seen").unwrap();
        assert_eq!(seen, 42);
    }

    #[test]
    fn no_module_root_leaves_an_import_unresolved() {
        let engine = Engine::new().unwrap();
        let result = engine.eval_module("entry.js", "import './dep.js';");
        assert!(result.is_err());
    }
}
