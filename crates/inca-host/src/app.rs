// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Loads an entry module — resolving its own `import`s against sibling
//! files on disk — and opens a GPUI window on whatever tree it mounts.
//!
//! One binary serves any app, so it cannot know whether a bundle registers
//! input handlers; it always renders through `EventDispatcher`, which wires
//! only the nodes something listens to.
//!
//! `--dev` additionally answers messages on stdin and writes them on
//! stdout, via [`crate::dev`] speaking [`crate::protocol`]'s wire format.
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
    App, Bounds, Context, Pixels, SharedString, Size, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions, div, prelude::*, px, size,
};
use gpui_platform::application;

use inca_bridge::bindings::install;
use inca_bridge::{ErrorReporter, EventDispatcher, Host, install_dev, stderr_reporter};
use inca_gpui::{AttributeValue, NodeId, VirtualNode, render_tree_with_events};
use inca_jsenv::{Engine, EngineError, console};

use crate::config;

use crate::dev::{
    Failure, SharedWriter, StdoutWriter, install_panic_hook, report_startup_failure, reporter_for,
    send,
};
use crate::menu;
use crate::protocol::{ErrorCode, Outgoing};

/// What an app is called when nothing named it.
const DEFAULT_APP_NAME: &str = "Inca";

/// Window size to fall back to when neither the app's config nor its root
/// element gives one.
const DEFAULT_WINDOW_SIZE: (f32, f32) = (800.0, 600.0);

/// Reads the window size straight from the app the bundle mounted, so the
/// window fits its content.
///
/// `root` is the empty container the [`Host`] allocates for the bundle to
/// `mount()` against — the mounted app becomes `root`'s first (and only)
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

    let dimension = |key: &str| {
        style
            .and_then(|props| props.get(key))
            .and_then(|value| match value {
                // A window dimension in px is always far within f32's
                // precision range — no meaningful truncation risk here.
                #[allow(clippy::cast_possible_truncation)]
                AttributeValue::Number(n) => Some(*n as f32),
                _ => None,
            })
    };

    (dimension("width"), dimension("height"))
}

/// A window dimension that is finite and above zero. Anything else reads
/// as absent.
pub(crate) fn usable(dimension: Option<f32>) -> Option<f32> {
    dimension.filter(|value| value.is_finite() && *value > 0.0)
}

/// The smallest size the window can be resized to, where the app's config
/// gives one. A dimension it leaves out is unconstrained.
fn window_min_size(window: Option<&config::WindowConfig>) -> Option<Size<Pixels>> {
    let min_width = usable(window.and_then(|w| w.min_width));
    let min_height = usable(window.and_then(|w| w.min_height));
    if min_width.is_none() && min_height.is_none() {
        return None;
    }
    Some(size(
        px(min_width.unwrap_or(0.0)),
        px(min_height.unwrap_or(0.0)),
    ))
}

/// The window's title: what the app's config asks for, then the app's own
/// name. A title of nothing but spaces counts as none.
fn window_title(window: Option<&config::WindowConfig>, name: Option<&str>) -> Option<String> {
    window
        .and_then(|window| window.title.clone())
        .filter(|title| !title.trim().is_empty())
        .or_else(|| name.map(ToOwned::to_owned))
}

/// Whether the app's config lets a user resize its window by hand.
/// Unconfigured reads as fixed-size.
pub(crate) fn is_resizable(window: Option<&config::WindowConfig>) -> bool {
    window.and_then(|w| w.resizable).unwrap_or(false)
}

