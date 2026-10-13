// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The development protocol: reads newline-delimited JSON-RPC on stdin and
//! answers on stdout. A `reload` evaluates the new entry as a pending load
//! that swaps into the window when it settles. Any other notification goes
//! to the loading app while a load is pending, and to the window's app
//! otherwise, as `__inca_dev__.receive(method, paramsJson)`; see
//! [`wire`] for the wire format this speaks.
//!
//! The first entry starts as a [`Loading`] and the window opens once its
//! top-level `await` settles. An entry still pending after [`LOAD_TIMEOUT`]
//! opens the window at the size the config gives, else the mounted content's
//! size, else the default size, unless the app reported a failed load.

use std::cell::RefCell;
use std::fs;
use std::io::{self, BufRead, Write};
use std::ops::ControlFlow;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use gpui::{App, FutureExt, WindowHandle};
use serde_json::Value;

use inca_jsenv::{Engine, EngineError, ErrorReporter, report_jobs};

use crate::app::{Failure, HostedApp, Loading, Session, maybe_auto_resize_to_content, open};
use crate::config::is_resizable;
pub(crate) use wire::ErrorCode;
use wire::{Incoming, Method, Outgoing};

mod globals;
mod wire;

pub(crate) use globals::{DevSend, LoadSignal, call_dev_receive, install_dev, install_load_signal};

/// Where an encoded protocol line goes. Production writes to real stdout on
/// its own thread (see [`StdoutWriter`]); tests substitute something they can
/// read back.
pub(crate) trait Writer {
    fn write_line(&self, line: String);

    /// Waits until every line already written has reached its destination.
    fn close(&self) {}
}

/// Shared so the same writer reaches every place that reports something
/// (`respond`, a reload's failure, the dispatcher's reporter) without each
/// owning a copy of the underlying channel or thread.
pub(crate) type SharedWriter = Rc<dyn Writer>;

/// Owns real stdout on a dedicated thread. `write_line` only pushes onto an
/// unbounded channel, so a parent that reads its stdin slowly stalls that
/// thread, never the one rendering frames. The thread holds the stdout lock
/// one line at a time, so [`install_panic_hook`] can write between lines.
pub(crate) struct StdoutWriter(
    async_channel::Sender<String>,
    RefCell<Option<thread::JoinHandle<()>>>,
);

impl StdoutWriter {
    pub(crate) fn spawn() -> Self {
        let (sender, receiver) = async_channel::unbounded::<String>();
        let handle = thread::spawn(move || {
            while let Ok(line) = receiver.recv_blocking() {
                let mut stdout = io::stdout().lock();
                if let Err(err) = writeln!(stdout, "{line}").and_then(|()| stdout.flush()) {
                    log::warn!("failed to write to stdout: {err}");
                    break;
                }
            }
        });
        Self(sender, RefCell::new(Some(handle)))
    }
}

impl Writer for StdoutWriter {
    fn write_line(&self, line: String) {
        if self.0.send_blocking(line).is_err() {
            log::warn!("stdout writer thread is gone");
        }
    }

    fn close(&self) {
        self.0.close();
        if let Some(handle) = self.1.borrow_mut().take() {
            let _ = handle.join();
        }
    }
}

/// Encodes and writes one protocol line. A message that fails to encode
/// never reaches `writer`. There is nothing sound to send instead.
pub(crate) fn send(writer: &dyn Writer, message: &Outgoing) {
    match wire::encode(message) {
        Ok(line) => writer.write_line(line),
        Err(err) => log::error!("failed to encode {message:?}: {err}"),
    }
}

/// Writes the `-32603` line reporting a panic to `out`, with `message` as
/// the error message, and flushes it. The write is synchronous: a panic
/// inside a platform callback can abort the process before a queued line
/// would be sent.
fn write_panic_line(out: &mut impl Write, message: &str) {
    let Ok(line) = wire::encode(&Outgoing::error(
        Value::Null,
        ErrorCode::InternalError,
        message,
    )) else {
        return;
    };
    let _ = writeln!(out, "{line}").and_then(|()| out.flush());
}

/// Installs a panic hook that reports each panic to the protocol client as
/// a `-32603` error with a `null` id, after the previously installed hook
/// has run.
pub(crate) fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        write_panic_line(&mut io::stdout().lock(), &info.to_string());
    }));
}

