// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The development protocol: reads newline-delimited JSON-RPC on stdin and
//! answers on stdout. A `reload` evaluates the new entry as a pending load
//! that swaps into the window when it settles. Any other notification goes
//! to the loading app while a load is pending, and to the window's app
//! otherwise, as `__inca_dev__.receive(method, paramsJson)` — see
//! [`crate::protocol`] for the wire format this speaks.
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

use inca_bridge::{ErrorReporter, call_dev_receive, report_jobs};
use inca_jsenv::EngineError;

use crate::app::{HostedApp, Loading, Session, is_resizable, maybe_auto_resize_to_content, open};
use crate::protocol::{self, ErrorCode, Incoming, Method, Outgoing};

/// Why a request could not be answered with a result.
#[derive(Debug)]
pub(crate) enum Failure {
    /// Something outside JS: an unreadable file, a window that went away.
    Message(ErrorCode, String),
    /// A value the app threw, which carries its own frames.
    Thrown(EngineError),
}

/// Where an encoded protocol line goes. Production writes to real stdout on
/// its own thread (see [`StdoutWriter`]); tests substitute something they can
/// read back.
pub(crate) trait Writer {
    fn write_line(&self, line: String);

    /// Waits until every line already written has reached its destination.
    fn close(&self) {}
}

/// Shared so the same writer reaches every place that reports something —
/// `respond`, a reload's failure, the dispatcher's reporter — without each
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
/// never reaches `writer` — there is nothing sound to send instead.
pub(crate) fn send(writer: &dyn Writer, message: &Outgoing) {
    match protocol::encode(message) {
        Ok(line) => writer.write_line(line),
        Err(err) => log::error!("failed to encode {message:?}: {err}"),
    }
}