/// The size to open the window at: what the app's config asks for, then
/// what its root element declares, then [`DEFAULT_WINDOW_SIZE`]. A window
/// never opens below the minimum it declared.
pub(crate) fn window_size(
    window: Option<&config::WindowConfig>,
    content: (Option<f32>, Option<f32>),
) -> (f32, f32) {
    let configured = |pick: fn(&config::WindowConfig) -> Option<f32>| usable(window.and_then(pick));
    let at_least = |pick: fn(&config::WindowConfig) -> Option<f32>| {
        usable(window.and_then(pick)).unwrap_or(0.0)
    };
    (
        configured(|w| w.width)
            .or(usable(content.0))
            .unwrap_or(DEFAULT_WINDOW_SIZE.0)
            .max(at_least(|w| w.min_width)),
        configured(|w| w.height)
            .or(usable(content.1))
            .unwrap_or(DEFAULT_WINDOW_SIZE.1)
            .max(at_least(|w| w.min_height)),
    )
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
    /// The empty container the entry mounts against — [`Host::root`]'s own
    /// value, fixed once allocated.
    fn root(&self) -> NodeId {
        self.host.borrow().root
    }

    /// Starts an engine rooted at `entry_path`'s directory and gives it
    /// everything an entry expects to find — `console`, the native
    /// bindings, and, in dev (`writer.is_some()`), `__inca_dev__` — before
    /// any entry code runs, so one that logs while evaluating is heard
    /// rather than met with a `ReferenceError`.
    ///
    /// `__inca_dev__` doesn't exist at all when `writer` is `None`
    /// (production) — see [`inca_bridge::install_dev`] for what it does.
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the engine fails to start, or `console`,
    /// the bindings, or `__inca_dev__` fail to install.
    fn start_engine(
        entry_path: &str,
        writer: Option<&SharedWriter>,
    ) -> Result<(Rc<RefCell<Host>>, Engine), EngineError> {
        let host = Rc::new(RefCell::new(Host::default()));

        let module_root = Path::new(entry_path).parent().unwrap_or(Path::new("."));
        let engine = Engine::builder().module_root(module_root).build()?;
        engine.with(|ctx| {
            console::install(&ctx, &console::to_stderr(), console::color_from_env())
                .and_then(|()| install(&ctx, &host))
                .and_then(|()| match writer {
                    Some(writer) => install_dev(&ctx, crate::dev::dev_send(writer)),
                    None => Ok(()),
                })
                .map_err(|err| EngineError::capture(&ctx, &err))
        })?;
        Ok((host, engine))
    }

    /// Evaluates `source` — the entry's own already-read content — into a
    /// fresh engine, then wires up event dispatch. An `import` in `source`
    /// resolves against a sibling file next to `entry_path`. `writer` is
    /// `Some` only in dev — see [`Self::start_engine`].
    ///
    /// # Errors
    ///
    /// Returns the thrown value if the engine fails to start, the bindings
    /// or `console` fail to install, or `source` throws while evaluating —
    /// including an unresolved `import` for a sibling file that isn't there.
    pub(crate) fn load(
        entry_path: &str,
        source: &str,
        reporter: ErrorReporter,
        writer: Option<&SharedWriter>,
    ) -> Result<Self, EngineError> {
        let (host, engine) = Self::start_engine(entry_path, writer)?;
        engine.eval_module(entry_path, source)?;

        let engine = Rc::new(engine);
        let dispatcher =
            EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host)).with_reporter(reporter);
        Ok(Self {
            engine,
            host,
            dispatcher,
        })
    }
}

pub(crate) struct HostedApp {
    pub(crate) session: Session,
    /// The window section of the app's config as of the last `start`/reload,
    /// kept so a later content-driven resize can reuse [`window_size`]'s own
    /// fallback chain.
    pub(crate) window_config: Option<config::WindowConfig>,
    /// Whether [`maybe_auto_resize_to_content`] has already resized the
    /// window's width and its height — each fires once.
    pub(crate) auto_resized: Cell<(bool, bool)>,
}

impl Render for HostedApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session = &self.session;
        let transitions = session.host.borrow_mut().focus.apply_pending(window, cx);
        for transition in &transitions {
            transition.dispatch(&session.dispatcher, window, cx);
        }
        let host = session.host.borrow();
        render_tree_with_events(&host.tree, host.root, &session.dispatcher)
            .unwrap_or_else(|| div().into_any_element())
    }
}

/// Resizes each window dimension to fit content the first time that
/// dimension becomes usable after the window already opened. The HMR
/// bootstrap entry evaluates fire-and-forget, so nothing is mounted yet when
/// the window's initial size is computed in [`start`] — this catches up once
/// the app finishes mounting.
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

