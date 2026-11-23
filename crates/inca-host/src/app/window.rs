// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window of an app: the entry that is still loading, the view that draws the document, and
//! the opening of the window.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, Point, SharedString, TitlebarOptions, Window, WindowBounds, WindowHandle,
    WindowOptions, div, prelude::*, px, size,
};

use inca_gpui::{EventSink, NodeLayout, Scrollbars, build_element_with_events, node_gutter};
use inca_jsenv::{EngineError, ErrorReporter, PendingModule};
use inca_style::build_spec;

use super::session::{Failure, Session, clock_of};
use super::size::{content_window_size, maybe_auto_resize_to_content};
use crate::config::{
    self, DEFAULT_APP_NAME, is_resizable, window_min_size, window_size, window_title,
};
use crate::dev::LoadSignal;
use crate::menu;

/// A [`Session`] whose entry is still evaluating.
pub(crate) struct Loading {
    pub(super) module: PendingModule,
    pub(super) session: Session,
    pub(super) signal: Rc<LoadSignal>,
    pub(super) seen_failures: Cell<u32>,
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
        session.dispatcher.clear_layout();
        let mode = Scrollbars::of(cx);
        let host = session.cells.document.borrow();
        let top = host.tree.document_element();
        let spec = top.and_then(|id| build_spec(&host.tree, &host.style, id));
        let element = spec.as_ref().map_or_else(
            || div().into_any_element(),
            |spec| build_element_with_events(spec, &session.dispatcher, mode),
        );
        // The element of the document fills the window; the walk records only its children.
        if let Some(id) = top {
            let gutter = spec
                .as_ref()
                .map(|spec| node_gutter(spec, &session.dispatcher, mode))
                .unwrap_or_default();
            session.dispatcher.record_layout(
                id,
                NodeLayout {
                    bounds: Bounds::new(Point::default(), window.viewport_size()),
                    border: px(0.0),
                    scroll: Point::default(),
                    gutter,
                },
            );
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
    let content = content_window_size(&session.host.document.borrow(), session.root());
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
                // A window without focus keeps its frame rate for `requestAnimationFrame`.
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use gpui::TestAppContext;
    use inca_jsenv::stderr_reporter;
    use inca_test_macros::gpui_cases;

    use crate::tests::TEST_ENTRY_PATH;

    const DEFERS_ITS_MOUNT: &str = r"
        globalThis.mounted = false;
        Promise.resolve().then(() => { globalThis.mounted = true; });
    ";

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

    /// Runs `script` in the engine of `window`, redraws it and evaluates `read` to a string.
    fn after_draw(
        cx: &mut TestAppContext,
        window: WindowHandle<HostedApp>,
        script: &str,
        read: &str,
    ) -> String {
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
                    app.session.engine.eval::<String>(read).unwrap()
                })
                .unwrap()
        })
    }

    #[gpui::test]
    fn the_root_has_a_box_only_while_the_document_holds_it(cx: &mut TestAppContext) {
        cx.update(|cx| start(cx, TEST_ENTRY_PATH, "", stderr_reporter()).unwrap());
        cx.run_until_parked();
        let window = cx.update(|cx| cx.windows()[0].downcast::<HostedApp>().unwrap());
        let drawn_box = |cx: &mut TestAppContext, script: &str| -> bool {
            let read = "String(__inca_native__.layoutOf(__inca_native__.rootNodeId()) !== null)";
            after_draw(cx, window, script, read) == "true"
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

    #[gpui_cases]
    #[gpui::test]
    #[case::scroll("scroll", "15,15")]
    #[case::auto("auto", "0,0")]
    fn the_root_box_records_the_strips_of_its_own_overflow(
        cx: &mut TestAppContext,
        overflow: &str,
        strips: &str,
    ) {
        cx.update(|cx| start(cx, TEST_ENTRY_PATH, "", stderr_reporter()).unwrap());
        cx.run_until_parked();
        let window = cx.update(|cx| cx.windows()[0].downcast::<HostedApp>().unwrap());
        let script = format!(
            "__inca_native__.setStyle(__inca_native__.rootNodeId(), 'overflow', '{overflow}');"
        );
        let read = "String(__inca_native__.layoutOf(__inca_native__.rootNodeId()).slice(7))";
        assert_eq!(after_draw(cx, window, &script, read), strips);
    }
}
