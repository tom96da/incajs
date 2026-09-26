// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The dev protocol: reads newline-delimited JSON-RPC on stdin, answers on
//! stdout, and reloads a [`Session`] on `reload` — see [`crate::protocol`]
//! for the wire format this speaks.

use std::fs;
use std::io::{self, BufRead, Write};
use std::rc::Rc;
use std::thread;

use gpui::{App, WindowHandle};
use serde_json::Value;

use inca_bridge::{ErrorReporter, drain_jobs_and_refresh};
use inca_jsenv::EngineError;

use crate::app::{HostedApp, Session};
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
            Session::load(entry_path, &source, reporter_for(writer)).map_err(Failure::Thrown)
        })
        .and_then(|session| {
            window
                .update(cx, |app, window, _| {
                    app.session = session;
                    drain_jobs_and_refresh(&app.session.engine, window);
                })
                .map_err(|err| Failure::Message(ErrorCode::BundleFailed, err.to_string()))
        });

    respond(id, outcome, writer);
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
                Method::Unknown => respond(
                    id.as_ref(),
                    Err(Failure::Message(
                        ErrorCode::MethodNotFound,
                        "unknown method".to_owned(),
                    )),
                    &writer,
                ),
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