/// Brings up the engine, the tree and the window, under the config the
/// app's build wrote beside its entry. `writer` is `Some` only in dev,
/// installing `__inca_dev__` — see [`Session::start_engine`].
///
/// # Errors
///
/// Returns the window that failed to open, or the value the entry threw.
pub(crate) fn start(
    cx: &mut App,
    entry_path: &str,
    source: &str,
    reporter: ErrorReporter,
    writer: Option<&SharedWriter>,
) -> Result<WindowHandle<HostedApp>, Failure> {
    let session = Session::load(entry_path, source, reporter, writer).map_err(Failure::Thrown)?;
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
            |_, cx| {
                cx.new(|_| HostedApp {
                    session,
                    window_config: window_config.cloned(),
                    auto_resized: Cell::new((false, false)),
                })
            },
        )
        .map_err(|err| Failure::Message(ErrorCode::BundleFailed, err.to_string()))?;
    cx.activate(true);

    // A reactivity scheduler batches its first effects into a microtask, so
    // mounting leaves work queued that nothing else would come back for
    // until the first input event — or never, in an app that takes none.
    window
        .update(cx, |app, window, _| {
            app.session.dispatcher.drain_jobs_and_refresh(window);
            maybe_auto_resize_to_content(app, window);
        })
        .map_err(|err| Failure::Message(ErrorCode::BundleFailed, err.to_string()))?;

    Ok(window)
}

/// Runs the bundle at `entry_path` in a GPUI window and blocks until the
/// app quits.
///
/// With `dev` set, the host also reads rebuild and HMR messages from stdin
/// and reports errors to the parent process. Returns `ExitCode::SUCCESS`
/// after a normal quit and `ExitCode::FAILURE` when the file cannot be
/// read. A bundle that fails to start exits the process with status 1.
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

        let error_reporter = writer.as_ref().map_or_else(stderr_reporter, reporter_for);
        match start(cx, &entry_path, &source, error_reporter, writer.as_ref()) {
            Ok(window) => {
                if let Some(writer) = &writer {
                    send(&**writer, &Outgoing::ready());
                    crate::dev::serve_dev_protocol(
                        cx,
                        window,
                        entry_path.clone(),
                        Rc::clone(writer),
                    );
                }
            }
            Err(failure) => {
                report_startup_failure(&failure, writer.as_ref());
                if let Some(writer) = &writer {
                    writer.close();
                }
                // No window ever opened, so the event loop has nothing to
                // quit from.
                std::process::exit(1);
            }
        }
    });

    if let Some(writer) = closing {
        writer.close();
    }
    ExitCode::SUCCESS
}

