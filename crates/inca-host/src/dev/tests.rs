// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use gpui::TestAppContext;
use serde_json::json;

use crate::tests::ScratchDir;

mod loading;
mod reload;
mod serve;
mod timers;

const TEST_ENTRY_PATH: &str = "/test/entry.js";

/// Captures every line written to it, so a test can read back what was sent.
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

/// Writes `source` as the entry of `dir`, which then holds no config of its
/// own, and returns the entry path.
fn write_entry(dir: &ScratchDir, source: &str) -> String {
    let path = dir.join("bundle.js");
    std::fs::write(&path, source).unwrap();
    path.to_str().unwrap().to_owned()
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

/// [`serve`] over the executor clock, so the timers of the first session follow
/// `advance_clock`.
fn serve_on_executor_clock(
    cx: &mut TestAppContext,
    source: &str,
    entry_path: &str,
    writer: &SharedWriter,
) -> async_channel::Sender<String> {
    let started = cx.executor().now();
    let clock = crate::app::clock_of(&cx.executor());
    let first = Session::begin_with(
        entry_path,
        source,
        reporter_for(writer),
        Some(writer),
        clock,
    )
    .unwrap();
    serve_started(cx, first, started, entry_path, writer)
}

/// [`serve_idle`] over the executor clock.
fn serve_idle_on_executor_clock(
    cx: &mut TestAppContext,
    source: &str,
    entry_path: &str,
    capturing: &Rc<CapturingWriter>,
) -> (WindowHandle<HostedApp>, async_channel::Sender<String>) {
    let writer: SharedWriter = Rc::clone(capturing) as SharedWriter;
    let sender = serve_on_executor_clock(cx, source, entry_path, &writer);
    cx.run_until_parked();
    assert_eq!(methods(capturing), ["ready"]);
    capturing.0.borrow_mut().clear();
    (the_window(cx).unwrap(), sender)
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

/// An entry no notification settles.
const NEVER_SETTLES: &str = "await new Promise(() => {});";

fn ready_message() -> Value {
    json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } })
}

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
