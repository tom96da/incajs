// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests of several modules.

#![allow(clippy::unwrap_used)]

use std::rc::Rc;

use std::cell::Cell;

use gpui::{AppContext as _, Context, TestAppContext, Window, point, px};
use inca_document::{EventPayload, EventTargetId, MousePayload, PointerSource, WheelPayload};
use inca_gpui::EventSink;
use inca_jsenv::Clock;
use inca_node::NodeId;

use crate::test_support::{Reported, capture_reporter, engine_for, engine_on_clock};
use crate::{DocumentState, EventDispatcher, Host};
mod fake_script;

pub(crate) use fake_script::*;

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
    let host = Rc::new(Host::default());
    let engine = engine_on_clock(&host, clock);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    let dispatcher = EventDispatcher::new(engine, Rc::clone(&host));
    (dispatcher, host)
}

pub(crate) fn at(x: f32, y: f32) -> gpui::Point<gpui::Pixels> {
    point(px(x), px(y))
}

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

// Snapshot per call: [type, bubbles, cancelable, composed, defaultPrevented
// before, defaultPrevented after preventDefault(), eventPhase,
// currentTarget, isTrusted, timeStamp].
pub(crate) const RECORDER: &str = r"
    globalThis.seen = [];
    globalThis.record = (e) => {
        const before = e.defaultPrevented; e.preventDefault();
        globalThis.seen.push([e.type, e.bubbles, e.cancelable, e.composed,
            before, e.defaultPrevented, e.eventPhase, e.currentTarget,
            e.isTrusted, e.timeStamp]); };
    ";

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

/// Whether `node` matches the state class `class`.
pub(crate) fn state(host: &DocumentState, node: NodeId, class: &str) -> bool {
    inca_style::matches_selector(&host.tree, node, class).unwrap()
}

/// Appends a `<style>` element that holds `css` under the root.
pub(crate) fn add_sheet(host: &Host, css: &str) {
    let host = &mut *host.document.borrow_mut();
    let [style, text] = ["style", "#text"].map(|tag| host.tree.create_node(tag).unwrap());
    host.tree.set_data(text, css).unwrap();
    host.tree.append_child(style, text).unwrap();
    host.tree.append_child(host.root(), style).unwrap();
}

pub(crate) mod keys;

pub(crate) mod pointer {
    use gpui::{MouseButton, MouseUpEvent, VisualTestContext};

    use super::*;

    const POINTER_AND_MOUSE: [&str; 17] = [
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
    ];

    /// Two nodes `a` and `b` under the host root. [`scene`] runs the dispatch on a [`FakeScript`]
    /// and [`js_scene`] on an engine whose listeners on `a` and `b` record `type@target` into
    /// `globalThis.out`, for the tests of what JS reads.
    pub(crate) struct Scene {
        pub(crate) dispatcher: EventDispatcher,
        pub(crate) host: Rc<Host>,
        pub(crate) fake: Option<Rc<FakeScript>>,
        pub(crate) cx: VisualTestContext,
        pub(crate) a: NodeId,
        pub(crate) b: NodeId,
        watched: Vec<NodeId>,
    }

    pub(crate) fn scene(cx: &mut TestAppContext) -> Scene {
        let (dispatcher, host, fake) = fake_dispatcher();
        let (a, b) = (node_under_root(&host), node_under_root(&host));
        Scene {
            dispatcher,
            host,
            fake: Some(fake),
            cx: cx.add_empty_window().clone(),
            a,
            b,
            watched: vec![a, b],
        }
    }

    pub(crate) fn js_scene(cx: &mut TestAppContext) -> Scene {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let (a, b) = (node_under_root(&host), node_under_root(&host));
        let record = r"
            (e) => {
                if (e.eventPhase === 2) {
                    out.push(`${e.type}@${e.target}`);
                }
            }
            ";
        for node in [a, b] {
            for event in POINTER_AND_MOUSE.into_iter().chain(["auxclick"]) {
                listen(&dispatcher, node, event, record);
            }
        }
        Scene {
            dispatcher,
            host,
            fake: None,
            cx: cx.add_empty_window().clone(),
            a,
            b,
            watched: vec![a, b],
        }
    }

