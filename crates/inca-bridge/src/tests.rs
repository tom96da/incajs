// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests of several modules.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use inca_document::NodeBox;
use inca_document::geometry::Rect;
use inca_document::{
    EventPayload, EventTargetId, MousePayload, PointerSource, Walk, WheelPayload, flags_of,
};
use inca_jsenv::{Clock, Engine};
use inca_node::NodeId;

pub(crate) use crate::test_support::RECORDER;
use crate::test_support::{
    Clocked, LISTEN_JS, Reported, TestView, capture_reporter, engine_for, engine_on_clock,
    engine_with_bindings, engine_with_view,
};
use crate::{Host, JsScript};

/// What a window does to deliver an event to the engine, without a window: the walk, then the
/// jobs and the failures it left. A test that fires an event through it reads what JS sees.
pub(crate) struct EventDispatcher {
    pub(crate) engine: Rc<inca_jsenv::Engine>,
    pub(crate) host: Rc<Host>,
    script: JsScript,
}

impl EventDispatcher {
    pub(crate) fn new(engine: Rc<inca_jsenv::Engine>, host: Rc<Host>) -> Self {
        let script = JsScript::new(
            Rc::clone(&engine),
            Rc::clone(&host),
            inca_jsenv::stderr_reporter(),
        );
        Self {
            engine,
            host,
            script,
        }
    }

    pub(crate) fn with_reporter(self, reporter: inca_jsenv::ErrorReporter) -> Self {
        self.script.set_reporter(reporter);
        self
    }

    pub(crate) fn report_pending(&self) {
        self.script.report_pending();
    }

    /// Fires `event` at `start`: the listeners run, then the job queue drains when one ran.
    /// Returns whether a listener prevented the default action. A `start` that left the tree
    /// fires nothing.
    pub(crate) fn fire(&self, start: NodeId, event: &str, payload: &EventPayload) -> bool {
        if self.host.document.borrow().tree().get(start).is_none() {
            return false;
        }
        let outcome = self.script.walk(&Walk {
            name: event,
            flags: flags_of(
                event,
                match payload {
                    EventPayload::Input(input) => Some(input.input_type),
                    _ => None,
                },
            ),
            target: EventTargetId::Node(start),
            cut: &[],
            payload,
            trusted: true,
        });
        self.script.report_pending();
        if outcome.entered {
            self.script.drain_jobs();
            self.script.report_pending();
        }
        outcome.canceled
    }
}

pub(crate) fn dispatcher_with_engine() -> (EventDispatcher, Rc<Host>, Reported) {
    let host = Rc::new(Host::default());
    let engine = engine_for(&host);
    engine.eval::<()>("globalThis.out = [];").unwrap();

    let (report, reported) = capture_reporter();
    let dispatcher = EventDispatcher::new(engine, Rc::clone(&host)).with_reporter(report);

    (dispatcher, host, reported)
}

/// [`dispatcher_with_engine`] on an engine whose clock is `clock`.
pub(crate) fn dispatcher_on_clock(clock: Clock) -> (EventDispatcher, Rc<Host>) {
    let host = Rc::new(Host::with_clock(clock.clone()));
    let engine = engine_on_clock(&host, clock);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    let dispatcher = EventDispatcher::new(engine, Rc::clone(&host));
    (dispatcher, host)
}

pub(crate) fn seen(dispatcher: &EventDispatcher) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

pub(crate) fn seen_rows(dispatcher: &EventDispatcher) -> Vec<Vec<serde_json::Value>> {
    serde_json::from_value(seen(dispatcher)).unwrap()
}

/// Registers the JS function expression `f` on `node` as a bubble-phase, active listener.
pub(crate) fn listen(dispatcher: &EventDispatcher, node: NodeId, event: &str, f: &str) {
    listen_with(dispatcher, node, event, (false, false), f);
}

/// Registers `f` on `node` with `(capture, passive)`.
pub(crate) fn listen_with(
    dispatcher: &EventDispatcher,
    node: NodeId,
    event: &str,
    (capture, passive): (bool, bool),
    f: &str,
) {
    dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_test_listen__({node}, '{event}', {f}, {capture}, {passive});"
        ))
        .unwrap();
}

