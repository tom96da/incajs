// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The development protocol: reads newline-delimited JSON-RPC on stdin and
//! answers on stdout. A `reload` evaluates the new entry as a pending load
//! that swaps into the window when it settles. Any other notification goes
//! to the loading app while a load is pending, and to the window's app
//! otherwise, as `__inca_dev__.receive(method, paramsJson)`; see
//! [`wire`] for the wire format this speaks.
//!
//! The first entry starts as a [`crate::app::Loading`] and the window opens once its
//! top-level `await` settles. An entry still pending after [`serve::LOAD_TIMEOUT`]
//! opens the window at the size the config gives, else the mounted content's
//! size, else the default size, unless the app reported a failed load.

use std::cell::RefCell;
use std::io::{self, BufRead, Write};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use inca_jsenv::{EngineError, ErrorReporter};

pub(crate) use wire::ErrorCode;
use wire::Outgoing;

mod globals;
mod reload;
mod serve;
mod startup;
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

/// One queued line and, for a panic report, the sender that confirms its flush.
type Queued = (String, Option<mpsc::SyncSender<()>>);

/// How long the panic hook waits for the writer thread's flush: over 100 times
/// the slowest flush measured under 24 busy loops (8.15 ms).
const PANIC_REPORT_WAIT: Duration = Duration::from_secs(1);

/// Owns real stdout on a dedicated thread. `write_line` only pushes onto an
/// unbounded channel, so a parent that reads its stdin slowly stalls that
/// thread, never the one rendering frames. This thread is the only code that
/// holds the stdout lock.
pub(crate) struct StdoutWriter(
    async_channel::Sender<Queued>,
    RefCell<Option<thread::JoinHandle<()>>>,
    Option<thread::ThreadId>,
);

/// Writes each queued line to the sink `open` returns, flushes it, then
/// confirms a line that carries a confirmation sender.
fn write_lines<W: Write>(receiver: &async_channel::Receiver<Queued>, mut open: impl FnMut() -> W) {
    while let Ok((line, ack)) = receiver.recv_blocking() {
        let mut out = open();
        if let Err(err) = writeln!(out, "{line}").and_then(|()| out.flush()) {
            log::warn!("failed to write to stdout: {err}");
            break;
        }
        if let Some(ack) = ack {
            let _ = ack.send(());
        }
    }
}

impl StdoutWriter {
    pub(crate) fn spawn() -> Self {
        let (sender, receiver) = async_channel::unbounded();
        let handle = thread::spawn(move || write_lines(&receiver, || io::stdout().lock()));
        let id = handle.thread().id();
        Self(sender, RefCell::new(Some(handle)), Some(id))
    }

    /// Returns a clone of the sender to enqueue lines from other threads.
    pub(crate) fn sender_clone(&self) -> async_channel::Sender<Queued> {
        self.0.clone()
    }

    /// The id of the thread that writes the lines, `None` for a writer without one.
    pub(crate) fn thread_id(&self) -> Option<thread::ThreadId> {
        self.2
    }
}

impl Writer for StdoutWriter {
    fn write_line(&self, line: String) {
        if self.0.send_blocking((line, None)).is_err() {
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

/// Encodes a panic message as a JSON-RPC `-32603` error line with null id.
fn panic_line(message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": { "code": ErrorCode::InternalError.code(), "message": message },
    })
    .to_string()
}

/// Queues `message` as a `-32603` line on `sender` and waits up to `wait` for
/// the writer thread's flush. A closed channel, a panic on the writer thread
/// `writer` itself and a missing confirmation are written to `stderr`. The release host aborts on
/// a panic, so the line has to be flushed before the hook returns.
fn report_panic(
    sender: &async_channel::Sender<Queued>,
    writer: Option<thread::ThreadId>,
    message: &str,
    stderr: &mut impl io::Write,
    wait: Duration,
) {
    let line = panic_line(message);
    if writer == Some(thread::current().id()) {
        let _ = writeln!(
            stderr,
            "panic report not sent. the panic is on the writer thread"
        );
        return;
    }
    let (ack, confirmed) = mpsc::sync_channel(1);
    if sender.try_send((line, Some(ack))).is_err() {
        let _ = writeln!(
            stderr,
            "panic report not sent. the writer channel is closed"
        );
    } else if confirmed.recv_timeout(wait).is_err() {
        let _ = writeln!(
            stderr,
            "panic report not confirmed. the writer did not flush in time"
        );
    }
}

/// Installs a panic hook that runs the previous hook, then reports the panic
/// to the protocol client through `sender` as a `-32603` error with a `null`
/// id. `writer` is the thread that writes the lines.
pub(crate) fn install_panic_hook(
    sender: async_channel::Sender<Queued>,
    writer: Option<thread::ThreadId>,
) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        report_panic(
            &sender,
            writer,
            &info.to_string(),
            &mut io::stderr(),
            PANIC_REPORT_WAIT,
        );
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

pub(crate) use serve::serve_dev_protocol;
pub(crate) use startup::fail_startup;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use serde_json::Value;