/// Reads stdin on its own thread, because `gpui`'s `AsyncApp` isn't `Send`
/// and a blocking read must not sit on the main thread. The channel closing
/// means end of input: the parent went away.
pub(crate) fn stdin_lines() -> async_channel::Receiver<String> {
    let (sender, receiver) = async_channel::unbounded();
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if sender.send_blocking(line).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    log::error!("failed to read stdin: {err}");
                    break;
                }
            }
        }
    });
    receiver
}

/// Reports each app failure to the client as an `appError` notification on
/// `writer`.
pub(crate) fn reporter_for(writer: &SharedWriter) -> ErrorReporter {
    let writer = Rc::clone(writer);
    Rc::new(move |err: &EngineError| send(&*writer, &Outgoing::app_error(err)))
}

/// Adapts `writer` into the generic callback [`crate::dev::install_dev`]
/// takes: a JS `__inca_dev__.send(method, paramsJson)` call becomes a
/// `method` notification written to `writer`.
pub(crate) fn dev_send(writer: &SharedWriter) -> crate::dev::DevSend {
    let writer = Rc::clone(writer);
    Rc::new(move |method: &str, params| send(&*writer, &Outgoing::notification(method, params)))
}

/// Answers the request `id` came from. A notification carries no id and
/// takes no reply.
fn respond(id: Option<&Value>, outcome: &Result<(), Failure>, writer: &SharedWriter) {
    let Some(id) = id else { return };
    send(
        &**writer,
        &match outcome {
            Ok(()) => Outgoing::result(id.clone()),
            Err(failure) => outgoing(id.clone(), failure),
        },
    );
}

/// The error message that reports `failure` for the request `id`.
fn outgoing(id: Value, failure: &Failure) -> Outgoing {
    match failure {
        Failure::Message(code, message) => Outgoing::error(id, *code, message.clone()),
        Failure::Thrown(err) => Outgoing::thrown(id, ErrorCode::BundleFailed, err),
    }
}

/// How long the first load may stay pending with no window before the window
/// opens at the size the config gives, else the mounted content's size, else
/// the default size.
pub(crate) const LOAD_TIMEOUT: Duration = Duration::from_secs(2);

/// Puts `session` on screen in place of the window's current one.
fn swap_in(
    window: &WindowHandle<HostedApp>,
    cx: &mut App,
    session: Session,
) -> Result<(), Failure> {
    window
        .update(cx, |app, window, cx| {
            app.session = session;
            // A fixed-size window gets a fresh auto-resize chance,
            // as if it just launched with the reloaded content. A
            // resizable window may carry a manual resize the user
            // made since it last auto-resized, which reloading the
            // bundle must not discard.
            if !is_resizable(app.window_config.as_ref()) {
                app.auto_resized.set((false, false));
            }
            app.session.dispatcher.drain_jobs_and_refresh(window, cx);
            maybe_auto_resize_to_content(app, window);
        })
        .map_err(Failure::bundle)
}

/// `params` as the JSON text `__inca_dev__.receive` takes.
fn params_json(params: Option<Value>) -> String {
    serde_json::to_string(&params.unwrap_or(Value::Null)).unwrap_or_else(|_| "null".to_owned())
}

/// Relays an unrecognized notification (no `id`) into the window's app via
/// [`crate::dev::call_dev_receive`], then drains the job queue and
/// refreshes the window as a swap does. A callback may have queued
/// reactivity work or thrown.
fn relay_to_js(
    window: &WindowHandle<HostedApp>,
    cx: &mut App,
    method: &str,
    params: Option<Value>,
    reporter: &ErrorReporter,
) {
    let params_json = params_json(params);

    let _ = window.update(cx, |app, window, cx| {
        app.session.dispatcher.observe_focus(window, cx);
        deliver(&app.session.engine, method, &params_json, reporter);
        app.session.dispatcher.drain_jobs_and_refresh(window, cx);
        maybe_auto_resize_to_content(app, window);
    });
}

/// Calls `__inca_dev__.receive` on `engine` and reports what it throws.
fn deliver(engine: &Engine, method: &str, params_json: &str, reporter: &ErrorReporter) {
    if let Some(err) = call_dev_receive(engine, method, params_json) {
        reporter(&err);
    }
}

/// An entry that is still evaluating, with the `reload` ids waiting on it.
struct Pending {
    loading: Loading,
    ids: Vec<Value>,
}