    impl Scene {
        /// The events at `a` and `b` and at the nodes of [`Self::watch`] since the last call.
        pub(crate) fn take(&self) -> Vec<String> {
            let Some(fake) = &self.fake else {
                let out = eval_json(&self.dispatcher, "out.splice(0)");
                return serde_json::from_value(out).unwrap();
            };
            fake.take()
                .iter()
                .filter(|walked| {
                    matches!(walked.target, EventTargetId::Node(node) if self.watched.contains(&node))
                })
                .map(Walked::label)
                .collect()
        }

        /// The walks since the last call, at every node.
        pub(crate) fn take_walks(&self) -> Vec<Walked> {
            self.fake.as_ref().unwrap().take()
        }

        /// Makes [`Self::take`] report the events at `node` too.
        pub(crate) fn watch(&mut self, node: NodeId) {
            self.watched.push(node);
        }

        /// Cancels each event of type `name` while `cancel` is set.
        pub(crate) fn cancel(&self, name: &'static str, cancel: Rc<Cell<bool>>) {
            self.fake
                .as_ref()
                .unwrap()
                .on_walk(move |walked| walked.name == name && cancel.get());
        }

        /// Makes the `pointerdown` walk capture the pointer for `node`.
        pub(crate) fn capture_on_pointerdown(&self, node: NodeId) {
            let host = Rc::clone(&self.host);
            self.fake.as_ref().unwrap().on_walk(move |walked| {
                if walked.name == "pointerdown" {
                    request_capture(&host, node);
                }
                false
            });
        }

        /// Runs `f` on each walk, and cancels the event when it returns true.
        pub(crate) fn on_walk(&self, f: impl Fn(&Walked) -> bool + 'static) {
            self.fake.as_ref().unwrap().on_walk(f);
        }

        pub(crate) fn listen(&self, node: NodeId, event: &str, f: &str) {
            listen(&self.dispatcher, node, event, f);
        }

        /// Reports the pointer over `node` as the hover events do.
        pub(crate) fn hover(&mut self, node: NodeId) {
            let sink = self.dispatcher.clone();
            self.cx
                .update(|window, cx| sink.report_hover(Some(node), window, cx));
            *self.dispatcher.state.hovered.borrow_mut() = [node].into();
        }

        /// A raw move over `node` by `(x, y)` from the previous position.
        pub(crate) fn moved(&mut self, node: NodeId, (x, y): (f32, f32)) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.pointer_moved(at(x, y), cx);
                sink.pointer_over(node, window, cx);
                sink.pointer_input("mousemove", node, &mouse_at(x, y), window, cx);
            });
            *self.dispatcher.state.hovered.borrow_mut() = [node].into();
        }

        /// A raw move over `node` to `(x, y)` with the left button held.
        pub(crate) fn moved_held(&mut self, node: NodeId, (x, y): (f32, f32)) {
            let sink = self.dispatcher.clone();
            let payload = EventPayload::Mouse(MousePayload {
                buttons: 1,
                ..MousePayload::at(x, y, inca_document::Modifiers::none())
            });
            self.cx.update(|window, cx| {
                sink.pointer_moved(at(x, y), cx);
                sink.pointer_over(node, window, cx);
                sink.pointer_input("mousemove", node, &payload, window, cx);
            });
        }

        /// Starts (`active`) or ends the drag of a thumb of the container `node`.
        pub(crate) fn bar_drag(&mut self, node: NodeId, active: bool) {
            let sink = self.dispatcher.clone();
            self.cx
                .update(|window, cx| sink.bar_drag(node, active, window, cx));
        }

        pub(crate) fn down(&mut self, node: NodeId, button: u8) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.pointer_input("mousedown", node, &button_payload(button), window, cx);
            });
        }

        /// A press of `button` on a scrollbar of the container `node`.
        pub(crate) fn bar_down(&mut self, node: NodeId, button: u8) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.bar_pressed(node, &button_payload(button), window, cx);
            });
        }

        /// A release of `button` after a press on a scrollbar of the container `node`.
        pub(crate) fn bar_up(&mut self, node: NodeId, button: u8) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.bar_released(node, &button_payload(button), window, cx);
            });
        }

        /// A release of `button` over `node`, pressed over `pressed`.
        pub(crate) fn up(&mut self, node: NodeId, pressed: NodeId, button: u8) {
            self.up_counted(node, pressed, button, 1);
        }

        /// [`Self::up`] for the `count`-th click of a series.
        pub(crate) fn up_counted(
            &mut self,
            node: NodeId,
            pressed: NodeId,
            button: u8,
            count: usize,
        ) {
            let sink = self.dispatcher.clone();
            let mouse_button =
                [MouseButton::Left, MouseButton::Middle, MouseButton::Right][button as usize];
            let event = MouseUpEvent {
                button: mouse_button,
                position: at(1.0, 1.0),
                modifiers: gpui::Modifiers::none(),
                click_count: count,
            };
            self.cx.update(|window, cx| {
                sink.pointer_input("mouseup", node, &button_payload(button), window, cx);
                sink.pointer_released(&event, Some(pressed), node, window, cx);
            });
        }

        /// A script asking to capture the pointer for `node`.
        pub(crate) fn set_capture(&self, node: NodeId) {
            request_capture(&self.host, node);
        }

        /// A script releasing the capture of `node`.
        pub(crate) fn release_capture(&self, node: NodeId) {
            release_capture(&self.host, node);
        }
    }

    fn button_payload(button: u8) -> EventPayload {
        EventPayload::Mouse(MousePayload {
            button,
            ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
        })
    }
}

