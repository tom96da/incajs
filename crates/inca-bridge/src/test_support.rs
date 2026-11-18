// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

//! Shared setup for tests: an engine wired to a [`Host`] through the bindings,
//! with `__inca_test_listen__` for registering listeners, and a GPUI root that
//! renders the host's tree.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{Context, Render, TestAppContext, Window, WindowHandle, prelude::*};
use inca_gpui::{Scrollbars, render_tree_with_events};
use inca_jsenv::{
    Clock, Engine, EngineError, ErrorReporter, run_timers, stderr_reporter, system_clock,
};
use inca_node::NodeId;

use crate::bindings::install;
use crate::{EventDispatcher, EventListeners, EventTargetId, Host};

/// The HTML namespace.
pub const HTML: &str = "http://www.w3.org/1999/xhtml";

/// The SVG namespace.
pub const SVG: &str = "http://www.w3.org/2000/svg";

/// The `MathML` namespace.
pub const MATHML: &str = "http://www.w3.org/1998/Math/MathML";

/// Defines `__inca_test_listen__(target, type, fn, capture, passive)`, which
/// registers `fn` through `__inca_native__.addEventListener`.
pub const LISTEN_JS: &str = r"
    globalThis.__inca_test_listen__ = (target, type, fn, capture = false, passive = false) => {
        __inca_native__.addEventListener(target, type, fn, capture, passive);
    };
    ";

/// The callback ids of `(target, event)` in registration order.
#[must_use]
pub fn callback_ids(listeners: &EventListeners, target: EventTargetId, event: &str) -> Vec<u32> {
    listeners
        .listeners_for(target, event)
        .iter()
        .map(|l| l.callback)
        .collect()
}

/// An engine with the bindings installed on a fresh default [`Host`], returned
/// together with that host.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
#[must_use]
pub fn engine_with_bindings() -> (Engine, Rc<RefCell<Host>>) {
    let engine = Engine::new().unwrap();
    let host = Rc::new(RefCell::new(Host::default()));
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    (engine, host)
}

/// The `name` of what `call` throws, or `'none'`.
///
/// # Panics
///
/// Panics when the engine fails to evaluate the call.
#[must_use]
pub fn error_name(engine: &Engine, call: &str) -> String {
    let js = r"
        (() => {
            try {
                CALL;
                return 'none';
            } catch (e) {
                return e.name;
            }
        })()
        ";
    engine.eval(&js.replace("CALL", call)).unwrap()
}