/// What the serve loop knows: whether a window exists, the load in flight
/// and when the first load's wait ends.
struct Serve {
    window: Option<WindowHandle<HostedApp>>,
    pending: Option<Pending>,
    deadline: Instant,
    was_held: bool,
    entry_path: String,
    writer: SharedWriter,
    reporter: ErrorReporter,
}

impl Serve {
    /// Whether the first load waits for the app: no window yet and the app
    /// reported a failed load.
    fn held(&self) -> bool {
        self.window.is_none()
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.loading.start_failed())
    }

    /// Restarts the deadline when the app clears a failed-load report, even
    /// when the report and the clearing happened between two polls.
    fn track_hold(&mut self, cx: &App) {
        let held = self.held();
        let reported = self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.loading.reported_failure());
        if (self.was_held || reported) && !held {
            self.deadline = cx.background_executor().now() + LOAD_TIMEOUT;
        }
        self.was_held = held;
    }

    /// How long to wait for a line before the window opens. `None` waits for
    /// the next line.
    fn wait(&self, cx: &gpui::AsyncApp) -> Option<Duration> {
        (self.window.is_none() && !self.held()).then(|| {
            self.deadline
                .saturating_duration_since(cx.background_executor().now())
        })
    }

    /// Polls the pending load and acts on its outcome.
    fn advance(&mut self, cx: &mut App) {
        let Some(pending) = &self.pending else { return };
        let Some(result) = pending.loading.poll() else {
            let session = pending.loading.session();
            report_jobs(&session.engine, &self.reporter);
            session.dispatcher.report_pending();
            self.track_hold(cx);
            return;
        };
        if result.is_err() {
            pending
                .loading
                .session()
                .report_failed_start(&self.reporter);
        } else {
            pending.loading.session().dispatcher.report_pending();
        }
        let Some(Pending { loading, ids }) = self.pending.take() else {
            return;
        };
        let outcome = match (result, self.window) {
            (Ok(()), None) => {
                self.open_window(cx, loading.into_session());
                return self.answer(&ids, &Ok(()));
            }
            (Ok(()), Some(window)) => swap_in(&window, cx, loading.into_session()),
            (Err(err), None) => fail_startup(&Failure::Thrown(err), Some(&self.writer)),
            (Err(err), Some(_)) => Err(Failure::Thrown(err)),
        };
        self.answer(&ids, &outcome);
    }

    /// Opens the window on `session` and announces it.
    fn open_window(&mut self, cx: &mut App, session: Session) {
        match open(cx, session, &self.entry_path) {
            Ok(window) => {
                self.window = Some(window);
                send(&*self.writer, &Outgoing::ready());
            }
            Err(failure) => fail_startup(&failure, Some(&self.writer)),
        }
    }

    fn answer(&self, ids: &[Value], outcome: &Result<(), Failure>) {
        for id in ids {
            respond(Some(id), outcome, &self.writer);
        }
    }

    /// The wait for the first load ended: one more poll, then the window
    /// opens on whatever the app has mounted.
    fn time_out(&mut self, cx: &mut App) {
        self.advance(cx);
        // A report or a clear made in the final poll moves the deadline on.
        if self.window.is_some() || self.held() || cx.background_executor().now() < self.deadline {
            return;
        }
        if let Some(Pending { loading, ids }) = self.pending.take() {
            self.open_window(cx, loading.into_session());
            self.answer(&ids, &Ok(()));
        }
    }

    /// Re-reads the entry and starts it as the pending load. A load that
    /// cannot start is answered at once and leaves the state as it was.
    fn reload(&mut self, cx: &mut App, id: Option<Value>) {
        let begun = fs::read_to_string(&self.entry_path)
            .map_err(|err| {
                Failure::Message(
                    ErrorCode::BundleFailed,
                    format!("failed to read {}: {err}", self.entry_path),
                )
            })
            .and_then(|source| {
                Session::begin(
                    &self.entry_path,
                    &source,
                    Rc::clone(&self.reporter),
                    Some(&self.writer),
                )
                .map_err(Failure::Thrown)
            });
        let loading = match begun {
            Ok(loading) => loading,
            Err(failure) => return respond(id.as_ref(), &Err(failure), &self.writer),
        };
        let mut ids = self
            .pending
            .take()
            .map(|pending| pending.ids)
            .unwrap_or_default();
        ids.extend(id);
        self.pending = Some(Pending { loading, ids });
        self.track_hold(cx);
        self.advance(cx);
    }

    /// Relays a notification to the load in flight, or to the window's app
    /// when none is.
    fn relay(&mut self, cx: &mut App, method: &str, params: Option<Value>) {
        if let Some(pending) = &self.pending {
            let params_json = params_json(params);
            let engine = &pending.loading.session().engine;
            deliver(engine, method, &params_json, &self.reporter);
            self.advance(cx);
        } else if let Some(window) = &self.window {
            relay_to_js(window, cx, method, params, &self.reporter);
        }
    }

    /// Handles one stdin line.
    fn on_line(&mut self, cx: &mut App, line: &str) -> ControlFlow<()> {
        match wire::decode(line) {
            Ok(Incoming::Call {
                id,
                method: Method::Reload,
            }) => self.reload(cx, id),
            Ok(Incoming::Call {
                id,
                method: Method::Shutdown,
            }) => {
                respond(id.as_ref(), &Ok(()), &self.writer);
                return ControlFlow::Break(());
            }
            Ok(Incoming::Unrecognized {
                id: Some(id),
                method,
                ..
            }) => respond(
                Some(&id),
                &Err(Failure::Message(
                    ErrorCode::MethodNotFound,
                    format!("unknown method: {method}"),
                )),
                &self.writer,
            ),
            Ok(Incoming::Unrecognized {
                id: None,
                method,
                params,
            }) => self.relay(cx, &method, params),
            Ok(Incoming::Empty) => {}
            Err((code, message)) => {
                send(&*self.writer, &Outgoing::error(Value::Null, code, message));
            }
        }
        ControlFlow::Continue(())
    }
}

