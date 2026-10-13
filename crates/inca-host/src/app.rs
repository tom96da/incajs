// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Loads an entry module (resolving its own `import`s against sibling
//! files on disk) and opens a GPUI window on whatever tree it mounts.
//! Production evaluates the entry to completion before the window opens. The
//! host starts it as a [`Loading`] for the development window, and
//! [`crate::dev`] polls it until it settles.
//!
//! One binary serves any app, so it cannot know whether a bundle registers
//! input handlers; it always renders through `EventDispatcher`, which tracks
//! the pointer once a listener is registered or a node can take focus.
//!
//! `--dev` additionally answers messages on stdin and writes them on
//! stdout, via [`crate::dev`] speaking the development wire format.
//! stdout is then the message channel and nothing else may go there, so
//! every diagnostic, and the app's own `console`, go to stderr.
//!
//! Nothing here panics on a failure a user can cause.

use std::cell::{Cell, RefCell};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, Point, SharedString, Size, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions, div, prelude::*, px, size,
};
use gpui_platform::application;

use inca_bridge::bindings::install;
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{
    AttributeValue, EventSink, NodeId, NodeLayout, VirtualNode, render_tree_with_events,
};
use inca_jsenv::{Engine, EngineError, ErrorReporter, PendingModule, console, stderr_reporter};

use crate::config::{
    self, DEFAULT_APP_NAME, effective_config, is_resizable, usable, window_min_size, window_size,
    window_title,
};

use crate::console_output::{color_from_env, to_stderr};
use crate::dev::{ErrorCode, LoadSignal, install_dev, install_load_signal};
use crate::dev::{
    SharedWriter, StdoutWriter, fail_startup, install_panic_hook, reporter_for, serve_dev_protocol,
    stdin_lines,
};
use crate::menu;

/// Reads the window size straight from the app the bundle mounted, so the
/// window fits its content.
///
/// `root` is the empty container the [`Host`] allocates for the bundle to
/// `mount()` against. The mounted app becomes `root`'s first (and only)
/// child, never `root` itself, so `width`/`height` are read from that
/// child's style, not `root`'s.
///
/// A dimension the app's root doesn't declare comes back as `None`.
pub(crate) fn content_window_size(host: &Host, root: NodeId) -> (Option<f32>, Option<f32>) {
    let style = host
        .tree
        .get(root)
        .and_then(|node| node.children().first())
        .and_then(|&content_id| host.tree.get(content_id))
        .map(VirtualNode::style_props);

    // A window dimension in px is always far within f32's
    // precision range. No meaningful truncation risk here.
    #[allow(clippy::cast_possible_truncation)]
    let dimension = |key: &str| {
        style
            .and_then(|props| props.get(key))
            .and_then(AttributeValue::as_number)
            .map(|n| n as f32)
    };

    (dimension("width"), dimension("height"))
}

