// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::event::Flags;
use crate::test_support::{Reported, TestRoot, capture_reporter, engine_for, mount_active};
use gpui::{App, AppContext as _, Keystroke, TestAppContext, WindowHandle, point, px};
use inca_gpui::event_sink::EVENTS;
use inca_gpui::{EventSink, InputRequest, InputState, WheelPayload};

mod base_fields;
mod click;
mod constructors;
mod dispatch_event;
mod engine_entry;
mod frame;
mod hover_order;
mod input;
mod input_events;
mod jobs;
mod key_default;
mod movement;
mod pointer;
mod record;
mod target_objects;
mod targets;
mod walk;

/// An `input` holding a value (100 px wide, 6 px per char, from x 0) and a `button`, under a `div`
/// in an active window.
struct InputScene {
    window: WindowHandle<TestRoot>,
    dispatcher: EventDispatcher,
    host: Rc<RefCell<Host>>,
    engine: Rc<inca_jsenv::Engine>,
    parent: NodeId,
    input: NodeId,
    button: NodeId,
}

fn input_scene(cx: &mut TestAppContext, value: &str) -> InputScene {
    let host = Rc::new(RefCell::new(Host::default()));
    let (parent, input, button) = {
        let mut h = host.borrow_mut();
        let root = h.root();
        let [parent, input, button] =
            ["div", "input", "button"].map(|tag| h.tree.create_node(tag).unwrap());
        h.tree.append_child(root, parent).unwrap();
        h.tree.append_child(parent, input).unwrap();
        h.tree.append_child(parent, button).unwrap();
        h.tree.set_style(input, "width", 100.0).unwrap();
        h.tree.set_style(input, "text_size", 10.0).unwrap();
        h.focus.mark_tab_dirty(input);
        h.focus.mark_tab_dirty(button);
        h.inputs
            .get_mut()
            .entry(input)
            .or_default()
            .set_value(value);
        (parent, input, button)
    };
    let engine = engine_for(&host);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    let window = mount_active(cx, &host, &engine, parent);
    let dispatcher = window
        .read_with(cx, |root, _| root.dispatcher.clone())
        .unwrap();
    InputScene {
        window,
        dispatcher,
        host,
        engine,
        parent,
        input,
        button,
    }
}

impl InputScene {
    fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&Self, &mut gpui::Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
            .unwrap()
    }

    fn draw(&self, cx: &mut TestAppContext) {
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    }

    fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
        self.host.borrow_mut().focus.request_focus(node, false);
        self.draw(cx);
        cx.run_until_parked();
    }

    /// Dispatches the space-separated keystrokes and returns whether the last one was handled.
    fn key(&self, cx: &mut TestAppContext, keys: &str) -> bool {
        let mut handled = false;
        for key in keys.split(' ') {
            let keystroke = Keystroke::parse(key).unwrap();
            handled = self.in_window(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        cx.run_until_parked();
        self.draw(cx);
        handled
    }

    fn request(&self, cx: &mut TestAppContext, request: InputRequest) {
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .input_request(scene.input, request, window, cx);
        });
    }

    fn deactivate(&self, cx: &mut TestAppContext) {
        self.in_window(cx, |scene, window, cx| {
            scene.dispatcher.window_activated(false, window, cx);
        });
    }

    /// Sends the raw pointer `event` to `node`.
    fn pointer(&self, cx: &mut TestAppContext, event: &str, node: NodeId, mouse: MousePayload) {
        let payload = EventPayload::Mouse(mouse);
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .pointer_input(event, node, &payload, window, cx);
        });
    }

    fn press(&self, cx: &mut TestAppContext, x: f32, shift: bool) {
        self.pointer(cx, "mousedown", self.input, mouse_of(x, 0, shift));
    }

    fn move_to(&self, cx: &mut TestAppContext, x: f32) {
        self.pointer(cx, "mousemove", self.input, mouse_of(x, 0, false));
    }

    fn release(&self, cx: &mut TestAppContext, x: f32) {
        self.pointer(cx, "mouseup", self.input, mouse_of(x, 0, false));
    }

    fn state(&self) -> InputState {
        self.host.borrow().inputs.borrow()[&self.input].clone()
    }

    /// Selects `anchor` to `focus` in the input.
    fn select(&self, anchor: usize, focus: usize) {
        let host = self.host.borrow();
        let mut inputs = host.inputs.borrow_mut();
        inputs.get_mut(&self.input).unwrap().select(anchor, focus);
    }

    fn set_attribute(&self, name: &str) {
        let mut host = self.host.borrow_mut();
        host.tree.set_attribute(self.input, name, true).unwrap();
    }
}

/// A mouse payload of `button` at window x `x` and y 5.
fn mouse_of(x: f32, button: u8, shift: bool) -> MousePayload {
    let modifiers = gpui::Modifiers {
        shift,
        ..gpui::Modifiers::none()
    };
    MousePayload {
        button,
        ..MousePayload::at(at(x, 5.0), modifiers)
    }
}

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

/// A `click` from the mouse pointer.
fn pointer_click() -> EventPayload {
    EventPayload::Mouse(MousePayload {
        pointer: Some(PointerSource::Mouse),
        ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
    })
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

fn key_payload(modifiers: gpui::Modifiers) -> EventPayload {
    EventPayload::Key(inca_gpui::KeyPayload {
        key: "a".into(),
        repeat: false,
        modifiers,
        is_composing: false,
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

/// Registers `f` on the global target `tag`, `"$document"` or `"$window"`, in the capture phase
/// when `capture` is set.
fn listen_global(dispatcher: &EventDispatcher, tag: &str, event: &str, capture: bool, f: &str) {
    dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_test_listen__('{tag}', '{event}', {f}, {capture}, false);"
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

/// A new node under the host root, so an event at it also reaches document and window.
fn node_under_root(host: &Rc<RefCell<Host>>) -> NodeId {
    let mut h = host.borrow_mut();
    let node = h.tree.create_node("div").unwrap();
    let root = h.root();
    h.tree.append_child(root, node).unwrap();
    node
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
