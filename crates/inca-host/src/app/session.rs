// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One loaded bundle: the engine, the tree its JS built and the dispatcher.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gpui::BackgroundExecutor;

use inca_bridge::install;
use inca_bridge::{EventDispatcher, Host};
use inca_jsenv::{Clock, Engine, EngineError, ErrorReporter, Limits, console};
use inca_node::NodeId;

use crate::app::Loading;
use crate::config;
use crate::console_output::{color_from_env, to_stderr};
use crate::dev::{ErrorCode, LoadSignal, SharedWriter, install_dev, install_load_signal};

/// One mebibyte, in bytes.
const MIB: usize = 1024 * 1024;

/// The limits of the script engine. `memory` is the heap in bytes (1 GiB), `stack` is the native
/// stack in bytes (6 MiB), and a zero `time` sets no limit on how long a call runs.
const ENGINE_LIMITS: Limits = Limits {
    memory: 1024 * MIB,
    stack: 6 * MIB,
    time: Duration::ZERO,
};

/// Why a request could not be answered with a result.
#[derive(Debug)]
pub(crate) enum Failure {
    /// Something outside JS: an unreadable file, a window that went away.
    Message(ErrorCode, String),
    /// A value the app threw, which carries its own frames.
    Thrown(EngineError),
}

impl Failure {
    /// A failure to report as a bundle failure, with the text of `err`.
    pub(crate) fn bundle(err: impl std::fmt::Display) -> Self {
        Self::Message(ErrorCode::BundleFailed, err.to_string())
    }
}

/// An engine ready for an entry: its host, the engine, and what
/// `setLoadFailed` reports.
struct StartedEngine {
    host: Rc<RefCell<Host>>,
    engine: Engine,
    signal: Rc<LoadSignal>,
}

/// One loaded bundle: the engine running its JS, the tree that JS built, and
/// the dispatcher wiring events back. A reload replaces all of it at once,
/// so it travels together and a half-swapped state cannot exist.
///
/// Dropping one takes its whole `QuickJS` runtime with it, so no node,
/// listener or callback survives a reload.
pub(crate) struct Session {
    pub(crate) engine: Rc<Engine>,
    pub(crate) host: Rc<RefCell<Host>>,
    pub(crate) dispatcher: EventDispatcher,
}

/// The clock of `executor`, so a test that moves the executor's clock moves the timers.
pub(crate) fn clock_of(executor: &BackgroundExecutor) -> Clock {
    let executor = executor.clone();
    Rc::new(move || executor.now())
}

impl Session {
    /// The root node of the host's tree.
    pub(super) fn root(&self) -> NodeId {
        self.host.borrow().root()
    }

    /// Starts an engine rooted at `entry_path`'s directory and gives it
    /// everything an entry expects to find: `console`, the native bindings
    /// and, for the development window (`writer.is_some()`), `__inca_dev__`
    /// with its `send`, `setLoadFailed` and `compile` methods. All of it is in
    /// place before any entry code runs, so an entry that logs while evaluating
    /// finds `console`.
    ///
    /// `__inca_dev__` exists only when `writer` is `Some`; see
    /// [`crate::dev::install_dev`] for what it does. The returned `signal`
    /// records what `setLoadFailed` reports, and stays at its defaults in
    /// production.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the engine fails to start, or `console`,
    /// the bindings, or `__inca_dev__` fail to install.
    fn start_engine(
        entry_path: &str,
        writer: Option<&SharedWriter>,
        clock: Clock,
    ) -> Result<StartedEngine, EngineError> {
        let host = Rc::new(RefCell::new(Host::with_clock(clock.clone())));
        let signal = Rc::new(LoadSignal::default());

        let module_root = Path::new(entry_path).parent().unwrap_or(Path::new("."));
        let config = config::try_read(Path::new(entry_path)).unwrap_or_default();
        let engine = Engine::builder()
            .module_root(module_root)
            .stack_paths(config::stack_paths(&config, Path::new(entry_path)))
            .limits(ENGINE_LIMITS)
            .clock(clock)
            .build()?;
        engine.with(|ctx| {
            console::install(&ctx, &to_stderr(), color_from_env())
                .and_then(|()| install(&ctx, &host))
                .and_then(|()| match writer {
                    Some(writer) => install_dev(&ctx, crate::dev::dev_send(writer))
                        .and_then(|()| install_load_signal(&ctx, Rc::clone(&signal))),
                    None => Ok(()),
                })
                .map_err(|err| EngineError::capture(&ctx, &err))
        })?;
        Ok(StartedEngine {
            host,
            engine,
            signal,
        })
    }

