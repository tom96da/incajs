// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The dev protocol: reads newline-delimited JSON-RPC on stdin, answers on
//! stdout, reloads a [`Session`] on `reload`, and relays any other
//! notification into the running app as `__inca_dev__.receive(method,
//! paramsJson)` — see [`crate::protocol`] for the wire format this speaks.

use std::fs;
use std::io::{self, BufRead, Write};
use std::rc::Rc;
use std::thread;

use gpui::{App, WindowHandle};
use serde_json::Value;

use inca_bridge::{ErrorReporter, call_dev_receive, drain_jobs_and_refresh};
use inca_jsenv::EngineError;

use crate::app::{HostedApp, Session, is_resizable, maybe_auto_resize_to_content};
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
}

/// Shared so the same writer reaches every place that reports something —
/// `respond`, a reload's failure, the dispatcher's reporter — without each
/// owning a copy of the underlying channel or thread.
pub(crate) type SharedWriter = Rc<dyn Writer>;

/// Owns real stdout on a dedicated thread. `write_line` only pushes onto an
/// unbounded channel, so a parent that reads its stdin slowly stalls that
/// thread, never the one rendering frames.
pub(crate) struct StdoutWriter(async_channel::Sender<String>);

impl StdoutWriter {
    pub(crate) fn spawn() -> Self {
        let (sender, receiver) = async_channel::unbounded::<String>();
        thread::spawn(move || {
            let mut stdout = io::stdout().lock();
            while let Ok(line) = receiver.recv_blocking() {
                if let Err(err) = writeln!(stdout, "{line}").and_then(|()| stdout.flush()) {
                    log::warn!("failed to write to stdout: {err}");
                    break;
                }
            }
        });
        Self(sender)
    }
}