/// Registers `f` on the global target `tag`, `"$window"`, in the capture phase
/// when `capture` is set.
pub(crate) fn listen_global(
    dispatcher: &EventDispatcher,
    tag: &str,
    event: &str,
    capture: bool,
    f: &str,
) {
    dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_test_listen__('{tag}', '{event}', {f}, {capture}, false);"
        ))
        .unwrap();
}

/// Returns `globalThis.out` as JSON.
pub(crate) fn read_out(dispatcher: &EventDispatcher) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.out)")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// Fires `event` with `payload` at `start` once and returns `globalThis.out`.
pub(crate) fn fire_and_read(
    dispatcher: &EventDispatcher,
    start: NodeId,
    event: &str,
    payload: &EventPayload,
) -> serde_json::Value {
    dispatcher.fire(start, event, payload);
    read_out(dispatcher)
}

/// Whether the callback table of `engine` holds a value under `id`.
pub(crate) fn callback_stored(engine: &inca_jsenv::Engine, id: u32) -> bool {
    engine.with(|ctx| inca_jsenv::callbacks::get(&ctx, id).unwrap().is_some())
}

pub(crate) fn eval_json(dispatcher: &EventDispatcher, js: &str) -> serde_json::Value {
    crate::test_support::json_of(&dispatcher.engine, js)
}

/// A listener that pushes `[name, currentTarget, eventPhase]`.
pub(crate) fn tagged(name: &str) -> String {
    format!("(e) => out.push(['{name}', e.currentTarget, e.eventPhase])")
}