/// `expr` evaluated in `engine` and read back as JSON.
///
/// # Panics
///
/// Panics when the engine fails to evaluate `expr` or the value has no JSON form.
#[must_use]
pub fn json_of(engine: &Engine, expr: &str) -> serde_json::Value {
    let text: String = engine.eval(&format!("JSON.stringify({expr})")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The helpers of a member test: `n` is the native object, `h(id)` the handle of a node, `mk(tag)`
/// a handle of a new element under the root, `loose(tag)` one with no parent, `txt(data)` a text
/// node under the root and `free(handle)` frees the node of a handle.
const MEMBER_PRELUDE: &str = r"
    const n = __inca_native__;
    const h = (id) => n.nodeHandle(id)[0];
    const mk = (tag) => {
        const id = n.createNode(tag);
        n.appendChild(n.rootNodeId(), id);
        return h(id);
    };
    const loose = (tag) => h(n.createNode(tag));
    const txt = (data) => {
        const id = n.createNode('#text');
        n.setCharacterData(id, data);
        n.appendChild(n.rootNodeId(), id);
        return h(id);
    };
    const free = (handle) => n.destroyNode(handle.__node_id);
    ";

/// The body `code` of a member test, which `return`s a value, run over the helpers of
/// [`MEMBER_PRELUDE`] in a new engine, and the value read back as JSON.
///
/// # Panics
///
/// Panics when the body throws or its value has no JSON form.
#[must_use]
pub fn member_json(code: &str) -> (serde_json::Value, Rc<RefCell<Host>>) {
    let (engine, host) = engine_with_bindings();
    let text: String = engine
        .eval(&format!(
            r"
            (() => {{
                {MEMBER_PRELUDE}
                return JSON.stringify((() => {{
                    {code}
                }})());
            }})()
            "
        ))
        .unwrap();
    (serde_json::from_str(&text).unwrap(), host)
}

/// The `name` of what the body `code` of a member test throws, or `'none'`.
///
/// # Panics
///
/// Panics when the engine fails to evaluate the body.
#[must_use]
pub fn member_error(code: &str) -> String {
    let (engine, _host) = engine_with_bindings();
    engine
        .eval(&format!(
            r"
            (() => {{
                {MEMBER_PRELUDE}
                try {{
                    (() => {{
                        {code}
                    }})();
                    return 'none';
                }} catch (e) {{
                    return e.name;
                }}
            }})()
            "
        ))
        .unwrap()
}

/// An engine over a host whose clock the test moves.
pub struct Clocked {
    /// The engine, with the bindings and an empty global `log` array.
    pub engine: Rc<Engine>,
    /// The host the engine is bound to.
    pub host: Rc<RefCell<Host>>,
    now: Rc<Cell<Instant>>,
}

impl Clocked {
    /// A new engine and host on a clock that stands still.
    ///
    /// # Panics
    ///
    /// Panics when the engine fails to initialise.
    #[must_use]
    pub fn new() -> Self {
        let now = Rc::new(Cell::new(Instant::now()));
        let clock = {
            let now = Rc::clone(&now);
            Rc::new(move || now.get())
        };
        let host = Rc::new(RefCell::new(Host::with_clock(clock)));
        let engine = engine_for(&host);
        engine.eval::<()>("globalThis.log = [];").unwrap();
        Self { engine, host, now }
    }

    /// Moves the clock by `ms` and runs the timers that are due.
    pub fn advance(&self, ms: u64) {
        self.now.set(self.now.get() + Duration::from_millis(ms));
        let timers = Rc::clone(&self.host.borrow().timers);
        run_timers(&self.engine, &timers, &stderr_reporter());
    }

    /// `expr` evaluated and read back as JSON.
    #[must_use]
    pub fn json(&self, expr: &str) -> serde_json::Value {
        json_of(&self.engine, expr)
    }
}

impl Default for Clocked {
    fn default() -> Self {
        Self::new()
    }
}

/// Registers the function that `js` evaluates to as an `abort` listener of the signal that will
/// get the id `signal`, before the signal exists. The listener is never removed.
///
/// # Panics
///
/// Panics when the engine fails to evaluate `js` or store it.
pub fn listen_ahead(engine: &Engine, host: &Rc<RefCell<Host>>, signal: u32, js: &str) {
    let callback = engine.with(|ctx| {
        let function: rquickjs::Value = ctx.eval(js).unwrap();
        inca_jsenv::callbacks::store(&ctx, function)
            .unwrap()
            .unwrap()
    });
    host.borrow_mut().listeners.insert(
        EventTargetId::Signal(signal),
        "abort",
        callback,
        false,
        false,
    );
}

/// An engine with the bindings installed on `host` and [`LISTEN_JS`] evaluated.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
pub fn engine_for(host: &Rc<RefCell<Host>>) -> Rc<Engine> {
    engine_on_clock(host, system_clock())
}

/// [`engine_for`] with an engine whose `performance.now()` reads `clock`.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
pub(crate) fn engine_on_clock(host: &Rc<RefCell<Host>>, clock: Clock) -> Rc<Engine> {
    let engine = Rc::new(Engine::builder().clock(clock).build().unwrap());
    engine.with(|ctx| install(&ctx, host)).unwrap();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine
}

/// A window root that renders the subtree at `node` of `host` through
/// `dispatcher`, calling [`EventDispatcher::begin_frame`] on every frame.
pub struct TestRoot {
    /// The host whose tree is rendered.
    pub host: Rc<RefCell<Host>>,
    /// The node the rendering starts at.
    pub node: NodeId,
    /// The dispatcher the rendered elements report to.
    pub dispatcher: EventDispatcher,
}

impl Render for TestRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dispatcher.begin_frame(window, cx);
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher, Scrollbars::Overlay)
            .unwrap()
    }
}