    #[test]
    fn panic_line_keeps_the_internal_error_protocol_shape() {
        let message = "boom\nsecond line";
        let line = panic_line(message);
        assert_eq!(line.lines().count(), 1);
        let parsed: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], Value::Null);
        assert_eq!(parsed["error"]["code"], -32603);
        assert_eq!(parsed["error"]["message"], message);
    }

    /// A sink that records what had been flushed when a flush completes.
    #[derive(Clone, Default)]
    struct SlowSink {
        pending: Arc<Mutex<String>>,
        flushed: Arc<Mutex<String>>,
    }

    impl Write for SlowSink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.pending
                .lock()
                .unwrap()
                .push_str(&String::from_utf8_lossy(buf));
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            thread::sleep(Duration::from_millis(50));
            let pending = self.pending.lock().unwrap().clone();
            *self.flushed.lock().unwrap() = pending;
            Ok(())
        }
    }

    #[test]
    fn the_writer_acknowledges_a_panic_line_after_its_flush() {
        let (sender, receiver) = async_channel::unbounded();
        let sink = SlowSink::default();
        let writer_sink = sink.clone();
        let handle = thread::spawn(move || write_lines(&receiver, || writer_sink.clone()));
        let mut stderr = Vec::new();

        report_panic(&sender, None, "boom", &mut stderr, Duration::from_secs(5));

        assert!(sink.flushed.lock().unwrap().contains("boom"));
        assert!(stderr.is_empty());
        drop(sender);
        handle.join().unwrap();
    }

    #[test]
    fn a_closed_panic_report_channel_is_reported_on_stderr() {
        let (sender, receiver) = async_channel::unbounded::<Queued>();
        drop(receiver);
        let mut stderr = Vec::new();

        report_panic(&sender, None, "test", &mut stderr, Duration::from_secs(5));

        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "panic report not sent. the writer channel is closed\n"
        );
    }

    #[test]
    fn an_unacknowledged_panic_report_is_reported_on_stderr_after_the_wait() {
        let (sender, receiver) = async_channel::unbounded::<Queued>();
        let mut stderr = Vec::new();

        report_panic(
            &sender,
            None,
            "test",
            &mut stderr,
            Duration::from_millis(20),
        );

        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "panic report not confirmed. the writer did not flush in time\n"
        );
        drop(receiver);
    }

    #[test]
    fn a_panic_report_follows_lines_already_queued() {
        let (sender, receiver) = async_channel::unbounded();
        sender.send_blocking(("earlier".to_owned(), None)).unwrap();
        let mut stderr = Vec::new();

        report_panic(
            &sender,
            None,
            "panic",
            &mut stderr,
            Duration::from_millis(1),
        );

        assert_eq!(receiver.recv_blocking().unwrap().0, "earlier");
        let (report, ack) = receiver.recv_blocking().unwrap();
        assert!(report.contains("panic"));
        assert!(ack.is_some());
    }

    #[test]
    fn a_panic_on_the_writer_thread_is_reported_without_queueing() {
        let (sender, receiver) = async_channel::unbounded::<Queued>();
        let mut stderr = Vec::new();
        let start = Instant::now();

        report_panic(
            &sender,
            Some(thread::current().id()),
            "boom",
            &mut stderr,
            Duration::from_secs(5),
        );

        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "panic report not sent. the panic is on the writer thread\n"
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn a_regular_protocol_line_carries_no_acknowledgement() {
        let (sender, receiver) = async_channel::unbounded();
        let writer = StdoutWriter(sender, RefCell::new(None), None);

        writer.write_line("line".to_owned());

        let (line, ack) = receiver.recv_blocking().unwrap();
        assert_eq!(line, "line");
        assert!(ack.is_none());
    }

    struct FailingFlush;

    impl Write for FailingFlush {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("broken pipe"))
        }
    }

    #[test]
    fn a_failed_flush_ends_the_writer_and_drops_the_confirmation() {
        let (sender, receiver) = async_channel::unbounded();
        let handle = thread::spawn(move || write_lines(&receiver, || FailingFlush));
        let mut stderr = Vec::new();
        let start = Instant::now();

        report_panic(&sender, None, "boom", &mut stderr, Duration::from_secs(5));

        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "panic report not confirmed. the writer did not flush in time\n"
        );
        handle.join().unwrap();
    }
}