/// Runs `body` as a function body next to the natives `n`, a new div `id` and its list `list`.
/// `class` is the class attribute of the div, absent for `None`. A `freed` div is destroyed after
/// the list is read from it. Returns what `body` returns, as JSON.
pub(crate) fn list_exec(class: Option<&str>, freed: bool, body: &str) -> serde_json::Value {
    let (engine, _host) = crate::test_support::engine_with_bindings();
    let class = serde_json::to_string(&class).unwrap();
    let destroy = if freed { "n.destroyNode(id);" } else { "" };
    let json: String = engine
        .eval(&format!(
            r"
            JSON.stringify((() => {{
                const n = __inca_native__;
                const id = n.createNode('div');
                const class_ = {class};
                if (class_ !== null) {{
                    n.setAttribute(id, 'class', class_);
                }}
                const list = n.classList(id);
                {destroy}
                {body}
            }})())
            "
        ))
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// [`list_exec`] on a div that is not freed.
pub(crate) fn list_run(class: Option<&str>, body: &str) -> serde_json::Value {
    list_exec(class, false, body)
}

/// The helpers that need no JS engine: trees, payloads, windows and fake-script scenes. They are
/// the part of this file that moves with the dispatch.
pub(crate) mod native {
    use super::*;

    pub(crate) fn mouse_at(x: f32, y: f32) -> EventPayload {
        EventPayload::Mouse(MousePayload::at(x, y, inca_document::Modifiers::none()))
    }

    /// A `click` from the mouse pointer.
    pub(crate) fn pointer_click() -> EventPayload {
        EventPayload::Mouse(MousePayload {
            pointer: Some(PointerSource::Mouse),
            ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
        })
    }

    pub(crate) fn wheel_at(x: f32, y: f32) -> EventPayload {
        EventPayload::Wheel(WheelPayload {
            mouse: MousePayload::at(x, y, inca_document::Modifiers::none()),
            delta_x: 0.0,
            delta_y: 1.0,
            delta_z: 0.0,
            delta_mode: 0,
        })
    }

    pub(crate) fn key_payload(modifiers: inca_document::Modifiers) -> EventPayload {
        EventPayload::Key(inca_document::KeyPayload {
            key: "a".into(),
            repeat: false,
            modifiers,
            is_composing: false,
        })
    }

    pub(crate) fn tree_of_three(host: &Rc<Host>) -> [NodeId; 3] {
        let mut h = host.document.borrow_mut();
        let outer = h.create_node("div").unwrap();
        let parent = h.create_node("div").unwrap();
        let child = h.create_node("div").unwrap();
        let view = host.window.borrow();
        h.append_child(&*view, outer, parent).unwrap();
        h.append_child(&*view, parent, child).unwrap();
        [outer, parent, child]
    }

    /// A new node under the host root, so an event at it also reaches document and window.
    pub(crate) fn node_under_root(host: &Rc<Host>) -> NodeId {
        let mut h = host.document.borrow_mut();
        let node = h.create_node("div").unwrap();
        let root = h.root();
        h.append_child(&*host.window.borrow(), root, node).unwrap();
        node
    }
}

pub(crate) use native::*;

/// Builds `p > (a, b)`, `q > x`, `g > (g1, g2)` and the parentless `loose`, `loose2`, `t`
/// (text), `c` (comment) and `f` (empty fragment).
const FAMILY_SETUP: &str = r"
    globalThis.n = __inca_native__;
    const make = (tag) => n.createNode(tag);
    globalThis.p = make('div');
    globalThis.a = make('span');
    globalThis.b = make('span');
    globalThis.q = make('div');
    globalThis.x = make('span');
    globalThis.g = make('#fragment');
    globalThis.g1 = make('i');
    globalThis.g2 = make('i');
    globalThis.loose = make('span');
    globalThis.loose2 = make('span');
    globalThis.t = make('#text');
    globalThis.c = make('#comment');
    globalThis.f = make('#fragment');
    n.appendChild(p, a);
    n.appendChild(p, b);
    n.appendChild(q, x);
    n.appendChild(g, g1);
    n.appendChild(g, g2);
    ";

pub(crate) fn engine_with_family() -> (Engine, Rc<Host>) {
    let (engine, host, _view) = engine_with_family_view();
    (engine, host)
}

pub(crate) fn engine_with_family_view() -> (Engine, Rc<Host>, Rc<RefCell<TestView>>) {
    let (engine, host, view) = engine_with_view();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine.eval::<()>(FAMILY_SETUP).unwrap();
    (engine, host, view)
}

/// [`engine_with_bindings`] after `LISTEN_JS` and `setup` ran.
pub(crate) fn engine_with_setup(setup: &str) -> (Engine, Rc<Host>) {
    let (engine, host) = engine_with_bindings();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine.eval::<()>(setup).unwrap();
    (engine, host)
}

/// An engine with the global `el`, a `div` under the root, whose last box is `b`
/// (`x`, `y`, `width`, `height`, `border`, `scroll x`, `scroll y`, `gutter w`, `gutter h`).
pub(crate) fn with_box(b: [f32; 9]) -> (Engine, Rc<Host>, u32) {
    let (engine, host, _view, id) = with_box_view(b);
    (engine, host, id)
}

/// [`with_box`] with the view that holds the box.
pub(crate) fn with_box_view(b: [f32; 9]) -> (Engine, Rc<Host>, Rc<RefCell<TestView>>, u32) {
    let (engine, host, view) = engine_with_view();
    let id: u32 = engine
        .eval(
            r"
            const n = __inca_native__;
            globalThis.id = n.createNode('div');
            n.appendChild(n.rootNodeId(), id);
            globalThis.el = n.nodeHandle(id)[0];
            n.setStyle(id, 'overflow', 'scroll');
            id;
            ",
        )
        .unwrap();
    record(&view, id, b);
    (engine, host, view, id)
}

pub(crate) fn record(view: &Rc<RefCell<TestView>>, id: u32, b: [f32; 9]) {
    let node_box = NodeBox {
        rect: Rect {
            x: b[0],
            y: b[1],
            width: b[2],
            height: b[3],
        },
        border: b[4],
        scroll: (b[5], b[6]),
        gutter: (b[7], b[8]),
    };
    view.borrow_mut().boxes.insert(id, node_box);
}

pub(crate) fn read(engine: &Engine, expr: &str) -> serde_json::Value {
    let text: String = engine.eval(&format!("JSON.stringify({expr})")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// A node, a controller `c` with its signal `s`, a log and `add(target, fn, signal, capture)`,
/// which registers a `click` listener with the signal option.
const SIGNAL_OPTION_SETUP: &str = r"
    globalThis.n = __inca_native__;
    globalThis.node = n.createNode('div');
    globalThis.c = new AbortController();
    globalThis.s = c.signal;
    globalThis.add = (target, fn, signal, capture = false) =>
        n.addEventListener(target, 'click', fn, capture, false, signal);
    ";

/// A [`Clocked`] engine after [`SIGNAL_OPTION_SETUP`].
pub(crate) fn clocked_with_signal_option() -> Clocked {
    let f = Clocked::new();
    f.engine.eval::<()>(SIGNAL_OPTION_SETUP).unwrap();
    f
}