/// Opens a window of [`TestRoot`] over `node` of `host`, dispatching into
/// `engine`, and draws the first frame.
///
/// # Panics
///
/// Panics when the window cannot be updated.
pub fn mount_root(
    cx: &mut TestAppContext,
    host: &Rc<RefCell<Host>>,
    engine: &Rc<Engine>,
    node: NodeId,
) -> WindowHandle<TestRoot> {
    let dispatcher = EventDispatcher::new(Rc::clone(engine), Rc::clone(host));
    let window = cx.add_window(|_, _| TestRoot {
        host: Rc::clone(host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    window
}

/// [`mount_root`] followed by activating the window.
///
/// # Panics
///
/// Panics when the window cannot be updated.
pub fn mount_active(
    cx: &mut TestAppContext,
    host: &Rc<RefCell<Host>>,
    engine: &Rc<Engine>,
    node: NodeId,
) -> WindowHandle<TestRoot> {
    let window = mount_root(cx, host, engine, node);
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();
    window
}

/// A host whose timers read the executor clock of `cx`, so
/// `cx.executor().advance_clock(..)` moves them.
#[must_use]
pub fn host_on_executor_clock(cx: &TestAppContext) -> Rc<RefCell<Host>> {
    let executor = cx.executor();
    Rc::new(RefCell::new(Host::with_clock(Rc::new(move || {
        executor.now()
    }))))
}

/// [`mount_root`] over the root of `host`, with `reporter` and a running timer pump.
///
/// # Panics
///
/// Panics when the window cannot be updated.
pub fn mount_with_timers(
    cx: &mut TestAppContext,
    host: &Rc<RefCell<Host>>,
    engine: &Rc<Engine>,
    reporter: ErrorReporter,
) -> WindowHandle<TestRoot> {
    let node = host.borrow().root();
    let dispatcher =
        EventDispatcher::new(Rc::clone(engine), Rc::clone(host)).with_reporter(reporter);
    let window = cx.add_window(|_, _| TestRoot {
        host: Rc::clone(host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    window
        .update(cx, |root, window, cx| {
            root.dispatcher.start_timers(window.window_handle(), cx);
        })
        .unwrap();
    window
}

/// A host whose tree holds `parent` over `children` nodes, each with
/// `tabindex="0"`. The parent takes focus as well when `parent_focusable` is
/// set.
///
/// # Panics
///
/// Panics when the tree rejects a node.
#[must_use]
pub fn focusable_tree(
    children: usize,
    parent_focusable: bool,
) -> (Rc<RefCell<Host>>, NodeId, Vec<NodeId>) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let parent = host_mut.tree.create_node("div").unwrap();
    let root = host_mut.root();
    host_mut.tree.append_child(root, parent).unwrap();
    let nodes: Vec<NodeId> = (0..children)
        .map(|_| {
            let child = host_mut.tree.create_node("div").unwrap();
            host_mut.tree.append_child(parent, child).unwrap();
            child
        })
        .collect();
    let focusable = parent_focusable
        .then_some(parent)
        .into_iter()
        .chain(nodes.iter().copied());
    for node in focusable {
        host_mut.tree.set_attribute(node, "tabindex", "0").unwrap();
        host_mut.focus.mark_tab_dirty(node);
    }
    drop(host_mut);
    (host, parent, nodes)
}

/// Registers `callback`, a JS function expression, for each of `events` on
/// each of `nodes`.
///
/// # Panics
///
/// Panics when the script fails.
pub fn listen_each(
    engine: &Engine,
    nodes: &[NodeId],
    events: &[&str],
    callback: &str,
    passive: bool,
) {
    for node in nodes {
        for event in events {
            engine
                .eval::<()>(&format!(
                    "__inca_test_listen__({node}, '{event}', {callback}, false, {passive});"
                ))
                .unwrap();
        }
    }
}

/// Every failure a test reporter received, in order.
pub type Reported = Rc<RefCell<Vec<EngineError>>>;

/// A reporter that appends every failure to the returned list.
#[must_use]
pub fn capture_reporter() -> (ErrorReporter, Reported) {
    let reported: Reported = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&reported);
    (
        Rc::new(move |err: &EngineError| sink.borrow_mut().push(err.clone())),
        reported,
    )
}