/// Writes the `-32603` line reporting a panic to `out`, with `message` as
/// the error message, and flushes it. The write is synchronous: a panic
/// inside a platform callback can abort the process before a queued line
/// would be sent.
fn write_panic_line(out: &mut impl Write, message: &str) {
    let Ok(line) = protocol::encode(&Outgoing::error(
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

/// Adapts `writer` into the generic callback [`inca_bridge::install_dev`]
/// takes: a JS `__inca_dev__.send(method, paramsJson)` call becomes a
/// `method` notification written to `writer`.
pub(crate) fn dev_send(writer: &SharedWriter) -> inca_bridge::DevSend {
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
            Err(Failure::Message(code, message)) => {
                Outgoing::error(id.clone(), *code, message.clone())
            }
            Err(Failure::Thrown(err)) => Outgoing::thrown(id.clone(), ErrorCode::BundleFailed, err),
        },
    );
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
        .update(cx, |app, window, _| {
            app.session = session;
            // A fixed-size window gets a fresh auto-resize chance,
            // as if it just launched with the reloaded content. A
            // resizable window may carry a manual resize the user
            // made since it last auto-resized, which reloading the
            // bundle must not discard.
            if !is_resizable(app.window_config.as_ref()) {
                app.auto_resized.set((false, false));
            }
            app.session.dispatcher.drain_jobs_and_refresh(window);
            maybe_auto_resize_to_content(app, window);
        })
        .map_err(|err| Failure::Message(ErrorCode::BundleFailed, err.to_string()))
}

/// `params` as the JSON text `__inca_dev__.receive` takes.
fn params_json(params: Option<Value>) -> String {
    serde_json::to_string(&params.unwrap_or(Value::Null)).unwrap_or_else(|_| "null".to_owned())
}

/// Relays an unrecognized notification (no `id`) into the window's app via
/// [`inca_bridge::call_dev_receive`], then drains the job queue and
/// refreshes the window as a swap does — a callback may have queued
/// reactivity work or thrown.
fn relay_to_js(
    window: &WindowHandle<HostedApp>,
    cx: &mut App,
    method: &str,
    params: Option<Value>,
    writer: &SharedWriter,
) {
    let params_json = params_json(params);
    let reporter = reporter_for(writer);

    let _ = window.update(cx, |app, window, _| {
        if let Some(err) = call_dev_receive(&app.session.engine, method, &params_json) {
            reporter(&err);
        }
        app.session.dispatcher.drain_jobs_and_refresh(window);
        maybe_auto_resize_to_content(app, window);
    });
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

    /// Restarts the deadline when the app clears a failed-load report.
    fn track_hold(&mut self, cx: &App) {
        let held = self.held();
        if self.was_held && !held {
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
            report_jobs(
                &pending.loading.session().engine,
                &reporter_for(&self.writer),
            );
            self.track_hold(cx);
            return;
        };
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
        if self.window.is_some() || self.held() {
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
                    reporter_for(&self.writer),
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
            if let Some(err) = call_dev_receive(engine, method, &params_json) {
                reporter_for(&self.writer)(&err);
            }
            self.advance(cx);
        } else if let Some(window) = &self.window {
            relay_to_js(window, cx, method, params, &self.writer);
        }
    }

    /// Handles one stdin line.
    fn on_line(&mut self, cx: &mut App, line: &str) -> ControlFlow<()> {
        match protocol::decode(line) {
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
        (Failure::Thrown(err), Some(writer)) => {
            send(
                &**writer,
                &Outgoing::thrown(Value::Null, ErrorCode::BundleFailed, err),
            );
        }
        (Failure::Thrown(err), None) => eprintln!("{err}"),
        (Failure::Message(code, message), Some(writer)) => {
            send(
                &**writer,
                &Outgoing::error(Value::Null, *code, message.clone()),
            );
        }
        (Failure::Message(_, message), None) => eprintln!("{message}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use gpui::TestAppContext;
    use serde_json::json;

    const TEST_ENTRY_PATH: &str = "/test/entry.js";

    /// Captures every line written to it instead of touching real stdout, so
    /// a test can read back what was sent.
    #[derive(Default)]
    struct CapturingWriter(RefCell<Vec<String>>);

    impl Writer for CapturingWriter {
        fn write_line(&self, line: String) {
            self.0.borrow_mut().push(line);
        }
    }

    const RECEIVES_DEV_EVENTS: &str = r"
        globalThis.received = [];
        __inca_dev__.receive = (method, paramsJson) => {
            globalThis.received.push([method, paramsJson]);
        };
    ";

    #[test]
    fn a_panic_is_written_as_one_internal_error_line() {
        let mut out = Vec::new();

        write_panic_line(&mut out, "boom\nsecond line");

        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 1);
        let decoded: Value = serde_json::from_str(&text).unwrap();
        assert!(decoded["id"].is_null());
        assert_eq!(decoded["error"]["code"], -32603);
        assert_eq!(decoded["error"]["message"], "boom\nsecond line");
    }

    /// A real file on disk, torn down with the test — `reload` (unlike
    /// `start`'s test call sites elsewhere) reads its entry back off disk.
    struct ScratchEntry(std::path::PathBuf);

    impl ScratchEntry {
        fn write(name: &str, source: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "inca-host-dev-test-{name}-{}-{:?}.js",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::write(&path, source).unwrap();
            Self(path)
        }

        fn path(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }

    impl Drop for ScratchEntry {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn mounts_a_div(width: u32, height: u32) -> String {
        format!(
            r"
                const node = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
                __inca_native__.setStyle(node, 'width', {width});
                __inca_native__.setStyle(node, 'height', {height});
            "
        )
    }

    fn mounts_a_div_sized(property: &str, value: u32) -> String {
        format!(
            r"
                const node = __inca_native__.createNode('div');
                __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
                __inca_native__.setStyle(node, '{property}', {value});
            "
        )
    }

    /// A directory holding just this test's entry and its own `inca.json`,
    /// unlike [`ScratchEntry`] whose files all share the OS temp root as
    /// their `config::read` parent — fine for entries with no config, but
    /// not for a config a test needs isolated to itself.
    struct ScratchApp(std::path::PathBuf);

    impl ScratchApp {
        fn write(name: &str, source: &str, config_json: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "inca-host-dev-test-app-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("bundle.js"), source).unwrap();
            std::fs::write(dir.join("inca.json"), config_json).unwrap();
            Self(dir)
        }

        fn entry(&self) -> String {
            self.0.join("bundle.js").to_str().unwrap().to_owned()
        }
    }

    impl Drop for ScratchApp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn bounds_of(cx: &mut TestAppContext, window: WindowHandle<HostedApp>) -> (f32, f32) {
        cx.update(|cx| {
            window
                .update(cx, |_, window, _| {
                    let b = window.bounds();
                    (f32::from(b.size.width), f32::from(b.size.height))
                })
                .unwrap()
        })
    }

    /// One JSON-RPC line.
    fn line(message: &Value) -> String {
        let mut message = message.clone();
        message["jsonrpc"] = json!("2.0");
        message.to_string()
    }

    fn notify(sender: &async_channel::Sender<String>, method: &str) {
        sender
            .send_blocking(line(&json!({ "method": method })))
            .unwrap();
    }

    fn request(sender: &async_channel::Sender<String>, id: u32, method: &str) {
        sender
            .send_blocking(line(&json!({ "id": id, "method": method })))
            .unwrap();
    }

    /// Every line written so far, decoded.
    fn written(capturing: &CapturingWriter) -> Vec<Value> {
        capturing
            .0
            .borrow()
            .iter()
            .map(|sent| serde_json::from_str(sent).unwrap())
            .collect()
    }

    fn methods(capturing: &CapturingWriter) -> Vec<String> {
        written(capturing)
            .iter()
            .map(|sent| sent["method"].as_str().unwrap_or("").to_owned())
            .collect()
    }

    fn the_window(cx: &mut TestAppContext) -> Option<WindowHandle<HostedApp>> {
        cx.update(|cx| {
            cx.windows()
                .first()
                .and_then(gpui::AnyWindowHandle::downcast::<HostedApp>)
        })
    }

    /// Evaluates `script` in the session the window shows.
    fn eval_in_window(
        cx: &mut TestAppContext,
        window: WindowHandle<HostedApp>,
        script: &str,
    ) -> String {
        cx.update(|cx| {
            window
                .update(cx, |app, _, _| app.session.engine.eval(script).unwrap())
                .unwrap()
        })
    }

    /// Starts the serve loop on a fresh channel with `first` evaluating, and
    /// returns its sender. The loop owns the only receiver, so
    /// `receiver_count() == 0` means it ended.
    fn serve_started(
        cx: &mut TestAppContext,
        first: Loading,
        started: Instant,
        entry_path: &str,
        writer: &SharedWriter,
    ) -> async_channel::Sender<String> {
        let (sender, lines) = async_channel::unbounded();
        cx.update(|cx| {
            serve_dev_protocol(
                cx,
                lines,
                first,
                started,
                entry_path.to_owned(),
                Rc::clone(writer),
            );
        });
        sender
    }

    fn serve(
        cx: &mut TestAppContext,
        source: &str,
        entry_path: &str,
        writer: &SharedWriter,
    ) -> async_channel::Sender<String> {
        let started = cx.executor().now();
        let first = Session::begin(entry_path, source, reporter_for(writer), Some(writer)).unwrap();
        serve_started(cx, first, started, entry_path, writer)
    }

    /// Serves an entry that settles at once, and forgets the `ready` line.
    fn serve_idle(
        cx: &mut TestAppContext,
        source: &str,
        entry_path: &str,
        capturing: &Rc<CapturingWriter>,
    ) -> (WindowHandle<HostedApp>, async_channel::Sender<String>) {
        let writer: SharedWriter = Rc::clone(capturing) as SharedWriter;
        let sender = serve(cx, source, entry_path, &writer);
        cx.run_until_parked();
        assert_eq!(methods(capturing), ["ready"]);
        capturing.0.borrow_mut().clear();
        (the_window(cx).unwrap(), sender)
    }

    fn capture() -> (Rc<CapturingWriter>, SharedWriter) {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;
        (capturing, writer)
    }

    /// An entry that settles when any notification arrives, then mounts a
    /// `width` by `height` div.
    fn waits_then_mounts(width: u32, height: u32) -> String {
        format!(
            "await new Promise((r) => {{ __inca_dev__.receive = () => r(); }});\n{}",
            mounts_a_div(width, height)
        )
    }

    /// An entry no notification settles.
    const NEVER_SETTLES: &str = "await new Promise(() => {});";

    #[gpui::test]
    fn a_relayed_notification_reaches_the_app_in_the_window(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, RECEIVES_DEV_EVENTS, TEST_ENTRY_PATH, &capturing);

        sender
            .send_blocking(line(
                &json!({ "method": "testEvent", "params": { "a": 1 } }),
            ))
            .unwrap();
        cx.run_until_parked();

        let received = eval_in_window(cx, window, "JSON.stringify(globalThis.received);");
        assert_eq!(received, r#"[["testEvent","{\"a\":1}"]]"#);
    }

    #[gpui::test]
    fn a_reload_resets_a_fixed_size_windows_auto_resize_latch(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("resize", &mounts_a_div(400, 200));
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), entry.path(), &capturing);
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));

        // A relay latches auto_resized — same size, so no visible change,
        // but this is the state a reload has to reset.
        notify(&sender, "testEvent");
        request(&sender, 1, "reload");
        cx.run_until_parked();

        assert_eq!(
            bounds_of(cx, window),
            (400.0, 200.0),
            "reload must reset the latch, or the new content's size never applies"
        );
    }

    #[gpui::test]
    fn a_reload_resets_the_width_latch_when_only_the_width_fired(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("resize-width", &mounts_a_div_sized("width", 400));
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(
            cx,
            &mounts_a_div_sized("width", 300),
            entry.path(),
            &capturing,
        );
        assert_eq!(bounds_of(cx, window), (300.0, 600.0));

        request(&sender, 1, "reload");
        cx.run_until_parked();

        assert_eq!(bounds_of(cx, window), (400.0, 600.0));
    }

    #[gpui::test]
    fn a_reload_resets_the_height_latch_when_only_the_height_fired(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("resize-height", &mounts_a_div_sized("height", 200));
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(
            cx,
            &mounts_a_div_sized("height", 150),
            entry.path(),
            &capturing,
        );
        assert_eq!(bounds_of(cx, window), (800.0, 150.0));

        request(&sender, 1, "reload");
        cx.run_until_parked();

        assert_eq!(bounds_of(cx, window), (800.0, 200.0));
    }

    #[gpui::test]
    fn a_reload_does_not_reset_a_resizable_windows_manual_resize(cx: &mut TestAppContext) {
        let app = ScratchApp::write(
            "resizable",
            &mounts_a_div(400, 200),
            r#"{"window":{"resizable":true}}"#,
        );
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), &app.entry(), &capturing);
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));

        // The user grabs an edge and resizes it by hand.
        cx.update(|cx| {
            window
                .update(cx, |_, window, _| {
                    window.resize(gpui::size(gpui::px(500.0), gpui::px(500.0)));
                })
                .unwrap();
        });
        assert_eq!(bounds_of(cx, window), (500.0, 500.0));

        request(&sender, 1, "reload");
        cx.run_until_parked();

        assert_eq!(
            bounds_of(cx, window),
            (500.0, 500.0),
            "a resizable window the user resized by hand must not snap back on reload"
        );
    }

    #[gpui::test]
    fn shutdown_is_answered_and_later_lines_are_ignored(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

        request(&sender, 1, "shutdown");
        request(&sender, 2, "nope");
        cx.run_until_parked();

        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains(r#""id":1"#));
        assert!(sent[0].contains(r#""result":null"#));
        assert_eq!(sender.receiver_count(), 0, "the loop ended");
    }

    #[gpui::test]
    fn a_closed_channel_ends_the_loop(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);
        assert_eq!(sender.receiver_count(), 1, "the loop is waiting");

        sender.close();
        cx.run_until_parked();

        assert_eq!(sender.receiver_count(), 0, "the loop ended");
        assert!(capturing.0.borrow().is_empty());
    }

    #[gpui::test]
    fn a_reload_line_is_answered_null_and_swaps_the_session_in(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("serve-reload", &mounts_a_div(400, 200));
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, &mounts_a_div(300, 150), entry.path(), &capturing);

        request(&sender, 7, "reload");
        cx.run_until_parked();

        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains(r#""id":7"#));
        assert!(sent[0].contains(r#""result":null"#));
        assert_eq!(bounds_of(cx, window), (400.0, 200.0));
        assert_eq!(sender.receiver_count(), 1, "the loop keeps serving");
    }

    #[gpui::test]
    fn an_unusable_line_is_answered_and_the_loop_keeps_serving(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

        for (line, code) in [("not json", "-32700"), ("[1]", "-32600")] {
            capturing.0.borrow_mut().clear();
            sender.send_blocking(line.to_owned()).unwrap();
            cx.run_until_parked();

            let sent = capturing.0.borrow();
            assert_eq!(sent.len(), 1, "{line}");
            assert!(sent[0].contains(&format!(r#""code":{code}"#)), "{line}");
            assert!(sent[0].contains(r#""id":null"#), "{line}");
            assert_eq!(sender.receiver_count(), 1, "{line}");
        }

        capturing.0.borrow_mut().clear();
        sender.send_blocking("  ".to_owned()).unwrap();
        cx.run_until_parked();
        assert!(capturing.0.borrow().is_empty());
        assert_eq!(sender.receiver_count(), 1);
    }

    #[gpui::test]
    fn an_unknown_request_line_is_answered_method_not_found(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, "", TEST_ENTRY_PATH, &capturing);

        request(&sender, 3, "nope");
        cx.run_until_parked();

        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains(r#""code":-32601"#));
        assert!(sent[0].contains(r#""id":3"#));
        assert!(sent[0].contains(r#""message":"unknown method: nope""#));
        assert_eq!(sender.receiver_count(), 1, "the loop keeps serving");
    }

    #[gpui::test]
    fn an_entry_that_settles_at_once_opens_the_window_before_serve_returns(
        cx: &mut TestAppContext,
    ) {
        let (capturing, writer) = capture();

        let _sender = serve(cx, &mounts_a_div(300, 150), TEST_ENTRY_PATH, &writer);

        assert_eq!(methods(&capturing), ["ready"]);
        let window = the_window(cx).unwrap();
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));
    }

    #[gpui::test]
    fn a_pending_entry_keeps_the_window_closed_until_a_notification_settles_it(
        cx: &mut TestAppContext,
    ) {
        let (capturing, writer) = capture();
        let sender = serve(cx, &waits_then_mounts(300, 150), TEST_ENTRY_PATH, &writer);
        cx.run_until_parked();
        assert!(the_window(cx).is_none());
        assert!(capturing.0.borrow().is_empty());

        notify(&sender, "go");
        cx.run_until_parked();

        assert_eq!(methods(&capturing), ["ready"]);
        let window = the_window(cx).unwrap();
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));
    }

    #[gpui::test]
    fn a_pending_entry_with_nothing_mounted_opens_the_default_window_after_the_timeout(
        cx: &mut TestAppContext,
    ) {
        let (capturing, writer) = capture();
        let source = format!(
            "await new Promise((r) => {{ __inca_dev__.receive = () => {{ {} r(); }}; }});",
            mounts_a_div(300, 150)
        );
        let sender = serve(cx, &source, TEST_ENTRY_PATH, &writer);
        cx.run_until_parked();

        cx.executor().advance_clock(LOAD_TIMEOUT);
        cx.run_until_parked();

        assert_eq!(methods(&capturing), ["ready"]);
        let window = the_window(cx).unwrap();
        assert_eq!(bounds_of(cx, window), (800.0, 600.0));

        notify(&sender, "mount");
        cx.run_until_parked();
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));
    }

    #[gpui::test]
    fn a_line_that_settles_nothing_does_not_extend_the_timeout(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve(
            cx,
            "__inca_dev__.receive = () => {}; await new Promise(() => {});",
            TEST_ENTRY_PATH,
            &writer,
        );
        cx.run_until_parked();

        cx.executor().advance_clock(Duration::from_millis(1500));
        notify(&sender, "noise");
        cx.run_until_parked();
        assert!(the_window(cx).is_none());

        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
        assert_eq!(methods(&capturing), ["ready"]);
        assert!(the_window(cx).is_some());
    }

    #[gpui::test]
    fn a_job_failure_while_pending_is_one_app_error_before_the_window(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let source = format!(
            "Promise.reject(new Error('job failed'));\n{}",
            waits_then_mounts(300, 150)
        );
        let sender = serve(cx, &source, TEST_ENTRY_PATH, &writer);
        cx.run_until_parked();

        assert_eq!(methods(&capturing), ["appError"]);
        assert!(the_window(cx).is_none());

        notify(&sender, "go");
        cx.run_until_parked();
        assert_eq!(methods(&capturing), ["appError", "ready"]);
    }

    /// A pending entry that settles on `go`, records every notification in
    /// `globalThis.got`, and says which session it is in `globalThis.marker`.
    fn records_until_go(marker: &str) -> String {
        format!(
            "globalThis.marker = '{marker}'; globalThis.got = [];\n\
             await new Promise((r) => {{ __inca_dev__.receive = (m) => {{ \
                 globalThis.got.push(m); if (m === 'go') r(); }}; }});\n{}",
            mounts_a_div(400, 200)
        )
    }

    #[gpui::test]
    fn a_pending_reload_is_answered_when_it_settles_and_the_old_app_stays_until_then(
        cx: &mut TestAppContext,
    ) {
        let entry = ScratchEntry::write("pending-reload", &records_until_go("new"));
        let (capturing, _) = capture();
        let first = format!("globalThis.marker = 'old';\n{}", mounts_a_div(300, 150));
        let (window, sender) = serve_idle(cx, &first, entry.path(), &capturing);

        request(&sender, 5, "reload");
        cx.run_until_parked();
        assert!(capturing.0.borrow().is_empty(), "nothing answers yet");
        assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "old");
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));

        notify(&sender, "go");
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [json!({ "jsonrpc": "2.0", "id": 5, "result": null })]
        );
        assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "new");
        assert_eq!(bounds_of(cx, window), (400.0, 200.0));
    }

    #[gpui::test]
    fn a_notification_during_a_pending_reload_reaches_the_new_app_only(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("relay-pending", &records_until_go("new"));
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, RECEIVES_DEV_EVENTS, entry.path(), &capturing);

        request(&sender, 1, "reload");
        notify(&sender, "first");
        cx.run_until_parked();
        assert_eq!(
            eval_in_window(cx, window, "String(globalThis.received.length);"),
            "0",
            "the old app saw nothing"
        );

        notify(&sender, "go");
        cx.run_until_parked();
        assert_eq!(
            eval_in_window(cx, window, "JSON.stringify(globalThis.got);"),
            r#"["first","go"]"#
        );
    }

    #[gpui::test]
    fn a_later_reload_replaces_a_pending_one_and_both_ids_get_its_outcome(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("superseded", NEVER_SETTLES);
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, "", entry.path(), &capturing);
        request(&sender, 1, "reload");
        cx.run_until_parked();

        std::fs::write(&entry.0, records_until_go("second")).unwrap();
        request(&sender, 2, "reload");
        notify(&sender, "go");
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [
                json!({ "jsonrpc": "2.0", "id": 1, "result": null }),
                json!({ "jsonrpc": "2.0", "id": 2, "result": null }),
            ]
        );
        assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "second");
    }

    #[gpui::test]
    fn a_pending_reload_that_rejects_is_answered_thrown_and_the_old_app_stays(
        cx: &mut TestAppContext,
    ) {
        let entry = ScratchEntry::write(
            "rejected-reload",
            "await new Promise((_, reject) => { \
                 __inca_dev__.receive = () => reject(new Error('late')); });",
        );
        let (capturing, _) = capture();
        let first = format!("globalThis.marker = 'old';\n{}", mounts_a_div(300, 150));
        let (window, sender) = serve_idle(cx, &first, entry.path(), &capturing);

        request(&sender, 7, "reload");
        notify(&sender, "go");
        cx.run_until_parked();

        let sent = written(&capturing);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["id"], 7);
        assert_eq!(sent[0]["error"]["code"], -32000);
        assert_eq!(sent[0]["error"]["message"], "Error: late");
        assert!(sent[0]["error"]["data"]["stack"].is_string());
        assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "old");
    }

    #[gpui::test]
    fn a_reload_that_throws_at_once_is_answered_while_an_earlier_one_stays_pending(
        cx: &mut TestAppContext,
    ) {
        let entry = ScratchEntry::write("sync-throw", &records_until_go("second"));
        let (capturing, _) = capture();
        let (window, sender) = serve_idle(cx, "", entry.path(), &capturing);
        request(&sender, 1, "reload");
        cx.run_until_parked();

        std::fs::write(&entry.0, "throw new Error('sync');").unwrap();
        request(&sender, 2, "reload");
        cx.run_until_parked();
        let sent = written(&capturing);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["id"], 2);
        assert_eq!(sent[0]["error"]["message"], "Error: sync");

        notify(&sender, "go");
        cx.run_until_parked();
        let sent = written(&capturing);
        assert_eq!(sent.len(), 2);
        assert_eq!(
            sent[1],
            json!({ "jsonrpc": "2.0", "id": 1, "result": null })
        );
        assert_eq!(eval_in_window(cx, window, "globalThis.marker;"), "second");
    }

    #[gpui::test]
    fn a_reload_of_a_missing_file_is_answered_at_once(cx: &mut TestAppContext) {
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, "", "/test/missing/entry.js", &capturing);

        request(&sender, 4, "reload");
        cx.run_until_parked();

        let sent = written(&capturing);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["id"], 4);
        assert_eq!(sent[0]["error"]["code"], -32000);
        assert!(
            sent[0]["error"]["message"]
                .as_str()
                .unwrap()
                .starts_with("failed to read /test/missing/entry.js")
        );
    }

    #[gpui::test]
    fn a_rejection_after_the_timeout_is_one_app_error_and_the_window_stays(
        cx: &mut TestAppContext,
    ) {
        let (capturing, writer) = capture();
        let sender = serve(
            cx,
            "await new Promise((_, reject) => { \
                 __inca_dev__.receive = () => reject(new Error('late')); });",
            TEST_ENTRY_PATH,
            &writer,
        );
        cx.run_until_parked();
        cx.executor().advance_clock(LOAD_TIMEOUT);
        cx.run_until_parked();
        assert_eq!(methods(&capturing), ["ready"]);

        notify(&sender, "go");
        cx.run_until_parked();

        assert_eq!(methods(&capturing), ["ready", "appError"]);
        assert!(the_window(cx).is_some());
    }

    #[gpui::test]
    fn shutdown_while_loading_is_answered_and_no_window_opens(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve(cx, NEVER_SETTLES, TEST_ENTRY_PATH, &writer);

        request(&sender, 1, "shutdown");
        request(&sender, 2, "nope");
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [json!({ "jsonrpc": "2.0", "id": 1, "result": null })]
        );
        assert!(the_window(cx).is_none());
        assert_eq!(sender.receiver_count(), 0, "the loop ended");
    }

    #[gpui::test]
    fn a_closed_channel_while_loading_ends_the_loop(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve(cx, NEVER_SETTLES, TEST_ENTRY_PATH, &writer);
        cx.run_until_parked();

        sender.close();
        cx.run_until_parked();

        assert_eq!(sender.receiver_count(), 0, "the loop ended");
        assert!(capturing.0.borrow().is_empty());
    }

    /// An entry that settles on `go`, reports a failed load on `fail` and
    /// clears the report on `clear`.
    fn reports_load_failures() -> String {
        format!(
            "await new Promise((r) => {{ __inca_dev__.receive = (m) => {{ \
                 if (m === 'fail') __inca_dev__.setLoadFailed(true); \
                 if (m === 'clear') __inca_dev__.setLoadFailed(false); \
                 if (m === 'go') r(); }}; }});\n{}",
            mounts_a_div(300, 150)
        )
    }

    #[gpui::test]
    fn a_failed_load_report_keeps_the_window_closed_past_the_timeout(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve(cx, &reports_load_failures(), TEST_ENTRY_PATH, &writer);
        notify(&sender, "fail");
        cx.run_until_parked();

        cx.executor().advance_clock(LOAD_TIMEOUT * 3);
        cx.run_until_parked();
        assert!(the_window(cx).is_none());
        assert!(capturing.0.borrow().is_empty());

        notify(&sender, "go");
        cx.run_until_parked();
        assert_eq!(methods(&capturing), ["ready"]);
        let window = the_window(cx).unwrap();
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));
    }

    #[gpui::test]
    fn clearing_the_failed_load_report_restarts_the_timeout(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve(cx, &reports_load_failures(), TEST_ENTRY_PATH, &writer);
        notify(&sender, "fail");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(10));
        cx.run_until_parked();
        assert!(the_window(cx).is_none());

        notify(&sender, "clear");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(1900));
        cx.run_until_parked();
        assert!(the_window(cx).is_none());

        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        assert_eq!(methods(&capturing), ["ready"]);
        let window = the_window(cx).unwrap();
        assert_eq!(bounds_of(cx, window), (800.0, 600.0));
    }

    #[gpui::test]
    fn a_reload_replaces_a_held_first_load_with_a_fresh_timeout(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("held-reload", NEVER_SETTLES);
        let (capturing, writer) = capture();
        let sender = serve(
            cx,
            "__inca_dev__.setLoadFailed(true); await new Promise(() => {});",
            entry.path(),
            &writer,
        );
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(5));
        cx.run_until_parked();
        assert!(the_window(cx).is_none());

        request(&sender, 9, "reload");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(1900));
        cx.run_until_parked();
        assert!(the_window(cx).is_none());

        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert!(the_window(cx).is_some());
        assert_eq!(
            written(&capturing),
            [
                json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } }),
                json!({ "jsonrpc": "2.0", "id": 9, "result": null }),
            ]
        );
    }

    #[gpui::test]
    fn a_reload_that_replaces_a_load_that_is_not_held_keeps_the_deadline(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("keeps-deadline", NEVER_SETTLES);
        let (capturing, writer) = capture();
        let sender = serve(cx, NEVER_SETTLES, entry.path(), &writer);
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(1500));
        request(&sender, 1, "reload");
        cx.run_until_parked();
        assert!(the_window(cx).is_none());

        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();

        assert!(the_window(cx).is_some());
        assert_eq!(
            written(&capturing),
            [
                json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } }),
                json!({ "jsonrpc": "2.0", "id": 1, "result": null }),
            ]
        );
    }

    #[gpui::test]
    fn the_timeout_answers_a_queued_reload_once(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("timeout-ids", &records_until_go("second"));
        let (capturing, writer) = capture();
        let sender = serve(cx, NEVER_SETTLES, entry.path(), &writer);
        request(&sender, 3, "reload");
        cx.run_until_parked();

        cx.executor().advance_clock(LOAD_TIMEOUT);
        cx.run_until_parked();
        notify(&sender, "go");
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [
                json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } }),
                json!({ "jsonrpc": "2.0", "id": 3, "result": null }),
            ]
        );
    }

    #[gpui::test]
    fn shutdown_during_a_pending_reload_answers_only_the_shutdown(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("shutdown-b", NEVER_SETTLES);
        let (capturing, _) = capture();
        let (_, sender) = serve_idle(cx, "", entry.path(), &capturing);
        request(&sender, 1, "reload");
        request(&sender, 2, "shutdown");
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [json!({ "jsonrpc": "2.0", "id": 2, "result": null })]
        );
        assert_eq!(sender.receiver_count(), 0, "the loop ended");
    }

    #[gpui::test]
    fn shutdown_ends_a_held_first_load(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve(
            cx,
            "__inca_dev__.setLoadFailed(true); await new Promise(() => {});",
            TEST_ENTRY_PATH,
            &writer,
        );

        request(&sender, 1, "shutdown");
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [json!({ "jsonrpc": "2.0", "id": 1, "result": null })]
        );
        assert_eq!(sender.receiver_count(), 0);
        assert!(the_window(cx).is_none());
    }

    fn ready_message() -> Value {
        json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } })
    }

    #[gpui::test]
    fn the_deadline_counts_from_the_start_of_the_evaluation(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let started = cx.executor().now();
        let first = Session::begin(
            TEST_ENTRY_PATH,
            NEVER_SETTLES,
            reporter_for(&writer),
            Some(&writer),
        )
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(1500));
        let sender = serve_started(cx, first, started, TEST_ENTRY_PATH, &writer);
        cx.run_until_parked();
        assert!(the_window(cx).is_none());

        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();

        assert!(the_window(cx).is_some());
        assert_eq!(written(&capturing), [ready_message()]);
        assert_eq!(sender.receiver_count(), 1);
    }

    #[gpui::test]
    fn a_queued_reload_is_answered_when_the_first_load_settles_in_time(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("queued-settles", &records_until_go("second"));
        let (capturing, writer) = capture();
        let sender = serve(cx, NEVER_SETTLES, entry.path(), &writer);
        request(&sender, 4, "reload");
        notify(&sender, "go");
        cx.run_until_parked();

        assert_eq!(
            written(&capturing),
            [
                ready_message(),
                json!({ "jsonrpc": "2.0", "id": 4, "result": null }),
            ]
        );
    }

    #[gpui::test]
    fn the_timeout_opens_the_window_at_the_size_of_content_already_mounted(
        cx: &mut TestAppContext,
    ) {
        let (capturing, writer) = capture();
        let source = format!("{}\n{NEVER_SETTLES}", mounts_a_div(300, 150));
        let _sender = serve(cx, &source, TEST_ENTRY_PATH, &writer);
        cx.run_until_parked();

        cx.executor().advance_clock(LOAD_TIMEOUT);
        cx.run_until_parked();

        let window = the_window(cx).unwrap();
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));
        assert_eq!(written(&capturing), [ready_message()]);
    }

    #[gpui::test]
    fn a_receive_that_throws_while_loading_is_reported(cx: &mut TestAppContext) {
        let (capturing, writer) = capture();
        let sender = serve(
            cx,
            &format!("__inca_dev__.receive = () => {{ throw new Error('rx'); }};\n{NEVER_SETTLES}"),
            TEST_ENTRY_PATH,
            &writer,
        );

        notify(&sender, "probe");
        cx.run_until_parked();

        let sent = written(&capturing);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["method"], "appError");
        assert_eq!(sent[0]["params"]["message"], "Error: rx");
        assert!(the_window(cx).is_none());
    }
}