/// Starts serving the protocol messages arriving on `lines` while `first`
/// evaluates, and returns once the first poll has run. The loop task answers
/// until `shutdown`, or until the channel closes, then quits the app.
///
/// The window opens when `first` settles, or [`LOAD_TIMEOUT`] after `started`
/// (the instant just before `first` began evaluating) when it is still
/// pending, unless the app reported a failed load. A `first` that fails with
/// no window open ends the process.
pub(crate) fn serve_dev_protocol(
    cx: &mut App,
    lines: async_channel::Receiver<String>,
    first: Loading,
    started: Instant,
    entry_path: String,
    writer: SharedWriter,
) {
    let mut serve = Serve {
        window: None,
        pending: Some(Pending {
            loading: first,
            ids: Vec::new(),
        }),
        deadline: started + LOAD_TIMEOUT,
        was_held: false,
        entry_path,
        reporter: reporter_for(&writer),
        writer,
    };
    serve.track_hold(cx);
    serve.advance(cx);

    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        loop {
            let received = match serve.wait(cx) {
                Some(wait) => lines
                    .recv()
                    .with_timeout(wait, cx.background_executor())
                    .await
                    .ok(),
                None => Some(lines.recv().await),
            };
            let flow = match received {
                Some(Ok(line)) => cx.update(|cx| serve.on_line(cx, &line)),
                Some(Err(_)) => ControlFlow::Break(()),
                None => {
                    cx.update(|cx| serve.time_out(cx));
                    ControlFlow::Continue(())
                }
            };
            if flow.is_break() {
                break;
            }
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// Reports a startup failure, closes the writer and ends the process with
/// status 1. No window ever opened, so the event loop has nothing to quit
/// from. `writer` is `None` in production.
pub(crate) fn fail_startup(failure: &Failure, writer: Option<&SharedWriter>) -> ! {
    report_startup_failure(failure, writer);
    if let Some(writer) = writer {
        writer.close();
    }
    std::process::exit(1)
}

/// Reports a startup failure wherever anyone is listening. There is no
/// window to keep, so this is the last thing the process says. `writer` is
/// `None` in production, where there is nobody on the other end of stdout.
pub(crate) fn report_startup_failure(failure: &Failure, writer: Option<&SharedWriter>) {
    match (failure, writer) {
        (failure, Some(writer)) => send(&**writer, &outgoing(Value::Null, failure)),
        (Failure::Thrown(err), None) => eprintln!("{err}"),
        (Failure::Message(_, message), None) => eprintln!("{message}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