/// Why a request could not be answered with a result.
#[derive(Debug)]
pub(crate) enum Failure {
    /// Something outside JS: an unreadable file, a window that went away.
    Message(ErrorCode, String),
    /// A value the app threw, which carries its own frames.
    Thrown(EngineError),
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

impl Session {
    /// The empty container the entry mounts against: [`Host::root`]'s own
    /// value, fixed once allocated.
    fn root(&self) -> NodeId {
        self.host.borrow().root
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
    ) -> Result<StartedEngine, EngineError> {
        let host = Rc::new(RefCell::new(Host::default()));
        let signal = Rc::new(LoadSignal::default());

        let module_root = Path::new(entry_path).parent().unwrap_or(Path::new("."));
        let engine = Engine::builder().module_root(module_root).build()?;
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
    pub(crate) fn load(
        entry_path: &str,
        source: &str,
        reporter: ErrorReporter,
    ) -> Result<Self, EngineError> {
        let StartedEngine { host, engine, .. } = Self::start_engine(entry_path, None)?;
        let failed = Rc::clone(&reporter);
        let session = Self::from_parts(host, engine, reporter);
        session
            .engine
            .eval_module(entry_path, source)
            .inspect_err(|_| session.report_failed_start(&failed))?;
        Ok(session)
    }

    /// Reports what the app failed on before its entry failed: the jobs that
    /// threw and the listener errors. The entry's own error is the caller's.
    pub(crate) fn report_failed_start(&self, reporter: &ErrorReporter) {
        for failure in self.engine.take_startup_failures() {
            reporter(&failure);
        }
        self.dispatcher.report_pending();
    }

    /// Starts evaluating `source` as [`Self::load`] does, and returns once
    /// the entry waits on something unsettled, or has completed.
    /// [`Loading::poll`] drives the rest.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the engine fails to start, the bindings
    /// or `console` fail to install, or `source` fails to link, throws, or
    /// rejects before it waits.
    pub(crate) fn begin(
        entry_path: &str,
        source: &str,
        reporter: ErrorReporter,
        writer: Option<&SharedWriter>,
    ) -> Result<Loading, EngineError> {
        let StartedEngine {
            host,
            engine,
            signal,
        } = Self::start_engine(entry_path, writer)?;
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
                loading.session.report_failed_start(&failed);
                Err(err)
            }
            _ => Ok(loading),
        }
    }
}

/// A [`Session`] whose entry is still evaluating.
pub(crate) struct Loading {
    module: PendingModule,
    session: Session,
    signal: Rc<LoadSignal>,
    seen_failures: Cell<u32>,
}

impl Loading {
    /// Runs the jobs the entry waits on. `None` while the entry is pending,
    /// then the entry's result.
    pub(crate) fn poll(&self) -> Option<Result<(), EngineError>> {
        self.session.engine.poll_module(&self.module)
    }

    /// The session the entry is mounting into.
    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    /// Whether the app last reported through `__inca_dev__.setLoadFailed`
    /// that its load failed.
    pub(crate) fn start_failed(&self) -> bool {
        self.signal.failed()
    }

    /// Whether the app reported a failed load since the previous call, even
    /// when a later report cleared it.
    pub(crate) fn reported_failure(&self) -> bool {
        self.seen_failures.replace(self.signal.failures()) != self.signal.failures()
    }

    /// The session, once the entry has settled.
    pub(crate) fn into_session(self) -> Session {
        self.session
    }
}

pub(crate) struct HostedApp {
    pub(crate) session: Session,
    /// The window section of the app's config as of the last `start`/reload,
    /// kept so a later content-driven resize can reuse [`window_size`]'s own
    /// fallback chain.
    pub(crate) window_config: Option<config::WindowConfig>,
    /// Whether [`maybe_auto_resize_to_content`] has already resized the
    /// window's width and its height. Each fires once.
    pub(crate) auto_resized: Cell<(bool, bool)>,
}

impl Render for HostedApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = &self.session;
        session.dispatcher.begin_frame(window, cx);
        let host = session.host.borrow();
        let element = render_tree_with_events(&host.tree, host.root, &session.dispatcher)
            .unwrap_or_else(|| div().into_any_element());
        // The root fills the window; the walk records only its children.
        session.dispatcher.record_layout(
            host.root,
            NodeLayout {
                bounds: Bounds::new(Point::default(), window.viewport_size()),
                border: px(0.0),
                scroll: Point::default(),
            },
        );
        element
    }
}

