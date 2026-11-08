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

use std::cell::Cell;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, Point, SharedString, TitlebarOptions, Window, WindowBounds, WindowHandle,
    WindowOptions, div, prelude::*, px, size,
};
use gpui_platform::application;

use inca_gpui::{EventSink, NodeLayout, render_tree_with_events};
use inca_jsenv::{EngineError, ErrorReporter, PendingModule, stderr_reporter};

use crate::config::{
    self, DEFAULT_APP_NAME, effective_config, is_resizable, window_min_size, window_size,
    window_title,
};

use crate::dev::LoadSignal;
use crate::dev::{
    SharedWriter, StdoutWriter, fail_startup, install_panic_hook, reporter_for, serve_dev_protocol,
    stdin_lines,
};
use crate::menu;
use crate::run_log;

mod session;
mod size;

pub(crate) use session::{Failure, Session, clock_of};
use size::content_window_size;
pub(crate) use size::maybe_auto_resize_to_content;

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
    /// The window section of the app's config, read once when the window opens.
    /// A content-driven resize reuses it through [`window_size`]'s fallback chain.
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
        let top = host.tree.document_element();
        let element = top
            .and_then(|id| render_tree_with_events(&host.tree, id, &session.dispatcher))
            .unwrap_or_else(|| div().into_any_element());
        // The element of the document fills the window; the walk records only its children.
        if let Some(id) = top {
            session.dispatcher.record_layout(
                id,
                NodeLayout {
                    bounds: Bounds::new(Point::default(), window.viewport_size()),
                    border: px(0.0),
                    scroll: Point::default(),
                },
            );
        } else {
            session.dispatcher.clear_layout();
        }
        element
    }
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
    let session = Session::load_with(
        entry_path,
        source,
        reporter,
        clock_of(cx.background_executor()),
    )
    .map_err(Failure::Thrown)?;
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
                inactive_frame_interval: None,
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
        .map_err(Failure::bundle)?;
    cx.activate(true);

    window
        .update(cx, |app, window, cx| {
            app.session
                .dispatcher
                .start_timers(window.window_handle(), cx);
            // A reactivity scheduler batches its first effects into a microtask, so
            // mounting leaves work queued that nothing else would come back for
            // until the first input event or timer, in an app that takes none.
            app.session.dispatcher.drain_jobs_and_refresh(window, cx);
            maybe_auto_resize_to_content(app, window);
        })
        .map_err(Failure::bundle)?;

    Ok(window)
}