/// A listener that pushes `[name, currentTarget, eventPhase]`.
pub(crate) fn tagged(name: &str) -> String {
    format!("(e) => out.push(['{name}', e.currentTarget, e.eventPhase])")
}

const SEEN: &str = r"
    globalThis.seen = [];
    globalThis.mark = (e) => { globalThis.seen.push(e.type + ':' + e.target); };
    ";

pub(crate) fn focusable_node(dispatcher: &EventDispatcher, host: &Rc<Host>) -> NodeId {
    let mut host = host.document.borrow_mut();
    let node = host.tree.create_node("div").unwrap();
    let root = host.root();
    host.tree.append_child(root, node).unwrap();
    host.tree
        .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
        .unwrap();
    host.focus.mark_tab_dirty(node);
    drop(host);
    dispatcher.engine.eval::<()>(SEEN).unwrap();
    for event in ["focus", "blur"] {
        listen(dispatcher, node, event, "mark");
    }
    node
}

/// A view whose render runs `begin_frame`, as a window's render does.
pub(crate) struct Frame(pub(crate) Rc<EventDispatcher>);

impl gpui::Render for Frame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.0.begin_frame(window, cx);
        gpui::div()
    }
}

pub(crate) fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<Frame>) {
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

pub(crate) fn set_active(
    cx: &mut TestAppContext,
    window: gpui::WindowHandle<Frame>,
    dispatcher: &EventDispatcher,
    active: bool,
) {
    cx.update_window(window.into(), |_, window, cx| {
        dispatcher.window_activated(active, window, cx);
    })
    .unwrap();
}

pub(crate) fn is_focused(host: &DocumentState, node: NodeId) -> bool {
    state(host, node, ":focus")
}

pub(crate) fn focus_scene(
    cx: &mut TestAppContext,
) -> (
    Rc<crate::EventDispatcher>,
    Rc<crate::Host>,
    gpui::WindowHandle<Frame>,
    u32,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = focusable_node(&dispatcher, &host);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    (dispatcher, host, window, node)
}

/// A focusable node under the root of `host`, on which no script listens.
pub(crate) fn fake_focusable_node(host: &Rc<Host>) -> NodeId {
    let mut host = host.document.borrow_mut();
    let node = host.tree.create_node("div").unwrap();
    let root = host.root();
    host.tree.append_child(root, node).unwrap();
    host.tree
        .set_attribute(node, "tabindex", inca_node::AttributeValue::from("0"))
        .unwrap();
    host.focus.mark_tab_dirty(node);
    node
}

/// A window over one focusable node, on a [`FakeScript`].
pub(crate) fn fake_focus_scene(
    cx: &mut TestAppContext,
) -> (
    Rc<EventDispatcher>,
    Rc<Host>,
    Rc<FakeScript>,
    gpui::WindowHandle<Frame>,
    NodeId,
) {
    let (dispatcher, host, fake) = fake_dispatcher();
    let node = fake_focusable_node(&host);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    (dispatcher, host, fake, window, node)
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
