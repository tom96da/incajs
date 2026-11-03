// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the tests of several modules.

#![allow(clippy::unwrap_used)]

use std::{
    env, fs,
    ops::Deref,
    path::{Path, PathBuf},
};

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gpui::{TestAppContext, WindowHandle};
use serde_json::{Value, json};

use inca_jsenv::{EngineError, stderr_reporter};

use inca_gpui::NodeId;

use crate::app::{HostedApp, Loading, Session};
use crate::dev::{SharedWriter, Writer, reporter_for, serve_dev_protocol};
use crate::harness::Harness;
use crate::snapshot::Node;

/// A directory under the OS temp root, unique per process, test name and
/// thread, removed on drop.
pub(crate) struct ScratchDir(PathBuf);

impl ScratchDir {
    pub(crate) fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "inca-host-test-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Deref for ScratchDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The sorted names of the entries of `dir`.
pub(crate) fn names_of(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// `count` log names of the year 2000, the oldest first.
pub(crate) fn old_names(count: usize) -> Vec<String> {
    (1..=count)
        .map(|month| format!("2000{month:02}01T000000Z.log"))
        .collect()
}

/// The entry path the tests evaluate `source` under. Never read from disk in a test that imports
/// nothing else, since [`Session::load`] only reads a sibling file when a `source` actually
/// imports one.
pub(crate) const TEST_ENTRY_PATH: &str = "/test/entry.js";

pub(crate) fn load(source: &str) -> Result<Session, EngineError> {
    Session::load(TEST_ENTRY_PATH, source, stderr_reporter())
}

/// Captures every line written to it, so a test can read back what was sent.
#[derive(Default)]
pub(crate) struct CapturingWriter(pub(crate) RefCell<Vec<String>>);

impl Writer for CapturingWriter {
    fn write_line(&self, line: String) {
        self.0.borrow_mut().push(line);
    }
}

pub(crate) const RECEIVES_DEV_EVENTS: &str = r"
    globalThis.received = [];
    __inca_dev__.receive = (method, paramsJson) => {
        globalThis.received.push([method, paramsJson]);
    };
";

/// Writes `source` as the entry of `dir`, which then holds no config of its
/// own, and returns the entry path.
pub(crate) fn write_entry(dir: &ScratchDir, source: &str) -> String {
    let path = dir.join("bundle.js");
    std::fs::write(&path, source).unwrap();
    path.to_str().unwrap().to_owned()
}

pub(crate) fn mounts_a_div(width: u32, height: u32) -> String {
    format!(
        r"
            const node = __inca_native__.createNode('div');
            __inca_native__.appendChild(__inca_native__.rootNodeId(), node);
            __inca_native__.setStyle(node, 'width', {width});
            __inca_native__.setStyle(node, 'height', {height});
        "
    )
}

pub(crate) fn bounds_of(cx: &mut TestAppContext, window: WindowHandle<HostedApp>) -> (f32, f32) {
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
pub(crate) fn line(message: &Value) -> String {
    let mut message = message.clone();
    message["jsonrpc"] = json!("2.0");
    message.to_string()
}

pub(crate) fn notify(sender: &async_channel::Sender<String>, method: &str) {
    sender
        .send_blocking(line(&json!({ "method": method })))
        .unwrap();
}

pub(crate) fn request(sender: &async_channel::Sender<String>, id: u32, method: &str) {
    sender
        .send_blocking(line(&json!({ "id": id, "method": method })))
        .unwrap();
}

/// Every line written so far, decoded.
pub(crate) fn written(capturing: &CapturingWriter) -> Vec<Value> {
    capturing
        .0
        .borrow()
        .iter()
        .map(|sent| serde_json::from_str(sent).unwrap())
        .collect()
}

pub(crate) fn methods(capturing: &CapturingWriter) -> Vec<String> {
    written(capturing)
        .iter()
        .map(|sent| sent["method"].as_str().unwrap_or("").to_owned())
        .collect()
}

pub(crate) fn the_window(cx: &mut TestAppContext) -> Option<WindowHandle<HostedApp>> {
    cx.update(|cx| {
        cx.windows()
            .first()
            .and_then(gpui::AnyWindowHandle::downcast::<HostedApp>)
    })
}

/// Evaluates `script` in the session the window shows.
pub(crate) fn eval_in_window(
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
pub(crate) fn serve_started(
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

pub(crate) fn serve(
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
pub(crate) fn serve_on_executor_clock(
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
pub(crate) fn serve_idle_on_executor_clock(
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
pub(crate) fn serve_idle(
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

pub(crate) fn capture() -> (Rc<CapturingWriter>, SharedWriter) {
    let capturing = Rc::new(CapturingWriter::default());
    let writer: SharedWriter = Rc::clone(&capturing) as SharedWriter;
    (capturing, writer)
}

/// An entry no notification settles.
pub(crate) const NEVER_SETTLES: &str = "await new Promise(() => {});";

pub(crate) fn ready_message() -> Value {
    json!({ "jsonrpc": "2.0", "method": "ready", "params": { "protocol": 0 } })
}

pub(crate) fn eval_f64(h: &Harness, code: &str) -> f64 {
    h.window
        .read_with(&h.cx, |app, _| {
            app.session.engine.eval::<f64>(code).unwrap()
        })
        .unwrap()
}

pub(crate) fn id_where(h: &mut Harness, f: impl Fn(&Node) -> bool) -> NodeId {
    h.snapshot().find(&f).unwrap().id
}

/// Defines `listen(node, type, key)`, which registers the function `cb[key]` under the callback
/// id the host returns.
pub(crate) const LISTEN: &str = r"
        globalThis.cb = {};
        globalThis.listen = (node, type, key) => {
            if (typeof globalThis.cb[key] !== 'function') {
                throw new Error('cb[' + key + '] is not a function');
            }
            const id = __inca_native__.addEventListener(node, type, false, false);
            globalThis.__inca_callbacks__[id] = globalThis.cb[key];
        };
    ";

pub(crate) fn load_listening(cx: &mut TestAppContext, source: &str) -> Harness {
    Harness::load(cx, TEST_ENTRY_PATH, &format!("{LISTEN}{source}"))
}

pub(crate) fn log(h: &Harness) -> String {
    h.window
        .read_with(&h.cx, |app, _| {
            app.session
                .engine
                .eval::<String>("globalThis.log.join(',')")
                .unwrap()
        })
        .unwrap()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn node(h: &Harness, name: &str) -> NodeId {
    eval_f64(h, &format!("globalThis.{name}")) as NodeId
}
