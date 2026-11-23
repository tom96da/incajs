// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scenes the pointer tests run on and the JS helpers that count and record their events.

#![allow(clippy::unwrap_used, dead_code, unused_imports)]

pub use std::rc::Rc;

pub use super::rig::{self as wiring, Rig, Wired};
use super::rig::{at, dispatcher_with_engine, eval_json, listen, mouse_at, node_under_root};
pub use gpui::{
    Context, Modifiers, MouseButton, MouseDownEvent, MouseExitEvent, MouseUpEvent, Render,
    ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, Window, point, prelude::*, px,
};
pub use inca_gpui::{Scrollbars, render_tree_with_events};
pub use inca_jsenv::{Engine, EngineError};
pub use inca_node::NodeId;

use inca_document::{EventPayload, MousePayload};
use inca_gpui::EventSink;
pub use inca_style::set_style;
use inca_window::Cells;

/// Counts `event` on `node` into `globalThis.clicks` and keeps the last one
/// in `globalThis.lastEvent`.
pub fn install_click_counter(engine: &Engine, node: NodeId, event: &str) {
    engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({node}, '{event}', (event) => {{
                globalThis.clicks = (globalThis.clicks || 0) + 1;
                globalThis.lastEvent = event;
            }});
            "
        ))
        .unwrap();
}

pub fn clicks(engine: &Engine) -> f64 {
    engine.eval::<f64>("globalThis.clicks || 0").unwrap()
}

pub struct ClickableRoot {
    pub host: Rig,
    pub node: NodeId,
    pub dispatcher: inca_window::EventDispatcher,
}

impl Render for ClickableRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.document.borrow();
        render_tree_with_events(
            host.tree(),
            &host.style,
            self.node,
            &self.dispatcher,
            Scrollbars::Overlay,
        )
        .unwrap()
    }
}

pub fn build_clickable_tree() -> (Rig, NodeId) {
    let host = Rig::default();
    let mut host_mut = host.document.borrow_mut();
    let node = host_mut.create_node("div").unwrap();
    host_mut.set_style_prop(node, "width", 100.0).unwrap();
    host_mut.set_style_prop(node, "height", 100.0).unwrap();
    drop(host_mut);
    (host, node)
}

/// Mounts `node` of `host`, dispatching into `engine`, and draws the first
/// frame.
pub fn mount_clickable(
    cx: &mut TestAppContext,
    host: &Rig,
    engine: &Rc<Engine>,
    node: NodeId,
) -> VisualTestContext {
    let dispatcher = host
        .dispatcher(engine, inca_jsenv::stderr_reporter())
        .dispatcher()
        .clone();
    let window = cx.add_window(|_, _| ClickableRoot {
        host: host.clone(),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    VisualTestContext::from_window(window.into(), cx)
}

/// A row 300 wide holding pane A (0..100) and pane B (100..200), each 100x100.
/// The listeners record `[type, movementX, movementY]` into `globalThis.seen`.
/// `a_events` are registered on A and `root_events` on the row; B has no
/// listener.
pub struct Panes {
    host: Rig,
    pub root: NodeId,
    pub a: NodeId,
    pub b: NodeId,
    pub engine: Rc<Engine>,
    pub dispatcher: Wired,
}

pub fn build_panes(a_events: &[&str], root_events: &[&str]) -> Panes {
    build_panes_recording(
        a_events,
        root_events,
        "(e) => { globalThis.seen.push([e.type, e.movementX, e.movementY]); }",
    )
}

/// [`build_panes`] with `record`, a JS function, as every listener.
pub fn build_panes_recording(a_events: &[&str], root_events: &[&str], record: &str) -> Panes {
    let host = Rig::default();
    let (root, a, b) = {
        let view = host.window.borrow();
        let mut host = host.document.borrow_mut();
        let root = host.create_node("div").unwrap();
        host.set_style_prop(root, "display", "flex").unwrap();
        host.set_style_prop(root, "flex_direction", "row").unwrap();
        host.set_style_prop(root, "width", 300.0).unwrap();
        host.set_style_prop(root, "height", 100.0).unwrap();
        let mut pane = || {
            let pane = host.create_node("div").unwrap();
            host.set_style_prop(pane, "width", 100.0).unwrap();
            host.set_style_prop(pane, "height", 100.0).unwrap();
            host.append_child(&*view, root, pane).unwrap();
            pane
        };
        let (a, b) = (pane(), pane());
        (root, a, b)
    };
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    for (node, events) in [(a, a_events), (root, root_events)] {
        for event in events {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', {record});
                    "
                ))
                .unwrap();
        }
    }
    let dispatcher = host.dispatcher(&engine, inca_jsenv::stderr_reporter());
    Panes {
        host,
        root,
        a,
        b,
        engine,
        dispatcher,
    }
}

