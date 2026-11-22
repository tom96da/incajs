// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

//! Setup for the tests where a JS engine and a window meet: an engine bound to a host through the
//! bindings, a dispatcher over the cells the host shares with it, and a GPUI root that renders the
//! tree through that dispatcher.

use std::ops::Deref;
use std::rc::Rc;

use gpui::TestAppContext;
use inca_bridge::test_support::{Reported, json_of};
pub use inca_bridge::test_support::{capture_reporter, listen_each};
use inca_bridge::{Host, JsScript};
use inca_document::EventPayload;
use inca_jsenv::{Clock, Engine, ErrorReporter};
use inca_node::NodeId;
use inca_window::test_support::TestRoot;
use inca_window::{Cells, EventDispatcher};

use crate::script::{BridgeScript, compose};

/// A dispatcher whose script is an engine, with the engine and the host its bindings write to.
/// The dispatcher's methods are reached through `Deref`.
#[derive(Clone)]
pub struct Wired {
    dispatcher: EventDispatcher,
    /// The engine the dispatcher runs listeners in.
    pub engine: Rc<Engine>,
    /// The host the bindings of the engine write to.
    pub host: Rc<Host>,
}

impl Deref for Wired {
    type Target = EventDispatcher;

    fn deref(&self) -> &EventDispatcher {
        &self.dispatcher
    }
}

impl Wired {
    /// A dispatcher that runs listeners in `engine`, reporting failures to `reporter`.
    #[must_use]
    pub fn new(
        engine: Rc<Engine>,
        host: Rc<Host>,
        cells: Rc<Cells>,
        reporter: ErrorReporter,
    ) -> Self {
        let script = JsScript::new(Rc::clone(&engine), Rc::clone(&host), reporter);
        Self {
            dispatcher: EventDispatcher::new(Rc::new(BridgeScript(script)), cells),
            engine,
            host,
        }
    }

    /// The dispatcher.
    #[must_use]
    pub fn dispatcher(&self) -> &EventDispatcher {
        &self.dispatcher
    }
}

/// A fresh host, its cells, an engine with `globalThis.out = []` and a dispatcher that collects
/// its failures in the returned list.
#[must_use]
pub fn dispatcher_with_engine() -> (Wired, Rc<Cells>, Reported) {
    dispatcher_on_clock(inca_jsenv::system_clock())
}

/// [`dispatcher_with_engine`] on an engine whose clock is `clock`.
///
/// # Panics
///
/// Panics when the test setup fails.
#[must_use]
pub fn dispatcher_on_clock(clock: Clock) -> (Wired, Rc<Cells>, Reported) {
    let (host, cells) = compose(clock.clone());
    let rig = Rig {
        host: Rc::clone(&host),
        cells: Rc::clone(&cells),
    };
    let engine = engine_on_clock(&rig, clock);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    let (report, reported) = capture_reporter();
    let wired = Wired::new(engine, host, Rc::clone(&cells), report);
    (wired, cells, reported)
}

/// A rig whose timers read the executor clock of `cx`, so `cx.executor().advance_clock(..)`
/// moves them.
#[must_use]
pub fn host_on_executor_clock(cx: &TestAppContext) -> Rig {
    let executor = cx.executor();
    Rig::on_clock(Rc::new(move || executor.now()))
}

/// An engine with the bindings installed on the host of `rig` and `__inca_test_listen__` defined.
#[must_use]
pub fn engine_for(rig: &Rig) -> Rc<Engine> {
    inca_bridge::test_support::engine_for(&rig.host)
}

/// [`engine_for`] on an engine whose clock is `clock`.
#[must_use]
pub fn engine_on_clock(rig: &Rig, clock: Clock) -> Rc<Engine> {
    inca_bridge::test_support::engine_on_clock(&rig.host, clock)
}

/// A window of [`TestRoot`] over `node` that dispatches into `engine`, with the first frame drawn.
pub fn mount_root(
    cx: &mut TestAppContext,
    rig: &Rig,
    engine: &Rc<Engine>,
    node: NodeId,
) -> gpui::WindowHandle<TestRoot> {
    let (report, _reported) = capture_reporter();
    let wired = rig.dispatcher(engine, report);
    inca_window::test_support::mount_root(cx, wired.dispatcher(), node)
}

/// [`mount_root`] followed by activating the window.
pub fn mount_active(
    cx: &mut TestAppContext,
    rig: &Rig,
    engine: &Rc<Engine>,
    node: NodeId,
) -> gpui::WindowHandle<TestRoot> {
    let (report, _reported) = capture_reporter();
    let wired = rig.dispatcher(engine, report);
    inca_window::test_support::mount_active(cx, wired.dispatcher(), node)
}

/// [`mount_root`] over the root of the document, with `reporter` and a running timer pump.
///
/// # Panics
///
/// Panics when the test setup fails.
pub fn mount_with_timers(
    cx: &mut TestAppContext,
    rig: &Rig,
    engine: &Rc<Engine>,
    reporter: ErrorReporter,
) -> gpui::WindowHandle<TestRoot> {
    let node = rig.cells.document.borrow().root();
    let wired = rig.dispatcher(engine, reporter);
    let window = inca_window::test_support::mount_root(cx, wired.dispatcher(), node);
    window
        .update(cx, |root, window, cx| {
            root.dispatcher.start_timers(window.window_handle(), cx);
        })
        .unwrap();
    window
}

