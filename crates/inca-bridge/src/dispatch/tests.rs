// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::event::Flags;
use crate::test_support::{Reported, capture_reporter, engine_for};
use gpui::{TestAppContext, point, px};
use inca_gpui::event_sink::EVENTS;
use inca_gpui::{EventSink, WheelPayload};

mod base_fields;
mod click;
mod dispatch_event;
mod engine_entry;
mod frame;
mod jobs;
mod movement;
mod target_objects;
mod targets;
mod walk;

fn dispatcher_with_engine() -> (EventDispatcher, Rc<RefCell<Host>>, Reported) {
    let host = Rc::new(RefCell::new(Host::default()));
    let engine = engine_for(&host);
    engine.eval::<()>("globalThis.out = [];").unwrap();

    let (report, reported) = capture_reporter();
    let dispatcher = EventDispatcher::new(engine, Rc::clone(&host)).with_reporter(report);

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
const RECORDER: &str = r"
    globalThis.seen = [];
    globalThis.record = (e) => {
        const before = e.defaultPrevented; e.preventDefault();
        globalThis.seen.push([e.type, e.bubbles, e.cancelable, e.composed,
            before, e.defaultPrevented, e.eventPhase, e.currentTarget,
            e.isTrusted, e.timeStamp]); };
    ";

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

/// Registers the JS function expression `f` on `node` as a bubble-phase, active listener.
fn listen(dispatcher: &EventDispatcher, node: NodeId, event: &str, f: &str) {
    listen_with(dispatcher, node, event, (false, false), f);
}

/// Registers `f` on `node` with `(capture, passive)`.
fn listen_with(
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

fn eval_json(dispatcher: &EventDispatcher, js: &str) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>(&format!("JSON.stringify({js})"))
        .unwrap();
    serde_json::from_str(&json).unwrap()
}