impl Writer for StdoutWriter {
    fn write_line(&self, line: String) {
        if self.0.send_blocking(line).is_err() {
            log::warn!("stdout writer thread is gone");
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

/// Reads stdin on its own thread, because `gpui`'s `AsyncApp` isn't `Send`
/// and a blocking read must not sit on the main thread. The channel closing
/// means end of input: the parent went away.
fn stdin_lines() -> async_channel::Receiver<String> {
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

/// Where an app's own failures go in dev: the client is listening, and a
/// fault it can show beats a line it has to notice in a log. Outside dev
/// there is no writer at all — callers fall back to `stderr_reporter`
/// directly instead of calling this.
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
fn respond(id: Option<&Value>, outcome: Result<(), Failure>, writer: &SharedWriter) {
    let Some(id) = id else { return };
    send(
        &**writer,
        &match outcome {
            Ok(()) => Outgoing::result(id.clone()),
            Err(Failure::Message(code, message)) => Outgoing::error(id.clone(), code, message),
            Err(Failure::Thrown(err)) => {
                Outgoing::thrown(id.clone(), ErrorCode::BundleFailed, &err)
            }
        },
    );
}

/// Re-reads the entry and evaluates it into a fresh [`Session`], swapping
/// the window over only once that succeeds — an entry that fails to load
/// leaves the last working one on screen.
fn reload(
    window: &WindowHandle<HostedApp>,
    cx: &mut gpui::AsyncApp,
    entry_path: &str,
    id: Option<&Value>,
    writer: &SharedWriter,
) {
    let outcome = fs::read_to_string(entry_path)
        .map_err(|err| {
            Failure::Message(
                ErrorCode::BundleFailed,
                format!("failed to read {entry_path}: {err}"),
            )
        })
        .and_then(|source| {
            Session::load(entry_path, &source, reporter_for(writer), Some(writer))
                .map_err(Failure::Thrown)
        })
        .and_then(|session| {
            window
                .update(cx, |app, window, _| {
                    app.session = session;
                    // A fixed-size window gets a fresh auto-resize chance,
                    // as if it just launched with the reloaded content. A
                    // resizable window may carry a manual resize the user
                    // made since it last auto-resized, which reloading the
                    // bundle must not discard.
                    if !is_resizable(app.window_config.as_ref()) {
                        app.auto_resized.set(false);
                    }
                    drain_jobs_and_refresh(&app.session.engine, window);
                    maybe_auto_resize_to_content(app, window);
                })
                .map_err(|err| Failure::Message(ErrorCode::BundleFailed, err.to_string()))
        });

    respond(id, outcome, writer);
}

/// Relays an unrecognized notification (no `id`) into the running app via
/// [`inca_bridge::call_dev_receive`], then drains the job queue and
/// refreshes the window exactly as [`reload`] does — a callback may have
/// queued reactivity work or thrown.
fn relay_to_js(
    window: &WindowHandle<HostedApp>,
    cx: &mut gpui::AsyncApp,
    method: &str,
    params: Option<Value>,
    writer: &SharedWriter,
) {
    let params_json =
        serde_json::to_string(&params.unwrap_or(Value::Null)).unwrap_or_else(|_| "null".to_owned());
    let reporter = reporter_for(writer);

    let _ = window.update(cx, |app, window, _| {
        if let Some(err) = call_dev_receive(&app.session.engine, method, &params_json) {
            reporter(&err);
        }
        drain_jobs_and_refresh(&app.session.engine, window);
        maybe_auto_resize_to_content(app, window);
    });
}

/// Handles a decoded [`Incoming::Unrecognized`]: a request still answers
/// `-32601` exactly as before; a notification (no `id`) is relayed on into
/// the running app instead of being dropped.
fn handle_unrecognized(
    window: &WindowHandle<HostedApp>,
    cx: &mut gpui::AsyncApp,
    id: Option<Value>,
    method: &str,
    params: Option<Value>,
    writer: &SharedWriter,
) {
    let Some(id) = id else {
        relay_to_js(window, cx, method, params, writer);
        return;
    };
    respond(
        Some(&id),
        Err(Failure::Message(
            ErrorCode::MethodNotFound,
            "unknown method".to_owned(),
        )),
        writer,
    );
}

/// Answers protocol messages until `shutdown`, or until the parent closes
/// stdin, then quits the app.
pub(crate) fn serve_dev_protocol(
    cx: &mut App,
    window: WindowHandle<HostedApp>,
    entry_path: String,
    writer: SharedWriter,
) {
    let lines = stdin_lines();
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        while let Ok(line) = lines.recv().await {
            let (id, method) = match protocol::decode(&line) {
                Ok(Incoming::Call { id, method }) => (id, method),
                Ok(Incoming::Unrecognized { id, method, params }) => {
                    handle_unrecognized(&window, cx, id, &method, params, &writer);
                    continue;
                }
                Ok(Incoming::Empty) => continue,
                Err((code, message)) => {
                    send(&*writer, &Outgoing::error(Value::Null, code, message));
                    continue;
                }
            };

            match method {
                Method::Reload => reload(&window, cx, &entry_path, id.as_ref(), &writer),
                Method::Shutdown => {
                    respond(id.as_ref(), Ok(()), &writer);
                    break;
                }
            }
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// Reports a startup failure wherever anyone is listening. There is no
/// window to keep, so this is the last thing the process says. `writer` is
/// `None` outside dev, where there is nobody on the other end of stdout.
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
    use inca_bridge::stderr_reporter;
    use serde_json::json;

    use crate::app::start;

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

    #[gpui::test]
    fn an_unrecognized_notification_relays_into_the_running_app(cx: &mut TestAppContext) {
        let dev_writer: SharedWriter = Rc::new(CapturingWriter::default());
        let window = cx.update(|cx| {
            start(
                cx,
                TEST_ENTRY_PATH,
                RECEIVES_DEV_EVENTS,
                stderr_reporter(),
                Some(&dev_writer),
            )
            .unwrap()
        });
        cx.run_until_parked();

        let mut async_cx = cx.to_async();
        handle_unrecognized(
            &window,
            &mut async_cx,
            None,
            "testEvent",
            Some(json!({"a": 1})),
            &dev_writer,
        );
        cx.run_until_parked();

        let received = cx.update(|cx| {
            window
                .update(cx, |app, _, _| {
                    app.session
                        .engine
                        .eval::<String>("JSON.stringify(globalThis.received);")
                        .unwrap()
                })
                .unwrap()
        });
        assert_eq!(received, r#"[["testEvent","{\"a\":1}"]]"#);
    }

    #[gpui::test]
    fn an_unrecognized_request_still_answers_method_not_found(cx: &mut TestAppContext) {
        let capturing = Rc::new(CapturingWriter::default());
        let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;

        let window =
            cx.update(|cx| start(cx, TEST_ENTRY_PATH, "", stderr_reporter(), None).unwrap());
        cx.run_until_parked();

        let mut async_cx = cx.to_async();
        handle_unrecognized(
            &window,
            &mut async_cx,
            Some(json!(1)),
            "testEvent",
            None,
            &writer,
        );

        let sent = capturing.0.borrow();
        assert_eq!(sent.len(), 1);
        assert!(sent[0].contains(r#""code":-32601"#));
        assert!(sent[0].contains(r#""id":1"#));
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

    #[gpui::test]
    fn a_reload_resets_a_fixed_size_windows_auto_resize_latch(cx: &mut TestAppContext) {
        let entry = ScratchEntry::write("resize", &mounts_a_div(300, 150));
        let writer: SharedWriter = Rc::new(CapturingWriter::default());

        let window = cx.update(|cx| {
            start(
                cx,
                entry.path(),
                &mounts_a_div(300, 150),
                stderr_reporter(),
                Some(&writer),
            )
            .unwrap()
        });
        cx.run_until_parked();
        assert_eq!(bounds_of(cx, window), (300.0, 150.0));

        // A relay latches auto_resized — same size, so no visible change,
        // but this is the state a reload has to reset.
        let mut async_cx = cx.to_async();
        relay_to_js(&window, &mut async_cx, "testEvent", None, &writer);
        cx.run_until_parked();

        std::fs::write(&entry.0, mounts_a_div(400, 200)).unwrap();
        reload(&window, &mut async_cx, entry.path(), None, &writer);
        cx.run_until_parked();

        assert_eq!(
            bounds_of(cx, window),
            (400.0, 200.0),
            "reload must reset the latch, or the new content's size never applies"
        );
    }

    #[gpui::test]
    fn a_reload_does_not_reset_a_resizable_windows_manual_resize(cx: &mut TestAppContext) {
        let app = ScratchApp::write(
            "resizable",
            &mounts_a_div(300, 150),
            r#"{"window":{"resizable":true}}"#,
        );
        let writer: SharedWriter = Rc::new(CapturingWriter::default());

        let window = cx.update(|cx| {
            start(
                cx,
                &app.entry(),
                &mounts_a_div(300, 150),
                stderr_reporter(),
                Some(&writer),
            )
            .unwrap()
        });
        cx.run_until_parked();
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

        std::fs::write(app.0.join("bundle.js"), mounts_a_div(400, 200)).unwrap();
        let mut async_cx = cx.to_async();
        reload(&window, &mut async_cx, &app.entry(), None, &writer);
        cx.run_until_parked();

        assert_eq!(
            bounds_of(cx, window),
            (500.0, 500.0),
            "a resizable window the user resized by hand must not snap back on reload"
        );
    }
}