/// The settings the host applies for `config`, as pretty-printed JSON. A
/// `null` marks a value the config leaves to the host's own choice: the
/// app's root element sizes the window, and an unset limit stays open.
pub(crate) fn effective_config(config: &config::AppConfig) -> String {
    let window = config.window.as_ref();
    let effective = serde_json::json!({
        "name": config.name.as_deref().unwrap_or(DEFAULT_APP_NAME),
        "identifier": config.identifier,
        "window": {
            "title": window_title(window, config.name.as_deref()),
            "width": usable(window.and_then(|w| w.width)),
            "height": usable(window.and_then(|w| w.height)),
            "resizable": is_resizable(window),
            "minWidth": usable(window.and_then(|w| w.min_width)),
            "minHeight": usable(window.and_then(|w| w.min_height)),
        },
    });
    format!("{effective:#}")
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
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use inca_gpui::EventPayload;

    use crate::dev::Writer;

    /// The entry path these tests evaluate `source` under. Never read from
    /// disk in a test that imports nothing else, since [`Session::load`]
    /// only reads a sibling file when a `source` actually imports one.
    const TEST_ENTRY_PATH: &str = "/test/entry.js";

    fn load(source: &str) -> Result<Session, EngineError> {
        Session::load(TEST_ENTRY_PATH, source, stderr_reporter(), None)
    }

    /// Captures every line written to it instead of touching real stdout, so
    /// a test can read back what was sent.
    #[derive(Default)]
    struct CapturingWriter(RefCell<Vec<String>>);

    impl Writer for CapturingWriter {
        fn write_line(&self, line: String) {
            self.0.borrow_mut().push(line);
        }
    }

    #[test]
    fn inca_dev_does_not_exist_outside_dev() {
        let session = load("globalThis.hasDev = typeof __inca_dev__ !== 'undefined';").unwrap();

        assert!(!session.engine.eval::<bool>("globalThis.hasDev;").unwrap());
    }

    #[test]
    fn inca_dev_send_writes_a_notification_to_the_writer() {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        Session::load(
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

    const DEFERS_ITS_MOUNT: &str = r"
        globalThis.mounted = false;
        Promise.resolve().then(() => { globalThis.mounted = true; });
    ";

    #[test]
    fn evaluating_a_bundle_leaves_a_queued_microtask_pending() {
        let session = load(DEFERS_ITS_MOUNT).unwrap();

        assert!(
            !session.engine.eval::<bool>("globalThis.mounted;").unwrap(),
            "which is what start has to drain once the window is up"
        );
    }

    #[gpui::test]
    fn bringing_the_window_up_runs_what_mounting_only_queued(cx: &mut TestAppContext) {
        cx.update(|cx| {
            start(
                cx,
                TEST_ENTRY_PATH,
                DEFERS_ITS_MOUNT,
                stderr_reporter(),
                None,
            )
            .unwrap()
        });
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

    // Attaches one node under the root and registers a listener on it, so a
    // reload has both tree and registry state that could leak.
    const MOUNTING_BUNDLE: &str = r"
        const node = __inca_native__.createNode('div');
        __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
        __inca_native__.addEventListener(node, 'click', 0);
    ";

    #[test]
    fn a_reload_leaves_no_stale_nodes_listeners_or_callbacks() {
        let first = load(MOUNTING_BUNDLE).unwrap();
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
            "ids restart, so the tree is new rather than appended to"
        );
        assert_eq!(host.listeners.callbacks_for(children[0], "click"), &[0]);
        assert!(
            !second
                .engine
                .eval::<bool>("typeof globalThis.__inca_callbacks__ !== 'undefined';")
                .unwrap(),
            "the fresh engine must not carry the previous callback registry"
        );
    }

    const THROWING_LISTENER_BUNDLE: &str = r"
        const node = __inca_native__.createNode('div');
        __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
        __inca_native__.addEventListener(node, 'click', 0);
        globalThis.__inca_callbacks__ = {
            0: () => { throw new Error('boom'); },
        };
    ";

    #[gpui::test]
    fn a_throwing_listener_is_reported_exactly_once_to_the_writer(cx: &mut TestAppContext) {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;
        let reporter = reporter_for(&writer);

        let window = cx.update(|cx| {
            start(
                cx,
                TEST_ENTRY_PATH,
                THROWING_LISTENER_BUNDLE,
                reporter,
                Some(&writer),
            )
            .unwrap()
        });
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, cx| {
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
                        .dispatch(node, "click", &EventPayload::None, window, cx);
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

    /// Opens a window directly (bypassing [`start`]'s disk-backed config
    /// read) so a test can hand it a chosen `window_config` and starting
    /// size without a real `inca.json` on disk.
    fn open_hosted(
        cx: &mut TestAppContext,
        window_config: Option<config::WindowConfig>,
        initial_size: (f32, f32),
    ) -> WindowHandle<HostedApp> {
        let session = load("").unwrap();
        cx.update(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(initial_size.0), px(initial_size.1)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| HostedApp {
                        session,
                        window_config,
                        auto_resized: Cell::new((false, false)),
                    })
                },
            )
            .unwrap()
        })
    }

    /// Mounts a `div` under the session's root with a declared
    /// `width`/`height`, replacing any prior mount — simulates content
    /// becoming available after the window already opened.
    fn mount_content(app: &HostedApp, width: f32, height: f32) {
        let mut host = app.session.host.borrow_mut();
        let root = host.root;
        let existing = host.tree.get(root).unwrap().children().first().copied();
        let content = if let Some(id) = existing {
            id
        } else {
            let id = host.tree.create_node("div").unwrap();
            host.tree.append_child(root, id).unwrap();
            id
        };
        host.tree
            .set_style(content, "width", f64::from(width))
            .unwrap();
        host.tree
            .set_style(content, "height", f64::from(height))
            .unwrap();
    }

    fn bounds_size(cx: &mut TestAppContext, window: WindowHandle<HostedApp>) -> (f32, f32) {
        cx.update(|cx| {
            window
                .update(cx, |_, window, _| {
                    let bounds = window.bounds();
                    (f32::from(bounds.size.width), f32::from(bounds.size.height))
                })
                .unwrap()
        })
    }

    #[gpui::test]
    fn auto_resize_leaves_the_window_alone_until_content_becomes_usable(cx: &mut TestAppContext) {
        let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
        cx.run_until_parked();
        assert_eq!(bounds_size(cx, window), DEFAULT_WINDOW_SIZE);

        // Nothing mounted yet: the check runs (as it would on every dev
        // relay) but finds no usable content, so the window stays put.
        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(bounds_size(cx, window), DEFAULT_WINDOW_SIZE);

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_content(app, 300.0, 150.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(bounds_size(cx, window), (300.0, 150.0));
    }

    #[gpui::test]
    fn auto_resize_keeps_an_explicit_dimension_but_still_picks_up_the_other_from_content(
        cx: &mut TestAppContext,
    ) {
        let window_config = config::WindowConfig {
            width: Some(500.0),
            ..config::WindowConfig::default()
        };
        let window = open_hosted(cx, Some(window_config), (500.0, DEFAULT_WINDOW_SIZE.1));
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_content(app, 300.0, 150.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });

        assert_eq!(
            bounds_size(cx, window),
            (500.0, 150.0),
            "the configured width must win, but the never-configured height still \
             comes from content — matching window_size's own per-dimension precedence"
        );
    }

    #[gpui::test]
    fn start_auto_resizes_content_that_mounts_only_after_it_returns(cx: &mut TestAppContext) {
        const MOUNTS_LATER: &str = r"
            Promise.resolve().then(() => {
                const node = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
                __inca_native__.setStyle(node, 'width', 300);
                __inca_native__.setStyle(node, 'height', 150);
            });
        ";
        let window = cx.update(|cx| {
            start(cx, TEST_ENTRY_PATH, MOUNTS_LATER, stderr_reporter(), None).unwrap()
        });
        cx.run_until_parked();

        assert_eq!(
            bounds_size(cx, window),
            (300.0, 150.0),
            "start must give its own initial drain a chance to auto-resize, \
             not just a later reload or relay"
        );
    }

    #[gpui::test]
    fn auto_resize_never_fires_once_both_dimensions_are_configured(cx: &mut TestAppContext) {
        let window_config = config::WindowConfig {
            width: Some(500.0),
            height: Some(400.0),
            ..config::WindowConfig::default()
        };
        let window = open_hosted(cx, Some(window_config), (500.0, 400.0));
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_content(app, 300.0, 150.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });

        assert_eq!(
            bounds_size(cx, window),
            (500.0, 400.0),
            "content can't change either dimension once config fixes both"
        );
    }

    #[gpui::test]
    fn auto_resize_only_ever_fires_once(cx: &mut TestAppContext) {
        let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_content(app, 300.0, 150.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(bounds_size(cx, window), (300.0, 150.0));

        // Content changes again later; the latch must stop a second resize.
        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_content(app, 900.0, 700.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(bounds_size(cx, window), (300.0, 150.0));
    }

    /// Mounts an empty `div` under the session's root, then declares each
    /// given dimension on it.
    fn mount_partial(app: &HostedApp, width: Option<f64>, height: Option<f64>) {
        let mut host = app.session.host.borrow_mut();
        let root = host.root;
        let content = host.tree.create_node("div").unwrap();
        host.tree.append_child(root, content).unwrap();
        if let Some(width) = width {
            host.tree.set_style(content, "width", width).unwrap();
        }
        if let Some(height) = height {
            host.tree.set_style(content, "height", height).unwrap();
        }
    }

    #[gpui::test]
    fn auto_resize_fits_the_width_alone_when_the_root_sets_only_a_width(cx: &mut TestAppContext) {
        let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_partial(app, Some(300.0), None);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });

        assert_eq!(bounds_size(cx, window), (300.0, DEFAULT_WINDOW_SIZE.1));
    }

    #[gpui::test]
    fn auto_resize_fits_the_height_alone_when_the_root_sets_only_a_height(cx: &mut TestAppContext) {
        let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_partial(app, None, Some(150.0));
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });

        assert_eq!(bounds_size(cx, window), (DEFAULT_WINDOW_SIZE.0, 150.0));
    }

    #[gpui::test]
    fn auto_resize_picks_up_the_second_dimension_when_it_arrives_later(cx: &mut TestAppContext) {
        let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_partial(app, Some(300.0), None);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(bounds_size(cx, window), (300.0, DEFAULT_WINDOW_SIZE.1));

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_content(app, 900.0, 150.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(
            bounds_size(cx, window),
            (300.0, 150.0),
            "the width fired once already, the height still has its turn"
        );
    }

    #[gpui::test]
    fn auto_resize_leaves_a_dimension_the_user_resized_while_the_other_one_fires(
        cx: &mut TestAppContext,
    ) {
        let window = open_hosted(cx, None, DEFAULT_WINDOW_SIZE);
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_partial(app, Some(300.0), None);
                    maybe_auto_resize_to_content(app, window);
                    window.resize(size(px(350.0), px(450.0)));
                    mount_content(app, 300.0, 150.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });

        assert_eq!(bounds_size(cx, window), (350.0, 150.0));
    }

    #[gpui::test]
    fn auto_resize_keeps_the_current_height_when_only_the_width_fires(cx: &mut TestAppContext) {
        let window = open_hosted(cx, None, (700.0, 450.0));
        cx.run_until_parked();

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_partial(app, Some(300.0), None);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });

        assert_eq!(bounds_size(cx, window), (300.0, 450.0));
    }

    #[gpui::test]
    fn auto_resize_treats_an_unusable_configured_dimension_as_not_ready(cx: &mut TestAppContext) {
        let window_config = config::WindowConfig {
            width: Some(-100.0),
            ..config::WindowConfig::default()
        };
        let window = open_hosted(cx, Some(window_config), DEFAULT_WINDOW_SIZE);
        cx.run_until_parked();

        // A configured width that no window could open at must not count
        // as ready just because it's `Some(_)` — nothing usable exists yet.
        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(bounds_size(cx, window), DEFAULT_WINDOW_SIZE);

        cx.update(|cx| {
            window
                .update(cx, |app, window, _| {
                    mount_content(app, 300.0, 150.0);
                    maybe_auto_resize_to_content(app, window);
                })
                .unwrap();
        });
        assert_eq!(bounds_size(cx, window), (300.0, 150.0));
    }

    #[test]
    fn content_window_size_reads_the_mounted_root_childs_style() {
        let mut host = Host::default();
        let root = host.root;
        let content = host.tree.create_node("div").unwrap();
        host.tree.set_style(content, "width", 300.0).unwrap();
        host.tree.set_style(content, "height", 150.0).unwrap();
        host.tree.append_child(root, content).unwrap();

        assert_eq!(content_window_size(&host, root), (Some(300.0), Some(150.0)));
    }

    #[test]
    fn content_window_size_is_none_when_unset() {
        let mut host = Host::default();
        let root = host.root;
        let content = host.tree.create_node("div").unwrap();
        host.tree.append_child(root, content).unwrap();

        assert_eq!(content_window_size(&host, root), (None, None));
    }

    #[test]
    fn content_window_size_is_none_when_nothing_mounted() {
        let host = Host::default();

        assert_eq!(content_window_size(&host, host.root), (None, None));
    }

    #[test]
    fn a_dimension_no_window_can_open_at_falls_through() {
        let unusable = config::WindowConfig {
            width: Some(-100.0),
            height: Some(0.0),
            min_width: Some(f32::INFINITY),
            min_height: Some(f32::NAN),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&unusable), (Some(300.0), None)),
            (300.0, DEFAULT_WINDOW_SIZE.1)
        );
        assert_eq!(window_min_size(Some(&unusable)), None);
    }

    #[test]
    fn an_unusable_content_dimension_falls_through_too() {
        assert_eq!(
            window_size(None, (Some(f32::NAN), None)),
            DEFAULT_WINDOW_SIZE
        );
    }

    #[test]
    fn window_min_size_carries_both_dimensions_when_both_are_usable() {
        let both = config::WindowConfig {
            min_width: Some(320.0),
            min_height: Some(240.0),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_min_size(Some(&both)),
            Some(size(px(320.0), px(240.0)))
        );
    }

    #[test]
    fn window_min_size_keeps_the_dimension_that_is_usable() {
        let half_usable = config::WindowConfig {
            min_width: Some(320.0),
            min_height: Some(f32::NAN),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_min_size(Some(&half_usable)),
            Some(size(px(320.0), px(0.0)))
        );
    }

    #[test]
    fn a_window_opens_no_smaller_than_the_minimum_it_declared() {
        let below_its_minimum = config::WindowConfig {
            width: Some(200.0),
            min_width: Some(320.0),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&below_its_minimum), (None, None)),
            (320.0, DEFAULT_WINDOW_SIZE.1)
        );
    }

    #[test]
    fn a_minimum_above_the_default_raises_a_window_the_app_did_not_size() {
        let tall = config::WindowConfig {
            min_width: Some(2000.0),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&tall), (None, None)),
            (2000.0, DEFAULT_WINDOW_SIZE.1)
        );
    }

    #[test]
    fn a_title_of_nothing_but_spaces_leaves_the_window_named_after_the_app() {
        for blank in ["", "   "] {
            let window = config::WindowConfig {
                title: Some(blank.to_owned()),
                ..config::WindowConfig::default()
            };

            assert_eq!(
                window_title(Some(&window), Some("Demo")),
                Some("Demo".to_owned()),
                "{blank:?}"
            );
        }
    }

    #[test]
    fn a_title_the_app_set_wins_over_its_name() {
        let window = config::WindowConfig {
            title: Some("Window".to_owned()),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_title(Some(&window), Some("Demo")),
            Some("Window".to_owned())
        );
        assert_eq!(window_title(None, Some("Demo")), Some("Demo".to_owned()));
        assert_eq!(window_title(None, None), None);
    }

    #[test]
    fn window_min_size_is_absent_until_the_config_asks_for_one() {
        assert_eq!(window_min_size(None), None);
        assert_eq!(
            window_min_size(Some(&config::WindowConfig::default())),
            None
        );
    }

    #[test]
    fn window_min_size_leaves_a_dimension_the_config_omits_unconstrained() {
        let width_only = config::WindowConfig {
            min_width: Some(320.0),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_min_size(Some(&width_only)),
            Some(size(px(320.0), px(0.0)))
        );
    }

    #[test]
    fn window_size_prefers_the_config_over_the_mounted_content() {
        let configured = config::WindowConfig {
            width: Some(1024.0),
            height: Some(768.0),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&configured), (Some(300.0), Some(150.0))),
            (1024.0, 768.0)
        );
    }

    #[test]
    fn window_size_falls_through_each_source_per_dimension() {
        let width_only = config::WindowConfig {
            width: Some(1024.0),
            ..config::WindowConfig::default()
        };

        assert_eq!(
            window_size(Some(&width_only), (Some(300.0), Some(150.0))),
            (1024.0, 150.0)
        );
        assert_eq!(
            window_size(Some(&width_only), (None, None)),
            (1024.0, DEFAULT_WINDOW_SIZE.1)
        );
        assert_eq!(window_size(None, (None, None)), DEFAULT_WINDOW_SIZE);
    }

    fn effective(json: &str) -> serde_json::Value {
        let config: config::AppConfig = serde_json::from_str(json).unwrap();
        serde_json::from_str(&effective_config(&config)).unwrap()
    }

    #[test]
    fn an_app_that_declares_nothing_prints_the_defaults() {
        assert_eq!(
            effective("{}"),
            serde_json::json!({
                "name": "Inca",
                "identifier": null,
                "window": {
                    "title": null,
                    "width": null,
                    "height": null,
                    "resizable": false,
                    "minWidth": null,
                    "minHeight": null,
                },
            })
        );
    }

    #[test]
    fn every_declared_setting_is_printed() {
        assert_eq!(
            effective(
                r#"{"name":"Demo","identifier":"org.inca.demo",
                    "window":{"width":1024,"height":768,"title":"Window",
                              "resizable":true,"minWidth":320,"minHeight":240}}"#
            ),
            serde_json::json!({
                "name": "Demo",
                "identifier": "org.inca.demo",
                "window": {
                    "title": "Window",
                    "width": 1024.0,
                    "height": 768.0,
                    "resizable": true,
                    "minWidth": 320.0,
                    "minHeight": 240.0,
                },
            })
        );
    }

    #[test]
    fn the_window_title_falls_back_to_the_name() {
        let printed = effective(r#"{"name":"Demo","window":{"title":"  "}}"#);

        assert_eq!(printed["window"]["title"], "Demo");
        assert_eq!(effective(r#"{"name":"Demo"}"#)["window"]["title"], "Demo");
    }

    #[test]
    fn a_size_no_window_can_open_at_prints_as_null() {
        let printed =
            effective(r#"{"window":{"width":-5,"height":0,"minWidth":-1,"minHeight":0}}"#);

        for key in ["width", "height", "minWidth", "minHeight"] {
            assert!(printed["window"][key].is_null(), "{key}");
        }
    }

    #[test]
    fn the_report_of_a_missing_entry_names_it() {
        let dir = ScratchDir::new("print-missing");
        let entry = dir.0.join("bundle.js").to_string_lossy().into_owned();

        assert_eq!(
            config_report(&entry),
            Err(format!("failed to read {entry}: not a file"))
        );
    }

    #[test]
    fn the_report_reads_the_config_beside_the_entry() {
        let dir = ScratchDir::new("print-present");
        let entry = dir.0.join("bundle.js");
        fs::write(&entry, "").unwrap();
        fs::write(dir.0.join("inca.json"), r#"{"name":"Demo"}"#).unwrap();

        let report: serde_json::Value =
            serde_json::from_str(&config_report(&entry.to_string_lossy()).unwrap()).unwrap();
        assert_eq!(report["name"], "Demo");
    }

    /// A directory under the OS temp root, unique per test invocation, torn
    /// down on drop — this crate has no `tempfile` dependency to reach for.
    struct ScratchDir(PathBuf);

    impl ScratchDir {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "inca-host-test-{name}-{}-{:?}",
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
    fn bundle_beside_finds_a_flat_sibling_bundle() {
        let dir = ScratchDir::new("flat");
        fs::write(dir.0.join("bundle.js"), "").unwrap();

        assert_eq!(bundle_beside(&dir.0), Some(dir.0.join("bundle.js")));
    }

    #[test]
    fn bundle_beside_finds_a_macos_app_bundles_resources() {
        let dir = ScratchDir::new("app-bundle");
        let macos_dir = dir.0.join("Contents/MacOS");
        let resources_dir = dir.0.join("Contents/Resources");
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

        assert_eq!(bundle_beside(&dir.0), None);
    }

    #[test]
    fn a_chunk_importing_the_packaged_entry_back_does_not_evaluate_it_twice() {
        let dir = ScratchDir::new("entry-import-back");
        let macos_dir = dir.0.join("Contents/MacOS");
        let assets_dir = dir.0.join("Contents/Resources/assets");
        fs::create_dir_all(&macos_dir).unwrap();
        fs::create_dir_all(&assets_dir).unwrap();
        let entry = dir.0.join("Contents/Resources/bundle.js");
        fs::write(
            &entry,
            "globalThis.evaluations = (globalThis.evaluations ?? 0) + 1;\n\
             import('./assets/chunk.js');",
        )
        .unwrap();
        fs::write(assets_dir.join("chunk.js"), "import '../bundle.js';").unwrap();

        let path = bundle_beside(&macos_dir).unwrap();
        let session = Session::load(
            &path.to_string_lossy(),
            &fs::read_to_string(&path).unwrap(),
            stderr_reporter(),
            None,
        )
        .unwrap();
        assert!(session.engine.run_jobs().is_empty());

        let evaluations: i32 = session.engine.eval("globalThis.evaluations").unwrap();
        assert_eq!(evaluations, 1);
    }
}