/// Runs the bundle at `entry_path` in a GPUI window and blocks until the
/// app quits. Returns `ExitCode::SUCCESS` after a normal quit.
///
/// In production the entry evaluates to completion before the window opens.
/// With `dev` set, the host also reads rebuild and HMR messages from stdin
/// and reports errors to the parent process. The entry then runs while the
/// host serves those messages, and the window opens once the entry's
/// top-level `await` settles or two seconds pass. While the app reports a
/// failed load, the window stays closed.
///
/// A bundle that fails to start and an entry that cannot be read both exit
/// the process with status 1. In production the failure prints to stderr and
/// is written to the log file of the run; with `dev` set the failure is
/// reported as a throwing first bundle. On Linux, a `DISPLAY` or
/// `WAYLAND_DISPLAY` that names a server the host cannot reach exits the
/// process with status 1 and one line on stderr that names the variable.
///
/// An error the running app raises and a panic of a packaged app are written to
/// the log file of the run in the log directory of the app. With `dev` set, a
/// panic goes to the parent process as a `-32603` error.
#[must_use]
pub fn run_bundle(entry_path: &str, dev: bool) -> ExitCode {
    let writer: Option<SharedWriter> = if dev {
        let stdout_writer = StdoutWriter::spawn();
        let sender = stdout_writer.sender_clone();
        install_panic_hook(sender, stdout_writer.thread_id());
        Some(Rc::new(stdout_writer) as SharedWriter)
    } else {
        let identifier = config::try_read(Path::new(entry_path))
            .ok()
            .and_then(|config| config.identifier);
        run_log::install(
            run_log::log_dir(identifier.as_deref()),
            run_log::start_secs(),
        );
        None
    };
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    crate::display::exit_on_unreachable_display();
    let source = match fs::read_to_string(entry_path) {
        Ok(source) => source,
        Err(err) => fail_startup(
            &Failure::bundle(format!("failed to read {entry_path}: {err}")),
            writer.as_ref(),
        ),
    };
    let entry_path = entry_path.to_owned();

    let closing = writer.clone();
    application().run(move |cx: &mut App| {
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        crate::display::launched();
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

        let error_reporter = writer.as_ref().map_or_else(
            || {
                let base = stderr_reporter();
                Rc::new(move |err: &EngineError| {
                    run_log::error("error", &err.to_string());
                    base(err);
                }) as ErrorReporter
            },
            reporter_for,
        );
        if let Some(writer) = &writer {
            let lines = stdin_lines();
            let started = cx.background_executor().now();
            match Session::begin_with(
                &entry_path,
                &source,
                error_reporter,
                Some(writer),
                clock_of(cx.background_executor()),
            ) {
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
        } else if let Err(failure) = start(cx, &entry_path, &source, error_reporter) {
            fail_startup(&failure, None);
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
/// `entry_path` names no file or that `inca.json` cannot be read or parsed.
fn config_report(entry_path: &str) -> Result<String, String> {
    if !Path::new(entry_path).is_file() {
        return Err(format!("failed to read {entry_path}: not a file"));
    }
    let config = config::try_read(Path::new(entry_path))
        .map_err(|err| format!("failed to read {}", err.report()))?;
    if let Some((path, reason)) = config::unusable(&config) {
        let file = config::config_path(Path::new(entry_path)).unwrap_or_default();
        return Err(format!(
            "failed to read {}: {path}: {reason}",
            file.display()
        ));
    }
    Ok(effective_config(&config))
}

/// Prints the settings the host applies to the app whose entry is
/// `entry_path`, read from the `inca.json` beside it, and returns after
/// printing them. Returns `ExitCode::FAILURE` and prints nothing on stdout
/// when `entry_path` names no file or that `inca.json` cannot be read or
/// parsed, or a value in it that no window can use; the line on stderr then
/// names the file, and for a value of the wrong type, broken JSON or an
/// unusable value the path to it.
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
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use gpui::TestAppContext;

    use crate::tests::{ScratchDir, TEST_ENTRY_PATH};

    const DEFERS_ITS_MOUNT: &str = r"
        globalThis.mounted = false;
        Promise.resolve().then(() => { globalThis.mounted = true; });
    ";

    #[test]
    fn the_report_of_a_missing_entry_names_it() {
        let dir = ScratchDir::new("print-missing");
        let entry = dir.join("bundle.js").to_string_lossy().into_owned();

        assert_eq!(
            config_report(&entry),
            Err(format!("failed to read {entry}: not a file"))
        );
    }

    #[test]
    fn the_report_reads_the_config_beside_the_entry() {
        let dir = ScratchDir::new("print-present");
        let entry = dir.join("bundle.js");
        fs::write(&entry, "").unwrap();
        fs::write(dir.join("inca.json"), r#"{"name":"Demo"}"#).unwrap();

        let report: serde_json::Value =
            serde_json::from_str(&config_report(&entry.to_string_lossy()).unwrap()).unwrap();
        assert_eq!(report["name"], "Demo");
    }

    #[test]
    fn the_report_of_a_config_it_cannot_parse_names_the_field() {
        let dir = ScratchDir::new("print-malformed");
        let entry = dir.join("bundle.js");
        fs::write(&entry, "").unwrap();
        fs::write(dir.join("inca.json"), r#"{"window":{"width":"wide"}}"#).unwrap();

        let message = config_report(&entry.to_string_lossy()).unwrap_err();

        let config = dir.join("inca.json");
        assert!(
            message.starts_with(&format!(
                r#"failed to read {}: ["window","width"]: "#,
                config.display()
            )),
            "{message}"
        );
    }

    #[test]
    fn config_report_refuses_an_unusable_value_with_its_path() {
        let dir = ScratchDir::new("print-unusable");
        let entry = dir.join("bundle.js");
        fs::write(&entry, "").unwrap();
        fs::write(dir.join("inca.json"), r#"{"window":{"width":-100}}"#).unwrap();

        assert_eq!(
            config_report(&entry.to_string_lossy()),
            Err(format!(
                concat!(
                    r#"failed to read {}: ["window","width"]: "#,
                    "invalid value: -100, expected a finite number above zero"
                ),
                dir.join("inca.json").display()
            ))
        );
    }

    #[test]
    fn bundle_beside_finds_a_flat_sibling_bundle() {
        let dir = ScratchDir::new("flat");
        fs::write(dir.join("bundle.js"), "").unwrap();

        assert_eq!(bundle_beside(&dir), Some(dir.join("bundle.js")));
    }

    #[test]
    fn bundle_beside_finds_a_macos_app_bundles_resources() {
        let dir = ScratchDir::new("app-bundle");
        let macos_dir = dir.join("Contents/MacOS");
        let resources_dir = dir.join("Contents/Resources");
        fs::create_dir_all(&macos_dir).unwrap();
        fs::create_dir_all(&resources_dir).unwrap();
        fs::write(resources_dir.join("bundle.js"), "").unwrap();

        assert_eq!(
            bundle_beside(&macos_dir),
            Some(macos_dir.join("../Resources/bundle.js"))
        );
    }

    #[test]
    fn bundle_beside_is_none_when_nothing_is_there() {
        let dir = ScratchDir::new("empty");

        assert_eq!(bundle_beside(&dir), None);
    }

    #[test]
    fn a_chunk_importing_the_packaged_entry_back_does_not_evaluate_it_twice() {
        let dir = ScratchDir::new("entry-import-back");
        let macos_dir = dir.join("Contents/MacOS");
        let assets_dir = dir.join("Contents/Resources/assets");
        fs::create_dir_all(&macos_dir).unwrap();
        fs::create_dir_all(&assets_dir).unwrap();
        let entry = dir.join("Contents/Resources/bundle.js");
        fs::write(
            &entry,
            r"
            globalThis.evaluations = (globalThis.evaluations ?? 0) + 1;

            import('./assets/chunk.js');
            ",
        )
        .unwrap();
        fs::write(assets_dir.join("chunk.js"), "import '../bundle.js';").unwrap();

        let path = bundle_beside(&macos_dir).unwrap();
        let session = Session::load(
            &path.to_string_lossy(),
            &fs::read_to_string(&path).unwrap(),
            stderr_reporter(),
        )
        .unwrap();
        assert!(session.engine.run_jobs().is_empty());

        let evaluations: i32 = session.engine.eval("globalThis.evaluations").unwrap();
        assert_eq!(evaluations, 1);
    }

    #[gpui::test]
    fn bringing_the_window_up_runs_what_mounting_only_queued(cx: &mut TestAppContext) {
        cx.update(|cx| start(cx, TEST_ENTRY_PATH, DEFERS_ITS_MOUNT, stderr_reporter()).unwrap());
        cx.run_until_parked();

        let ran: bool = cx.update(|cx| {
            cx.windows()
                .first()
                .and_then(|window| {
                    window
                        .downcast::<HostedApp>()?
                        .read_with(cx, |app, _| {
                            app.session.engine.eval::<bool>("globalThis.mounted;").ok()
                        })
                        .ok()
                        .flatten()
                })
                .unwrap_or(false)
        });

        assert!(ran, "an onMounted-style effect must not wait for an event");
    }

    #[gpui::test]
    fn the_root_has_a_box_only_while_the_document_holds_it(cx: &mut TestAppContext) {
        cx.update(|cx| start(cx, TEST_ENTRY_PATH, "", stderr_reporter()).unwrap());
        cx.run_until_parked();
        let window = cx.update(|cx| cx.windows()[0].downcast::<HostedApp>().unwrap());
        let drawn_box = |cx: &mut TestAppContext, script: &str| -> bool {
            cx.update(|cx| {
                window
                    .read_with(cx, |app, _| app.session.engine.eval::<()>(script).unwrap())
                    .unwrap();
                cx.update_window(window.into(), |_, window, cx| {
                    window.refresh();
                    window.draw(cx).clear(cx);
                })
                .unwrap();
                window
                    .read_with(cx, |app, _| {
                        app.session
                            .engine
                            .eval::<bool>(
                                "__inca_native__.layoutOf(__inca_native__.rootNodeId()) !== null",
                            )
                            .unwrap()
                    })
                    .unwrap()
            })
        };
        assert!(drawn_box(cx, ""));

        let removed = drawn_box(
            cx,
            "__inca_native__.removeChild(__inca_native__.documentNodeId(), __inca_native__.rootNodeId());",
        );
        let appended = drawn_box(
            cx,
            "__inca_native__.appendChild(__inca_native__.documentNodeId(), __inca_native__.rootNodeId());",
        );

        assert_eq!((removed, appended), (false, true));
    }
}