/// Resizes each window dimension to fit content the first time that
/// dimension becomes usable after the window already opened. An app that
/// mounts after the window opened (an entry still pending when the load
/// timeout passed) has nothing mounted when [`open`] computes the initial
/// size. This catches up once the app finishes mounting.
///
/// A dimension the config fixes explicitly counts as ready right away. A
/// dimension still waiting for content keeps the window's current size.
pub(crate) fn maybe_auto_resize_to_content(app: &HostedApp, window: &mut Window) {
    let (width_done, height_done) = app.auto_resized.get();
    if width_done && height_done {
        return;
    }
    let window_config = app.window_config.as_ref();
    let content = content_window_size(&app.session.host.borrow(), app.session.root());
    let width_ready = !width_done
        && (window_config.is_some_and(|w| usable(w.width).is_some())
            || usable(content.0).is_some());
    let height_ready = !height_done
        && (window_config.is_some_and(|w| usable(w.height).is_some())
            || usable(content.1).is_some());
    if !width_ready && !height_ready {
        return;
    }

    let (width, height) = window_size(window_config, content);
    let current = window.bounds().size;
    window.resize(Size {
        width: if width_ready {
            px(width)
        } else {
            current.width
        },
        height: if height_ready {
            px(height)
        } else {
            current.height
        },
    });
    app.auto_resized
        .set((width_done || width_ready, height_done || height_ready));
}

/// Evaluates the entry to completion, then opens the window, under the
/// config the app's build wrote beside its entry. Production starts the host
/// this way.
///
/// # Errors
///
/// Returns `Failure::Thrown` with the value the entry threw, or
/// `Failure::Message` when the window cannot open or cannot run its first
/// update.
pub(crate) fn start(
    cx: &mut App,
    entry_path: &str,
    source: &str,
    reporter: ErrorReporter,
) -> Result<WindowHandle<HostedApp>, Failure> {
    let session = Session::load(entry_path, source, reporter).map_err(Failure::Thrown)?;
    open(cx, session, entry_path)
}

/// Opens the window on `session`, under the config the app's build wrote
/// beside `entry_path`, then runs the work that mounting queued.
///
/// # Errors
///
/// Returns `Failure::Message` when the window cannot open or cannot run its
/// first update.
pub(crate) fn open(
    cx: &mut App,
    session: Session,
    entry_path: &str,
) -> Result<WindowHandle<HostedApp>, Failure> {
    let app_config = config::read(Path::new(entry_path));

    if let (Some(name), Some(identifier)) = (&app_config.name, &app_config.identifier) {
        // Before any window opens, per `App::set_app_identity`.
        cx.set_app_identity(identifier, name);
    }
    menu::install(cx, app_config.name.as_deref().unwrap_or(DEFAULT_APP_NAME));

    let window_config = app_config.window.as_ref();
    let content = content_window_size(&session.host.borrow(), session.root());
    let (width, height) = window_size(window_config, content);
    let title = window_title(window_config, app_config.name.as_deref());

    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: title.map(SharedString::from),
                    ..Default::default()
                }),
                app_id: app_config.identifier.clone(),
                is_resizable: is_resizable(window_config),
                window_min_size: window_min_size(window_config),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    // The focused node loses `focus` with the window and gets
                    // it back with the window.
                    cx.observe_window_activation(window, |app: &mut HostedApp, window, cx| {
                        let active = window.is_window_active();
                        app.session.dispatcher.window_activated(active, window, cx);
                    })
                    .detach();
                    HostedApp {
                        session,
                        window_config: window_config.cloned(),
                        auto_resized: Cell::new((false, false)),
                    }
                })
            },
        )
        .map_err(|err| Failure::Message(ErrorCode::BundleFailed, err.to_string()))?;
    cx.activate(true);

    // A reactivity scheduler batches its first effects into a microtask, so
    // mounting leaves work queued that nothing else would come back for
    // until the first input event, or never, in an app that takes none.
    window
        .update(cx, |app, window, cx| {
            app.session.dispatcher.drain_jobs_and_refresh(window, cx);
            maybe_auto_resize_to_content(app, window);
        })
        .map_err(|err| Failure::Message(ErrorCode::BundleFailed, err.to_string()))?;

    Ok(window)
}