    /// Wires event dispatch onto an engine whose entry has been started.
    fn from_parts(host: Rc<RefCell<Host>>, engine: Engine, reporter: ErrorReporter) -> Self {
        let engine = Rc::new(engine);
        let dispatcher =
            EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host)).with_reporter(reporter);
        Self {
            engine,
            host,
            dispatcher,
        }
    }

    #[cfg(test)]
    pub(crate) fn load(
        entry_path: &str,
        source: &str,
        reporter: ErrorReporter,
    ) -> Result<Self, EngineError> {
        Self::load_with(entry_path, source, reporter, inca_jsenv::system_clock())
    }

    /// Evaluates `source` (the entry's own already-read content) into a
    /// fresh engine, then wires up event dispatch. An `import` in `source`
    /// resolves against a sibling file next to `entry_path`.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the engine fails to start, the bindings
    /// or `console` fail to install, or `source` throws while evaluating,
    /// including an unresolved `import` for a sibling file that isn't there,
    /// and a top-level `await` that is still pending once the queued jobs
    /// have run.
    pub(crate) fn load_with(
        entry_path: &str,
        source: &str,
        reporter: ErrorReporter,
        clock: Clock,
    ) -> Result<Self, EngineError> {
        let StartedEngine { host, engine, .. } = Self::start_engine(entry_path, None, clock)?;
        let failed = Rc::clone(&reporter);
        let session = Self::from_parts(host, engine, reporter);
        session
            .engine
            .eval_module(entry_path, source)
            .inspect_err(|err| session.report_failed_start(&failed, err))?;
        Ok(session)
    }

    /// Reports what the app failed on besides its entry: the jobs that threw, the jobs left
    /// behind, the unhandled rejections and the listener errors. `entry` is the caller's to
    /// report, so one failure equal to it is left out.
    pub(crate) fn report_failed_start(&self, reporter: &ErrorReporter, entry: &EngineError) {
        let mut failures = self.engine.run_jobs();
        if let Some(at) = failures.iter().position(|failure| failure == entry) {
            failures.remove(at);
        }
        for failure in &failures {
            reporter(failure);
        }
        self.dispatcher.report_pending();
    }

    #[cfg(test)]
    pub(crate) fn begin(
        entry_path: &str,
        source: &str,
        reporter: ErrorReporter,
        writer: Option<&SharedWriter>,
    ) -> Result<Loading, EngineError> {
        Self::begin_with(
            entry_path,
            source,
            reporter,
            writer,
            inca_jsenv::system_clock(),
        )
    }

    /// Starts evaluating `source` as [`Self::load_with`] does, and returns once
    /// the entry waits on something unsettled, or has completed.
    /// [`Loading::poll`] drives the rest.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the engine fails to start, the bindings
    /// or `console` fail to install, or `source` fails to link, throws, or
    /// rejects before it waits.
    pub(crate) fn begin_with(
        entry_path: &str,
        source: &str,
        reporter: ErrorReporter,
        writer: Option<&SharedWriter>,
        clock: Clock,
    ) -> Result<Loading, EngineError> {
        let StartedEngine {
            host,
            engine,
            signal,
        } = Self::start_engine(entry_path, writer, clock)?;
        let failed = Rc::clone(&reporter);
        let session = Self::from_parts(host, engine, reporter);
        let module = session.engine.start_module(entry_path, source)?;
        let loading = Loading {
            module,
            session,
            seen_failures: Cell::new(0),
            signal,
        };
        match loading.poll() {
            Some(Err(err)) => {
                loading.session.report_failed_start(&failed, &err);
                Err(err)
            }
            _ => Ok(loading),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use gpui::TestAppContext;
    use inca_gpui::EventPayload;
    use inca_jsenv::stderr_reporter;
    use rstest::rstest;

    use crate::app::{HostedApp, close_writer_on_quit, open};
    use crate::dev::{Writer, reporter_for};
    use crate::tests::{CapturingWriter, TEST_ENTRY_PATH, load};

    #[test]
    fn inca_dev_exists_only_with_a_writer() {
        let session = load("globalThis.hasDev = typeof __inca_dev__ !== 'undefined';").unwrap();

        assert!(!session.engine.eval::<bool>("globalThis.hasDev;").unwrap());
    }

    #[test]
    fn inca_dev_send_writes_a_notification_to_the_writer() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        Session::begin(
            TEST_ENTRY_PATH,
            r"__inca_dev__.send('testEvent', JSON.stringify({ a: 1 }));",
            stderr_reporter(),
            Some(&writer),
        )
        .unwrap();

        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains(r#""method":"testEvent""#));
        assert!(sent[0].contains(r#""a":1"#));
    }

    fn capturing_writer() -> SharedWriter {
        Rc::new(CapturingWriter::default())
    }

    fn begin(source: &str) -> Result<Loading, EngineError> {
        Session::begin(
            TEST_ENTRY_PATH,
            source,
            stderr_reporter(),
            Some(&capturing_writer()),
        )
    }

    #[test]
    fn a_pending_entry_begins_as_a_loading_that_settles_when_resolved() {
        let loading =
            begin("await new Promise((r) => { globalThis.release = r; }); globalThis.done = true;")
                .unwrap();

        assert_eq!(loading.poll(), None);
        loading.session().engine.eval::<()>("release();").unwrap();
        assert_eq!(loading.poll(), Some(Ok(())));
        let session = loading.into_session();
        assert!(session.engine.eval::<bool>("globalThis.done;").unwrap());
    }

    #[test]
    fn a_load_failed_report_in_the_synchronous_part_shows_right_after_begin() {
        let loading =
            begin("__inca_dev__.setLoadFailed(true); await new Promise(() => {});").unwrap();

        assert!(loading.start_failed());
        loading
            .session()
            .engine
            .eval::<()>("__inca_dev__.setLoadFailed(false);")
            .unwrap();
        assert!(!loading.start_failed());
    }

    #[test]
    fn reported_failure_is_true_once_per_report_even_when_cleared() {
        let loading = begin("await new Promise(() => {});").unwrap();
        let eval = |script| loading.session().engine.eval::<()>(script).unwrap();

        assert!(!loading.reported_failure());
        eval("__inca_dev__.setLoadFailed(true); __inca_dev__.setLoadFailed(false);");
        assert!(loading.reported_failure());
        assert!(!loading.reported_failure());
        eval("__inca_dev__.setLoadFailed(true);");
        assert!(loading.reported_failure());
    }

    #[rstest]
    #[case::throws_synchronously("throw new Error('sync');")]
    #[case::throws_after_an_await("await Promise.resolve(); throw new Error('async');")]
    fn begin_returns_the_error_of_an_entry_that_throws_or_rejects_before_it_waits(
        #[case] source: &str,
    ) {
        let err = begin(source).err().unwrap();
        assert!(err.message().starts_with("Error: "), "{source}");
        assert!(err.stack().is_some(), "{source}");
    }

    #[test]
    fn load_reports_a_pending_entry_as_an_error() {
        let err = load("await new Promise(() => {});").err().unwrap();

        assert!(!err.message().is_empty());
    }

    #[test]
    fn a_bundle_can_log_while_it_evaluates() {
        assert!(
            load("console.log('mounting', { ready: true });").is_ok(),
            "console has to exist before the bundle runs, not after"
        );
    }

    #[test]
    fn a_bundle_that_throws_yields_nothing() {
        let err = load("throw new Error('boom');").err().unwrap();

        assert_eq!(err.message(), "Error: boom");
        assert!(
            err.stack().is_some(),
            "which the client reports as data.stack"
        );
    }

    #[test]
    fn a_thrown_error_names_the_bundle_from_the_app_root() {
        let dir = crate::tests::ScratchDir::new("root-relative");
        std::fs::create_dir_all(dir.join("app/dist")).unwrap();
        std::fs::write(dir.join("app/dist/inca.json"), r#"{"root":".."}"#).unwrap();
        let entry = dir.join("app/dist/bundle.js");
        let entry_path = entry.to_str().unwrap();

        let err = Session::load(entry_path, "throw new Error('boom');", stderr_reporter())
            .err()
            .unwrap();

        let stack = err.stack().unwrap();
        assert!(stack.contains("(dist/bundle.js:"), "{stack}");
    }

    struct ClosingWriter(Rc<Cell<bool>>);

    impl Writer for ClosingWriter {
        fn write_line(&self, _line: String) {}

        fn close(&self) {
            self.0.set(true);
        }
    }

    #[gpui::test]
    fn quitting_closes_the_writer(cx: &mut TestAppContext) {
        let closed = Rc::new(Cell::new(false));
        cx.update(|cx| {
            close_writer_on_quit(cx, Rc::new(ClosingWriter(Rc::clone(&closed))));
            cx.shutdown();
        });
        cx.run_until_parked();

        assert!(closed.get());
    }

    // Attaches one node under the root and registers a listener on it, so a
    // reload has both tree and registry state that could leak.
    const MOUNTING_BUNDLE: &str = r"
        const node = __inca_native__.createNode('div');
        __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
        __inca_native__.addEventListener(node, 'click', () => {});
    ";

    #[test]
    fn a_reload_leaves_no_stale_nodes_listeners_or_callbacks() {
        let first = load(MOUNTING_BUNDLE).unwrap();
        first
            .engine
            .eval::<()>("__inca_native__.addEventListener('$window', 'click', () => {});")
            .unwrap();
        let first_child = first
            .host
            .borrow()
            .tree
            .get(first.root())
            .unwrap()
            .children()[0];

        let second = load(MOUNTING_BUNDLE).unwrap();
        drop(first);

        let host = second.host.borrow();
        let children = host.tree.get(second.root()).unwrap().children();
        assert_eq!(children.len(), 1, "the reloaded tree must not accumulate");
        assert_eq!(
            children[0], first_child,
            "ids restart, so the reloaded tree replaces the first"
        );
        let ids: Vec<u32> = host
            .listeners
            .listeners_for(inca_bridge::EventTargetId::Node(children[0]), "click")
            .iter()
            .map(|l| l.callback)
            .collect();
        assert_eq!(ids, &[1]);
        let stored = |id| {
            second
                .engine
                .with(|ctx| inca_jsenv::callbacks::get(&ctx, id).unwrap().is_some())
        };
        assert!(stored(1));
        assert!(
            !stored(2),
            "the fresh engine must not carry the previous callback table"
        );
    }

    const THROWING_LISTENER_BUNDLE: &str = r"
        const node = __inca_native__.createNode('div');
        __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
        __inca_native__.addEventListener(node, 'click', () => {
            throw new Error('boom');
        });
    ";

    #[gpui::test]
    fn a_throwing_listener_is_reported_exactly_once_to_the_writer(cx: &mut TestAppContext) {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;
        let reporter = reporter_for(&writer);

        let window = cx.update(|cx| {
            let session = Session::begin(
                TEST_ENTRY_PATH,
                THROWING_LISTENER_BUNDLE,
                reporter,
                Some(&writer),
            )
            .unwrap()
            .into_session();
            open(cx, session, TEST_ENTRY_PATH).unwrap()
        });
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app: &mut HostedApp, window, cx| {
                    let node = app
                        .session
                        .host
                        .borrow()
                        .tree
                        .get(app.session.root())
                        .unwrap()
                        .children()[0];
                    app.session
                        .dispatcher
                        .fire(node, "click", &EventPayload::None, window, cx);
                })
                .unwrap();
        });
        cx.run_until_parked();

        let sent = capturing.0.borrow();
        assert_eq!(
            sent.len(),
            1,
            "a throwing listener must be reported exactly once, not once per drain"
        );
        assert!(sent[0].contains(r#""method":"appError""#));
        assert!(sent[0].contains("boom"));
    }

    #[test]
    fn a_listener_error_is_reported_when_the_entry_throws_after_dispatching_it() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let err = Session::begin(
            TEST_ENTRY_PATH,
            &format!(
                "{THROWING_LISTENER_BUNDLE}\n__inca_native__.click(node);\nthrow new Error('entry');"
            ),
            reporter_for(&writer),
            Some(&writer),
        )
        .err()
        .unwrap();

        assert_eq!(err.message(), "Error: entry");
        let sent = capturing.0.borrow();
        assert_eq!(
            sent.len(),
            1,
            "the listener error is reported before the session is dropped"
        );
        assert!(sent[0].contains(r#""method":"appError""#));
        assert!(sent[0].contains("boom"));
    }

    #[test]
    fn a_listener_error_is_reported_when_load_throws_after_dispatching_it() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let err = Session::load(
            TEST_ENTRY_PATH,
            &format!(
                "{THROWING_LISTENER_BUNDLE}\n__inca_native__.click(node);\nthrow new Error('entry');"
            ),
            reporter_for(&writer),
        )
        .err()
        .unwrap();

        assert_eq!(err.message(), "Error: entry");
        let sent = capturing.0.borrow();
        assert_eq!(
            sent.len(),
            1,
            "the listener error is reported before the session is dropped"
        );
        assert!(sent[0].contains(r#""method":"appError""#));
        assert!(sent[0].contains("boom"));
    }

    #[test]
    fn a_job_error_is_reported_once_when_begin_rejects_after_it() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let err = Session::begin(
            TEST_ENTRY_PATH,
            r"
            queueMicrotask(() => { throw new Error('side'); });
            await Promise.resolve(); throw new Error('entry');
            ",
            reporter_for(&writer),
            Some(&writer),
        )
        .err()
        .unwrap();

        assert_eq!(err.message(), "Error: entry");
        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("side"));
    }

    #[test]
    fn a_job_error_is_reported_once_when_load_rejects_after_it() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let err = Session::load(
            TEST_ENTRY_PATH,
            r"
            queueMicrotask(() => { throw new Error('side'); });
            await Promise.resolve(); throw new Error('entry');
            ",
            reporter_for(&writer),
        )
        .err()
        .unwrap();

        assert_eq!(err.message(), "Error: entry");
        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("side"));
    }

    #[gpui::test]
    fn a_listener_that_throws_during_load_is_reported_when_the_window_opens(
        cx: &mut TestAppContext,
    ) {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;
        let reporter = reporter_for(&writer);
        let source = format!("{THROWING_LISTENER_BUNDLE}\n__inca_native__.click(node);");

        cx.update(|cx| {
            let session = Session::load(TEST_ENTRY_PATH, &source, reporter).unwrap();
            assert!(
                capturing.0.borrow().is_empty(),
                "nothing is reported before open"
            );
            open(cx, session, TEST_ENTRY_PATH).unwrap();
        });
        cx.run_until_parked();

        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains(r#""method":"appError""#));
        assert!(sent[0].contains("boom"));
    }

    /// Recurses until the stack limit throws, and records how deep it got.
    const RECURSES_UNTIL_IT_THROWS: &str = r"
        globalThis.depth = 0;
        function recurse() {
            globalThis.depth += 1;
            recurse();
        }
        try {
            recurse();
        } catch {}
    ";

    #[test]
    fn the_host_engine_runs_a_script_on_a_deeper_stack_than_the_engine_default() {
        // A test thread has 2 MiB, less than the stack limit of the engine.
        let (baseline, depth) = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let plain = Engine::builder().build().unwrap();
                plain.eval::<()>(RECURSES_UNTIL_IT_THROWS).unwrap();
                let session = load(RECURSES_UNTIL_IT_THROWS).unwrap();
                (
                    plain.eval::<u32>("globalThis.depth;").unwrap(),
                    session.engine.eval::<u32>("globalThis.depth;").unwrap(),
                )
            })
            .unwrap()
            .join()
            .unwrap();

        assert!(
            depth > 3 * baseline,
            "session depth {depth}, default depth {baseline}"
        );
    }

    #[test]
    fn an_unhandled_rejection_is_reported_when_begin_rejects() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let err = Session::begin(
            TEST_ENTRY_PATH,
            r"
            Promise.reject(new Error('lost'));
            throw new Error('entry');
            ",
            reporter_for(&writer),
            Some(&writer),
        )
        .err()
        .unwrap();

        assert_eq!(err.message(), "Error: entry");
        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("lost"));
    }

    #[test]
    fn a_job_behind_the_throw_is_reported_when_begin_rejects() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let err = Session::begin(
            TEST_ENTRY_PATH,
            r"
            queueMicrotask(() => {
                throw new Error('behind');
            });
            throw new Error('entry');
            ",
            reporter_for(&writer),
            Some(&writer),
        )
        .err()
        .unwrap();

        assert_eq!(err.message(), "Error: entry");
        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("behind"));
    }

    #[test]
    fn a_job_behind_the_throw_is_reported_when_load_rejects() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let err = Session::load(
            TEST_ENTRY_PATH,
            r"
            queueMicrotask(() => {
                throw new Error('behind');
            });
            throw new Error('entry');
            ",
            reporter_for(&writer),
        )
        .err()
        .unwrap();

        assert_eq!(err.message(), "Error: entry");
        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains("behind"));
    }
}
