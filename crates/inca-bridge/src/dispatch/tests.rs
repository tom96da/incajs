// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use crate::bindings::install;
use crate::event::Flags;
use gpui::{TestAppContext, point, px};
use inca_gpui::event_sink::EVENTS;
use inca_gpui::{EventSink, WheelPayload};
use inca_jsenv::EngineError;

mod base_fields;
mod click;
mod dispatch_event;
mod engine_entry;
mod frame;
mod jobs;
mod movement;
mod targets;
mod walk;

/// Every failure a test reporter received, in order.
type Reported = Rc<RefCell<Vec<EngineError>>>;

fn dispatcher_with_engine() -> (EventDispatcher, Rc<RefCell<Host>>, Reported) {
    let engine = Rc::new(Engine::new().unwrap());
    let host = Rc::new(RefCell::new(Host::default()));
    engine.with(|ctx| install(&ctx, &host)).unwrap();

    let reported: Reported = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&reported);
    let dispatcher = EventDispatcher::new(engine, Rc::clone(&host)).with_reporter(Rc::new(
        move |err: &EngineError| sink.borrow_mut().push(err.clone()),
    ));

    (dispatcher, host, reported)
}

fn at(x: f32, y: f32) -> gpui::Point<gpui::Pixels> {
    point(px(x), px(y))
}

fn mouse_at(x: f32, y: f32) -> EventPayload {
    EventPayload::Mouse(MousePayload::at(at(x, y), gpui::Modifiers::none()))
}

fn wheel_at(x: f32, y: f32) -> EventPayload {
    EventPayload::Wheel(WheelPayload {
        mouse: MousePayload::at(at(x, y), gpui::Modifiers::none()),
        delta_x: 0.0,
        delta_y: 1.0,
        delta_z: 0.0,
        delta_mode: 0,
    })
}

// Snapshot per call: [type, bubbles, cancelable, composed, defaultPrevented
// before, defaultPrevented after preventDefault(), eventPhase,
// currentTarget, isTrusted, timeStamp].
const RECORDER: &str = "globalThis.seen = []; \
    globalThis.__inca_callbacks__ = { 0: (e) => { \
        const before = e.defaultPrevented; e.preventDefault(); \
        globalThis.seen.push([e.type, e.bubbles, e.cancelable, e.composed, \
            before, e.defaultPrevented, e.eventPhase, e.currentTarget, \
            e.isTrusted, e.timeStamp]); } };";

fn seen(dispatcher: &EventDispatcher) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

fn seen_rows(dispatcher: &EventDispatcher) -> Vec<Vec<serde_json::Value>> {
    serde_json::from_value(seen(dispatcher)).unwrap()
}

fn listen(host: &Rc<RefCell<Host>>, node: NodeId, event: &str) {
    host.borrow_mut().listeners.register(node, event, 0);
}

fn tree_of_three(host: &Rc<RefCell<Host>>) -> [NodeId; 3] {
    let mut h = host.borrow_mut();
    let outer = h.tree.create_node("div").unwrap();
    let parent = h.tree.create_node("div").unwrap();
    let child = h.tree.create_node("div").unwrap();
    h.tree.append_child(outer, parent).unwrap();
    h.tree.append_child(parent, child).unwrap();
    [outer, parent, child]
}

/// Returns `globalThis.out` as JSON.
fn read_out(dispatcher: &EventDispatcher) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.out)")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// Fires `event` with `payload` at `start` once and returns `globalThis.out`.
fn fire_and_read(
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

fn callbacks(dispatcher: &EventDispatcher, js: &str) {
    dispatcher
        .engine
        .eval::<()>(&format!(
            "globalThis.out = []; globalThis.__inca_callbacks__ = {{ {js} }};"
        ))
        .unwrap();
}

fn eval_json(dispatcher: &EventDispatcher, js: &str) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>(&format!("JSON.stringify({js})"))
        .unwrap();
    serde_json::from_str(&json).unwrap()
}
