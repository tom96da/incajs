// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests of several modules.

#![allow(clippy::unwrap_used)]

use std::rc::Rc;

use gpui::TestAppContext;
use inca_document::{
    EventPayload, EventTargetId, MousePayload, PointerSource, Walk, WheelPayload, flags_of,
};
use inca_jsenv::Clock;
use inca_node::NodeId;

pub(crate) use crate::test_support::RECORDER;
use crate::test_support::{Reported, capture_reporter, engine_for, engine_on_clock};
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
    pub(crate) fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        _window: &mut gpui::Window,
        _cx: &mut gpui::App,
    ) -> bool {
        if self.host.document.borrow().tree.get(start).is_none() {
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
    cx: &mut TestAppContext,
    dispatcher: &EventDispatcher,
    start: NodeId,
    event: &str,
    payload: &EventPayload,
) -> serde_json::Value {
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(start, event, payload, window, cx);
    });
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
        let outer = h.tree.create_node("div").unwrap();
        let parent = h.tree.create_node("div").unwrap();
        let child = h.tree.create_node("div").unwrap();
        h.tree.append_child(outer, parent).unwrap();
        h.tree.append_child(parent, child).unwrap();
        [outer, parent, child]
    }

    /// A new node under the host root, so an event at it also reaches document and window.
    pub(crate) fn node_under_root(host: &Rc<Host>) -> NodeId {
        let mut h = host.document.borrow_mut();
        let node = h.tree.create_node("div").unwrap();
        let root = h.root();
        h.tree.append_child(root, node).unwrap();
        node
    }
}

pub(crate) use native::*;