/// Runs the bundle at `entry_path` in a GPUI window and blocks until the
/// app quits.
///
/// In production the entry evaluates to completion before the window opens.
/// With `dev` set, the host also reads rebuild and HMR messages from stdin
/// and reports errors to the parent process. The entry then runs while the
/// host serves those messages, and the window opens once the entry's
/// top-level `await` settles or two seconds pass. While the app reports a
/// failed load, the window stays closed. Returns `ExitCode::SUCCESS`
/// after a normal quit and `ExitCode::FAILURE` when the file cannot be read.
/// A bundle that fails to start exits the process with status 1.
#[must_use]
pub fn run_bundle(entry_path: &str, dev: bool) -> ExitCode {
    let source = match fs::read_to_string(entry_path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("failed to read {entry_path}: {err}");
            return ExitCode::FAILURE;
        }
    };
    let entry_path = entry_path.to_owned();
    let writer: Option<SharedWriter> = dev.then(|| Rc::new(StdoutWriter::spawn()) as SharedWriter);
    if dev {
        install_panic_hook();
    }

    let closing = writer.clone();
    application().run(move |cx: &mut App| {
        // macOS keeps an app alive with no windows left; this framework's
        // apps are single-window, so closing the window is quitting.
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Some(writer) = writer.clone() {
            close_writer_on_quit(cx, writer);
        }

        let error_reporter = writer.as_ref().map_or_else(stderr_reporter, reporter_for);
        let lines = writer.is_some().then(stdin_lines);
        match (&writer, lines) {
            (Some(writer), Some(lines)) => {
                let started = cx.background_executor().now();
                match Session::begin(&entry_path, &source, error_reporter, Some(writer)) {
                    Ok(first) => serve_dev_protocol(
                        cx,
                        lines,
                        first,
                        started,
                        entry_path.clone(),
                        Rc::clone(writer),
                    ),
                    Err(err) => fail_startup(&Failure::Thrown(err), Some(writer)),
                }
            }
            _ => {
                if let Err(failure) = start(cx, &entry_path, &source, error_reporter) {
                    fail_startup(&failure, None);
                }
            }
        }
    });

    if let Some(writer) = closing {
        writer.close();
    }
    ExitCode::SUCCESS
}

// macOS ends the process inside `quit`, so `run` never returns to close the writer.
fn close_writer_on_quit(cx: &mut App, writer: SharedWriter) {
    cx.on_app_quit(move |_| {
        writer.close();
        async {}
    })
    .detach();
}

/// The settings the host applies to the app whose entry is `entry_path`,
/// read from the `inca.json` beside it, or the message to report when
/// `entry_path` names no file.
fn config_report(entry_path: &str) -> Result<String, String> {
    if !Path::new(entry_path).is_file() {
        return Err(format!("failed to read {entry_path}: not a file"));
    }
    Ok(effective_config(&config::read(Path::new(entry_path))))
}

/// Prints the settings the host applies to the app whose entry is
/// `entry_path`, read from the `inca.json` beside it, and returns after
/// printing them. Returns `ExitCode::FAILURE` when `entry_path` names no
/// file.
#[must_use]
pub fn print_config(entry_path: &str) -> ExitCode {
    match config_report(entry_path) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Where a packaged app's bundle lives relative to `exe_dir`, tried in
/// order: a flat layout (Linux, `bundle.js` beside the executable) then a
/// macOS `.app`'s (`Contents/MacOS/<exe>` next to `Contents/Resources/`).
fn bundle_beside(exe_dir: &Path) -> Option<PathBuf> {
    [
        exe_dir.join("bundle.js"),
        exe_dir.join("../Resources/bundle.js"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
}

/// Finds the `bundle.js` that ships beside the running executable.
///
/// Looks next to the executable, then in `../Resources` (a macOS app
/// bundle). Returns `None` when neither holds a file.
#[must_use]
pub fn bundle_beside_exe() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    bundle_beside(exe.parent()?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