/// Mounts `panes` and draws the first frame.
pub fn mount_panes(cx: &mut TestAppContext, panes: &Panes) -> VisualTestContext {
    let window = cx.add_window(|_, _| ClickableRoot {
        host: panes.host.clone(),
        node: panes.root,
        dispatcher: panes.dispatcher.dispatcher().clone(),
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    VisualTestContext::from_window(window.into(), cx)
}

/// A parent over a child, both listening for `events`; the listeners record
/// `[type, currentTarget, eventId]` into `globalThis.seen`.
pub fn mount_recording_pair(
    cx: &mut TestAppContext,
    events: &[&str],
) -> (VisualTestContext, Rc<Engine>, NodeId, NodeId) {
    mount_recording_pair_with(
        cx,
        events,
        "(e) => { globalThis.seen.push([e.type, e.currentTarget, e.eventId]); }",
    )
}

/// [`mount_recording_pair`] with `record`, a JS function, as every listener.
pub fn mount_recording_pair_with(
    cx: &mut TestAppContext,
    events: &[&str],
    record: &str,
) -> (VisualTestContext, Rc<Engine>, NodeId, NodeId) {
    let host = Rig::default();
    let (parent, child) = {
        let view = host.window.borrow();
        let mut host = host.document.borrow_mut();
        let parent = host.create_node("div").unwrap();
        host.set_style_prop(parent, "width", 100.0).unwrap();
        host.set_style_prop(parent, "height", 100.0).unwrap();
        let child = host.create_node("div").unwrap();
        host.set_style_prop(child, "width", 100.0).unwrap();
        host.set_style_prop(child, "height", 100.0).unwrap();
        host.append_child(&*view, parent, child).unwrap();
        (parent, child)
    };
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    for event in events {
        for node in [parent, child] {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', {record});
                    "
                ))
                .unwrap();
        }
    }
    let dispatcher = host.dispatcher(&engine, inca_jsenv::stderr_reporter());
    let window = cx.add_window(|_, _| ClickableRoot {
        host,
        node: parent,
        dispatcher: dispatcher.dispatcher().clone(),
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    (
        VisualTestContext::from_window(window.into(), cx),
        engine,
        parent,
        child,
    )
}

pub fn take_ids(engine: &Engine) -> Vec<(String, NodeId, u64)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

/// The fields of `globalThis.lastEvent` that `want` names, as the listener
/// read them.
pub fn event_fields(engine: &Engine, want: &serde_json::Value) -> serde_json::Value {
    let keys =
        serde_json::to_string(&want.as_object().unwrap().keys().collect::<Vec<_>>()).unwrap();
    let json = engine
        .eval::<String>(&format!(
            "JSON.stringify(Object.fromEntries({keys}.map((k) => [k, globalThis.lastEvent[k]])))"
        ))
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

const POINTER_AND_MOUSE: [&str; 18] = [
    "pointerdown",
    "pointerup",
    "pointermove",
    "pointerover",
    "pointerout",
    "pointerenter",
    "pointerleave",
    "gotpointercapture",
    "lostpointercapture",
    "mousedown",
    "mouseup",
    "mousemove",
    "mouseover",
    "mouseout",
    "mouseenter",
    "mouseleave",
    "click",
    "auxclick",
];

/// Two nodes `a` and `b` under the document root. The listeners on `a` and `b` record
/// `type@target` into `globalThis.out`.
pub struct PointerScene {
    pub dispatcher: Wired,
    pub cells: Rc<Cells>,
    pub cx: VisualTestContext,
    pub a: NodeId,
    pub b: NodeId,
}

pub fn js_pointer_scene(cx: &mut TestAppContext) -> PointerScene {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let (a, b) = (node_under_root(&cells), node_under_root(&cells));
    let record = r"
        (e) => {
            if (e.eventPhase === 2) {
                out.push(`${e.type}@${e.target}`);
            }
        }
        ";
    for node in [a, b] {
        for event in POINTER_AND_MOUSE {
            listen(&dispatcher, node, event, record);
        }
    }
    PointerScene {
        dispatcher,
        cells,
        cx: cx.add_empty_window().clone(),
        a,
        b,
    }
}

fn button_payload(button: u8) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button,
        ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
    })
}

impl PointerScene {
    /// The events at `a` and `b` since the last call.
    pub fn take(&self) -> Vec<String> {
        serde_json::from_value(eval_json(&self.dispatcher, "out.splice(0)")).unwrap()
    }

    pub fn listen(&self, node: NodeId, event: &str, f: &str) {
        listen(&self.dispatcher, node, event, f);
    }

    /// Reports the pointer over `node` as the hover events do.
    pub fn hover(&mut self, node: NodeId) {
        let sink = self.dispatcher.dispatcher().clone();
        self.cx
            .update(|window, cx| sink.hover_changed(node, true, window, cx));
        self.cx.run_until_parked();
    }

    /// A raw move over `node` by `(x, y)` from the previous position.
    pub fn moved(&mut self, node: NodeId, (x, y): (f32, f32)) {
        let sink = self.dispatcher.dispatcher().clone();
        self.cx.update(|window, cx| {
            sink.pointer_moved(at(x, y), cx);
            sink.pointer_over(node, window, cx);
            sink.pointer_input("mousemove", node, &mouse_at(x, y), window, cx);
            sink.hover_changed(node, true, window, cx);
        });
    }

    pub fn down(&mut self, node: NodeId, button: u8) {
        let sink = self.dispatcher.dispatcher().clone();
        self.cx.update(|window, cx| {
            sink.pointer_input("mousedown", node, &button_payload(button), window, cx);
        });
    }

    /// A release of `button` over `node`, pressed over `pressed`.
    pub fn up(&mut self, node: NodeId, pressed: NodeId, button: u8) {
        let sink = self.dispatcher.dispatcher().clone();
        let mouse_button =
            [MouseButton::Left, MouseButton::Middle, MouseButton::Right][button as usize];
        let event = MouseUpEvent {
            button: mouse_button,
            position: at(1.0, 1.0),
            modifiers: gpui::Modifiers::none(),
            click_count: 1,
        };
        self.cx.update(|window, cx| {
            sink.pointer_input("mouseup", node, &button_payload(button), window, cx);
            sink.pointer_released(&event, Some(pressed), node, window, cx);
        });
    }
}
