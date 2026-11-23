// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The JS recorders and the payloads of the event tests.

#![allow(clippy::unwrap_used, dead_code, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Keystroke, MouseButton, MouseUpEvent, TestAppContext, VisualTestContext,
    Window, WindowHandle, WindowId,
};
use inca_document::{EVENTS, PointerSource, WheelPayload};
use inca_document::{EventPayload, EventTargetId, KeyPayload, MousePayload};
use inca_gpui::{EventSink, InputLayout, InputRequest};
use inca_node::NodeId;
use inca_style::set_style;
use inca_window::{Cells, EventDispatcher};

use super::rig::{self as wiring, *};

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

pub fn seen(dispatcher: &Wired) -> serde_json::Value {
    let json = dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

pub fn seen_rows(dispatcher: &Wired) -> Vec<Vec<serde_json::Value>> {
    serde_json::from_value(seen(dispatcher)).unwrap()
}

/// Gives every node one object `{ node }` and counts the calls.
pub const HOOK: &str = r"
    globalThis.calls = 0;
    globalThis.objs = {};
    globalThis.nodeOf = (o) => (o === null ? null : o.node);
    globalThis.__inca_targets__ = (ids) => {
        globalThis.calls += 1;
        return ids.map((id) => (globalThis.objs[id] ??= { node: id }));
    };
    ";

/// Records `[label, target, currentTarget, eventPhase, path]` of the event it hears.
pub fn note_walk(label: &str) -> String {
    format!(
        r"
        (e) => out.push([
            '{label}',
            nodeOf(e.target),
            nodeOf(e.currentTarget),
            e.eventPhase,
            e.composedPath().map(nodeOf),
        ])
        "
    )
}

pub fn mouse_button(button: u8) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button,
        buttons: inca_document::buttons_mask(button),
        ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
    })
}

pub const PATHS: &str = r"
    globalThis.seen = [];
    globalThis.paths = (e) => {
        globalThis.seen.push([e.target, e.currentTarget, e.eventPhase,
            e.composedPath()]); globalThis.kept = e; };
    ";

pub fn plain_mouse() -> MousePayload {
    MousePayload::at(7.0, 9.0, inca_document::Modifiers::none())
}

pub fn pointer_payload(source: PointerSource) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        pointer: Some(source),
        ..plain_mouse()
    })
}

pub fn wheel_payload() -> EventPayload {
    EventPayload::Wheel(WheelPayload {
        mouse: plain_mouse(),
        delta_x: 0.0,
        delta_y: 0.0,
        delta_z: 0.0,
        delta_mode: 0,
    })
}

const SEEN: &str = r"
    globalThis.seen = [];
    globalThis.mark = (e) => { globalThis.seen.push(e.type + ':' + e.target); };
    ";

/// A focusable node under the root whose `focus` and `blur` are recorded into `seen`.
pub fn focusable_node(dispatcher: &Wired, cells: &Rc<Cells>) -> NodeId {
    let mut document = cells.document.borrow_mut();
    let node = document.create_node("div").unwrap();
    let root = document.root();
    document
        .append_child(&*cells.window.borrow(), root, node)
        .unwrap();
    document
        .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
        .unwrap();
    drop(document);
    dispatcher.engine.eval::<()>(SEEN).unwrap();
    for event in ["focus", "blur"] {
        listen(dispatcher, node, event, "mark");
    }
    node
}
