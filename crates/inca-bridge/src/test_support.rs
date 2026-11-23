// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

//! Shared setup for tests: an engine wired to a [`Host`] through the bindings,
//! with `__inca_test_listen__` for registering listeners, and a [`TestView`] that stands in for
//! the window.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use std::collections::HashMap;

use inca_document::geometry::Align;
use inca_document::{EventListeners, NodeBox, ScrollRange, View};
use inca_jsenv::{
    Clock, Engine, EngineError, ErrorReporter, run_timers, stderr_reporter, system_clock,
};
use inca_node::{NodeId, VirtualTree};

use crate::Host;
use crate::bindings::install;
use inca_document::EventTargetId;

/// The HTML namespace.
pub const HTML: &str = "http://www.w3.org/1999/xhtml";

/// The SVG namespace.
pub const SVG: &str = "http://www.w3.org/2000/svg";

/// The `MathML` namespace.
pub const MATHML: &str = "http://www.w3.org/1998/Math/MathML";

// Snapshot per call: [type, bubbles, cancelable, composed, defaultPrevented
// before, defaultPrevented after preventDefault(), eventPhase,
// currentTarget, isTrusted, timeStamp].
pub const RECORDER: &str = r"
    globalThis.seen = [];
    globalThis.record = (e) => {
        const before = e.defaultPrevented; e.preventDefault();
        globalThis.seen.push([e.type, e.bubbles, e.cancelable, e.composed,
            before, e.defaultPrevented, e.eventPhase, e.currentTarget,
            e.isTrusted, e.timeStamp]); };
    ";

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
pub fn engine_with_bindings() -> (Engine, Rc<Host>) {
    let engine = Engine::new().unwrap();
    let host = Rc::new(Host::default());
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    (engine, host)
}

/// [`engine_with_bindings`] with the [`TestView`] that stands in for the host's window.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
#[cfg(test)]
pub(crate) fn engine_with_view() -> (Engine, Rc<Host>, Rc<RefCell<TestView>>) {
    let engine = Engine::new().unwrap();
    let (host, view) = Host::with_test_view(system_clock());
    let host = Rc::new(host);
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    (engine, host, view)
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
pub fn member_json(code: &str) -> (serde_json::Value, Rc<Host>) {
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
    pub host: Rc<Host>,
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
        let host = Rc::new(Host::with_clock(clock));
        let engine = engine_for(&host);
        engine.eval::<()>("globalThis.log = [];").unwrap();
        Self { engine, host, now }
    }

    /// Moves the clock by `ms` and runs the timers that are due.
    pub fn advance(&self, ms: u64) {
        self.now.set(self.now.get() + Duration::from_millis(ms));
        let timers = Rc::clone(&self.host.js.borrow().timers);
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
pub fn listen_ahead(engine: &Engine, host: &Rc<Host>, signal: u32, js: &str) {
    let callback = engine.with(|ctx| {
        let function: rquickjs::Value = ctx.eval(js).unwrap();
        inca_jsenv::callbacks::store(&ctx, function)
            .unwrap()
            .unwrap()
    });
    let added = host.document.borrow_mut().add_plain_listener(
        EventTargetId::Signal(signal),
        "abort",
        callback,
    );
    assert!(
        added.pin.is_empty(),
        "a signal that does not exist yet pins nothing"
    );
}

/// An engine with the bindings installed on `host` and [`LISTEN_JS`] evaluated.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
pub fn engine_for(host: &Rc<Host>) -> Rc<Engine> {
    engine_on_clock(host, system_clock())
}

/// [`engine_for`] with an engine whose `performance.now()` reads `clock`.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
pub fn engine_on_clock(host: &Rc<Host>, clock: Clock) -> Rc<Engine> {
    let engine = Rc::new(Engine::builder().clock(clock).build().unwrap());
    engine.with(|ctx| install(&ctx, host)).unwrap();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine
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

/// A window that draws nothing. It keeps the boxes and the scroll positions a test sets.
#[derive(Default)]
pub struct TestView {
    /// The box of each node the last frame drew.
    pub boxes: HashMap<NodeId, NodeBox>,
    /// The offset each scroll write left on each container, as `(left, top)`, unclamped.
    pub offsets: RefCell<HashMap<NodeId, (f32, f32)>>,
    /// The largest offset of each container, as `(left, top)`. A container without one has `0`.
    pub max: HashMap<NodeId, (f32, f32)>,
    /// The mouse buttons held, as the `buttons` bitmask of the events.
    pub held: u8,
}

impl View for TestView {
    fn node_box(&self, id: NodeId) -> Option<NodeBox> {
        self.boxes.get(&id).copied()
    }

    fn scroll_range(&self, id: NodeId) -> ScrollRange {
        let max = self.max.get(&id).copied().unwrap_or_default();
        let (left, top) = self.offsets.borrow().get(&id).copied().unwrap_or_default();
        ScrollRange {
            offset: (left.min(max.0), top.min(max.1)),
            max,
        }
    }

    fn scroll_to(&self, _tree: &VirtualTree, id: NodeId, left: Option<f64>, top: Option<f64>) {
        let mut offsets = self.offsets.borrow_mut();
        let at = offsets.entry(id).or_default();
        #[allow(clippy::cast_possible_truncation)]
        let clamped = |v: f64| v.max(0.0) as f32;
        if let Some(left) = left {
            at.0 = clamped(left);
        }
        if let Some(top) = top {
            at.1 = clamped(top);
        }
    }

    fn reveal(
        &self,
        _tree: &VirtualTree,
        _style: &inca_style::StyleCache,
        node: NodeId,
        _block: Align,
        _inline: Align,
    ) -> bool {
        self.boxes.contains_key(&node)
    }

    fn held_buttons(&self) -> u8 {
        self.held
    }

    fn reset_scroll(&self, tree: &VirtualTree, id: NodeId) {
        let mut offsets = self.offsets.borrow_mut();
        let mut pending = vec![id];
        while let Some(node) = pending.pop() {
            if let Some(offset) = offsets.get_mut(&node) {
                *offset = (0.0, 0.0);
            }
            if let Some(node) = tree.get(node) {
                pending.extend(node.children().iter().copied());
            }
        }
    }

    fn forget(&mut self, ids: &[NodeId]) {
        for id in ids {
            self.boxes.remove(id);
            self.offsets.get_mut().remove(id);
            self.max.remove(id);
        }
    }
}