/// A rig whose root holds `parent` over `children` nodes, each with `tabindex="0"`. The parent
/// takes focus as well when `parent_focusable` is set.
///
/// # Panics
///
/// Panics when the test setup fails.
#[must_use]
pub fn focusable_tree(children: usize, parent_focusable: bool) -> (Rig, NodeId, Vec<NodeId>) {
    let rig = Rig::default();
    let mut document = rig.cells.document.borrow_mut();
    let parent = document.tree.create_node("div").unwrap();
    let root = document.root();
    document.tree.append_child(root, parent).unwrap();
    let nodes: Vec<NodeId> = (0..children)
        .map(|_| {
            let child = document.tree.create_node("div").unwrap();
            document.tree.append_child(parent, child).unwrap();
            child
        })
        .collect();
    let focusable = parent_focusable
        .then_some(parent)
        .into_iter()
        .chain(nodes.iter().copied());
    for node in focusable {
        document.tree.set_attribute(node, "tabindex", "0").unwrap();
        document.focus.mark_tab_dirty(node);
    }
    drop(document);
    (rig.clone(), parent, nodes)
}

/// Registers the JS function expression `f` on `node` as a bubble-phase, active listener.
pub fn listen(wired: &Wired, node: NodeId, event: &str, f: &str) {
    listen_with(wired, node, event, (false, false), f);
}

/// Registers `f` on `node` with `(capture, passive)`.
///
/// # Panics
///
/// Panics when the test setup fails.
pub fn listen_with(
    wired: &Wired,
    node: NodeId,
    event: &str,
    (capture, passive): (bool, bool),
    f: &str,
) {
    wired
        .engine
        .eval::<()>(&format!(
            "__inca_test_listen__({node}, '{event}', {f}, {capture}, {passive});"
        ))
        .unwrap();
}

/// Registers `f` on the global target `tag`, `"$window"`, in the capture phase when `capture` is
/// set.
///
/// # Panics
///
/// Panics when the test setup fails.
pub fn listen_global(wired: &Wired, tag: &str, event: &str, capture: bool, f: &str) {
    wired
        .engine
        .eval::<()>(&format!(
            "__inca_test_listen__('{tag}', '{event}', {f}, {capture}, false);"
        ))
        .unwrap();
}

/// Returns `globalThis.out` as JSON.
#[must_use]
pub fn read_out(wired: &Wired) -> serde_json::Value {
    json_of(&wired.engine, "globalThis.out")
}

/// `js` evaluated in the engine of `wired` and read back as JSON.
#[must_use]
pub fn eval_json(wired: &Wired, js: &str) -> serde_json::Value {
    json_of(&wired.engine, js)
}

/// Fires `event` with `payload` at `start` once and returns `globalThis.out`.
pub fn fire_and_read(
    cx: &mut TestAppContext,
    wired: &Wired,
    start: NodeId,
    event: &str,
    payload: &EventPayload,
) -> serde_json::Value {
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        wired.fire(start, event, payload, window, cx);
    });
    read_out(wired)
}

/// Whether the callback table of `engine` holds a value under `id`.
///
/// # Panics
///
/// Panics when the test setup fails.
#[must_use]
pub fn callback_stored(engine: &Engine, id: u32) -> bool {
    engine.with(|ctx| inca_jsenv::callbacks::get(&ctx, id).unwrap().is_some())
}

/// A listener that pushes `[name, currentTarget, eventPhase]`.
#[must_use]
pub fn tagged(name: &str) -> String {
    format!("(e) => out.push(['{name}', e.currentTarget, e.eventPhase])")
}

/// A host of the bindings with the cells its dispatcher shares. The host is reached through
/// `Deref`.
#[derive(Clone)]
pub struct Rig {
    /// The host the bindings of an engine write to.
    pub host: Rc<Host>,
    /// The document and window cells the host shares with a dispatcher.
    pub cells: Rc<Cells>,
}

impl Default for Rig {
    fn default() -> Self {
        Self::on_clock(inca_jsenv::system_clock())
    }
}

impl Deref for Rig {
    type Target = Host;

    fn deref(&self) -> &Host {
        &self.host
    }
}

impl Rig {
    /// A rig whose timers read `clock`.
    #[must_use]
    pub fn on_clock(clock: Clock) -> Self {
        let (host, cells) = compose(clock);
        Self { host, cells }
    }

    /// An engine with the bindings installed on the host and `__inca_test_listen__` defined.
    #[must_use]
    pub fn engine(&self) -> Rc<Engine> {
        engine_for(self)
    }

    /// A dispatcher over the cells that runs listeners in `engine` and reports failures to
    /// `reporter`.
    #[must_use]
    pub fn dispatcher(&self, engine: &Rc<Engine>, reporter: ErrorReporter) -> Wired {
        Wired::new(
            Rc::clone(engine),
            Rc::clone(&self.host),
            Rc::clone(&self.cells),
            reporter,
        )
    }
}
