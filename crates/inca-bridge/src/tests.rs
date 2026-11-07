// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests of several modules.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Context, TestAppContext, Window, point, px};
use inca_gpui::{EventPayload, EventSink, MousePayload, PointerSource, WheelPayload};
use inca_jsenv::Clock;
use inca_node::NodeId;

use crate::test_support::{Reported, capture_reporter, engine_for, engine_on_clock};
use crate::{EventDispatcher, Host};

pub(crate) fn dispatcher_with_engine() -> (EventDispatcher, Rc<RefCell<Host>>, Reported) {
    let host = Rc::new(RefCell::new(Host::default()));
    let engine = engine_for(&host);
    engine.eval::<()>("globalThis.out = [];").unwrap();

    let (report, reported) = capture_reporter();
    let dispatcher = EventDispatcher::new(engine, Rc::clone(&host)).with_reporter(report);

    (dispatcher, host, reported)
}

/// [`dispatcher_with_engine`] on an engine whose clock is `clock`.
pub(crate) fn dispatcher_on_clock(clock: Clock) -> (EventDispatcher, Rc<RefCell<Host>>) {
    let host = Rc::new(RefCell::new(Host::default()));
    let engine = engine_on_clock(&host, clock);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    let dispatcher = EventDispatcher::new(engine, Rc::clone(&host));
    (dispatcher, host)
}

pub(crate) fn at(x: f32, y: f32) -> gpui::Point<gpui::Pixels> {
    point(px(x), px(y))
}

pub(crate) fn mouse_at(x: f32, y: f32) -> EventPayload {
    EventPayload::Mouse(MousePayload::at(at(x, y), gpui::Modifiers::none()))
}

/// A `click` from the mouse pointer.
pub(crate) fn pointer_click() -> EventPayload {
    EventPayload::Mouse(MousePayload {
        pointer: Some(PointerSource::Mouse),
        ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
    })
}

pub(crate) fn wheel_at(x: f32, y: f32) -> EventPayload {
    EventPayload::Wheel(WheelPayload {
        mouse: MousePayload::at(at(x, y), gpui::Modifiers::none()),
        delta_x: 0.0,
        delta_y: 1.0,
        delta_z: 0.0,
        delta_mode: 0,
    })
}

pub(crate) fn key_payload(modifiers: gpui::Modifiers) -> EventPayload {
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

/// Registers `f` on the global target `tag`, `"$document"` or `"$window"`, in the capture phase
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

pub(crate) fn tree_of_three(host: &Rc<RefCell<Host>>) -> [NodeId; 3] {
    let mut h = host.borrow_mut();
    let outer = h.tree.create_node("div").unwrap();
    let parent = h.tree.create_node("div").unwrap();
    let child = h.tree.create_node("div").unwrap();
    h.tree.append_child(outer, parent).unwrap();
    h.tree.append_child(parent, child).unwrap();
    [outer, parent, child]
}

/// A new node under the host root, so an event at it also reaches document and window.
pub(crate) fn node_under_root(host: &Rc<RefCell<Host>>) -> NodeId {
    let mut h = host.borrow_mut();
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
    let json = dispatcher
        .engine
        .eval::<String>(&format!("JSON.stringify({js})"))
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

/// Whether `node` matches the state class `class`.
pub(crate) fn state(host: &Host, node: NodeId, class: &str) -> bool {
    inca_node::matches_selector(&host.tree, node, class).unwrap()
}

/// Appends a `<style>` element that holds `css` under the root.
pub(crate) fn add_sheet(host: &RefCell<Host>, css: &str) {
    let host = &mut *host.borrow_mut();
    let [style, text] = ["style", "#text"].map(|tag| host.tree.create_node(tag).unwrap());
    host.tree.set_attribute(text, "value", css).unwrap();
    host.tree.append_child(style, text).unwrap();
    host.tree.append_child(host.root(), style).unwrap();
}

pub(crate) mod keys {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{App, AppContext as _, Keystroke, TestAppContext, Window, WindowHandle, WindowId};
    use inca_gpui::{EventPayload, EventSink, KeyPayload};
    use inca_node::NodeId;

    use super::mouse_at;
    use crate::test_support::{TestRoot, engine_for, focusable_tree, mount_active};
    use crate::{EventDispatcher, Host};

    /// Two buttons, a focusable div and a div without a tab index under one parent. The buttons and
    /// the focusable div have a `click` listener. All but the plain div have `keydown` and `keyup`
    /// listeners that call `preventDefault` while `globalThis.cancel` is set. The parent has the
    /// same listeners, controlled by `globalThis.cancelParent`.
    pub(crate) struct Scene {
        pub(crate) window: WindowHandle<TestRoot>,
        pub(crate) dispatcher: EventDispatcher,
        pub(crate) host: Rc<RefCell<Host>>,
        pub(crate) engine: Rc<inca_jsenv::Engine>,
        pub(crate) parent: NodeId,
        pub(crate) first: NodeId,
        pub(crate) second: NodeId,
        pub(crate) field: NodeId,
        pub(crate) plain: NodeId,
    }

    pub(crate) fn scene(cx: &mut TestAppContext) -> Scene {
        let (host, parent, children) = focusable_tree(1, false);
        let field = children[0];
        let (first, second, plain) = {
            let mut h = host.borrow_mut();
            let plain = h.tree.create_node("div").unwrap();
            h.tree.append_child(parent, plain).unwrap();
            let first = h.tree.create_node("button").unwrap();
            let second = h.tree.create_node("button").unwrap();
            h.tree.insert_before(parent, first, Some(field)).unwrap();
            h.tree.insert_before(parent, second, Some(field)).unwrap();
            h.focus.mark_tab_dirty(first);
            h.focus.mark_tab_dirty(second);
            (first, second, plain)
        };
        let engine = engine_for(&host);
        engine.eval::<()>("globalThis.out = [];").unwrap();
        for button in [first, second, field] {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({button}, 'click', (e) => {{
                        out.push(`click:${{e.currentTarget}}:${{e.isTrusted}}`);
                    }});
                    "
                ))
                .unwrap();
        }
        for node in [first, second, field] {
            for event in ["keydown", "keyup"] {
                engine
                    .eval::<()>(&format!(
                        r"
                        __inca_test_listen__({node}, '{event}', (e) => {{
                            if (globalThis.cancel) {{
                                e.preventDefault();
                            }}
                        }});
                        "
                    ))
                    .unwrap();
            }
        }
        for event in ["keydown", "keyup"] {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({parent}, '{event}', (e) => {{
                        if (globalThis.cancelParent) {{
                            e.preventDefault();
                        }}
                    }});
                    "
                ))
                .unwrap();
        }
        let window = mount_active(cx, &host, &engine, parent);
        let dispatcher = window
            .read_with(cx, |root, _| root.dispatcher.clone())
            .unwrap();
        Scene {
            window,
            dispatcher,
            host,
            engine,
            parent,
            first,
            second,
            field,
            plain,
        }
    }

    impl Scene {
        pub(crate) fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
            self.host.borrow_mut().focus.request_focus(node, false);
            self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
            cx.run_until_parked();
        }

        pub(crate) fn blur(&self, cx: &mut TestAppContext, node: NodeId) {
            self.host.borrow_mut().focus.request_blur(node);
            self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
            cx.run_until_parked();
        }

        pub(crate) fn focused(&self, cx: &mut TestAppContext) -> Option<NodeId> {
            self.in_window(cx, |scene, window, cx| {
                scene.host.borrow().focus.focused_node(window, cx)
            })
        }

        pub(crate) fn key(&self, cx: &mut TestAppContext, key: &str, up: bool, canceled: bool) {
            let keystroke = Keystroke::parse(key).unwrap();
            let payload = EventPayload::Key(KeyPayload {
                key: keystroke.key.clone(),
                repeat: false,
                modifiers: keystroke.modifiers,
                is_composing: false,
            });
            self.engine
                .eval::<()>(&format!("globalThis.cancel = {canceled};"))
                .unwrap();
            self.in_window(cx, |scene, window, cx| {
                scene
                    .dispatcher
                    .key_input(&keystroke, up, &payload, window, cx);
            });
            cx.run_until_parked();
        }

        pub(crate) fn mouse_down(&self, cx: &mut TestAppContext, node: NodeId) {
            let payload = mouse_at(1.0, 1.0);
            self.in_window(cx, |scene, window, cx| {
                let sink = &scene.dispatcher;
                sink.pointer_input("mousedown", node, &payload, window, cx);
                window.draw(cx).clear(cx);
            });
            cx.run_until_parked();
        }

        pub(crate) fn fire(&self, cx: &mut TestAppContext, node: NodeId, event: &str) {
            self.in_window(cx, |scene, window, cx| {
                let payload = EventPayload::Focus {
                    related_target: None,
                };
                EventSink::fire(&scene.dispatcher, node, event, &payload, window, cx);
            });
        }

        pub(crate) fn window_id(&self, cx: &mut TestAppContext) -> WindowId {
            self.in_window(cx, |_, window, _| window.window_handle().window_id())
        }

        pub(crate) fn in_window<R>(
            &self,
            cx: &mut TestAppContext,
            f: impl FnOnce(&Self, &mut Window, &mut App) -> R,
        ) -> R {
            cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
                .unwrap()
        }

        pub(crate) fn clicks(&self) -> String {
            self.engine.eval::<String>("JSON.stringify(out)").unwrap()
        }

        pub(crate) fn click_of(node: NodeId) -> String {
            format!(r#"["click:{node}:true"]"#)
        }
    }
}

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

    /// Two nodes `a` and `b` under the host root, each listening for every pointer and mouse event.
    /// A listener records `type@target` into `globalThis.out` where the event is at its target.
    pub(crate) struct Scene {
        pub(crate) dispatcher: EventDispatcher,
        pub(crate) host: Rc<RefCell<Host>>,
        pub(crate) cx: VisualTestContext,
        pub(crate) a: NodeId,
        pub(crate) b: NodeId,
    }

    pub(crate) fn scene(cx: &mut TestAppContext) -> Scene {
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
            cx: cx.add_empty_window().clone(),
            a,
            b,
        }
    }

    impl Scene {
        /// The events recorded since the last call.
        pub(crate) fn take(&self) -> Vec<String> {
            let out = eval_json(&self.dispatcher, "out.splice(0)");
            serde_json::from_value(out).unwrap()
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

        pub(crate) fn down(&mut self, node: NodeId, button: u8) {
            let sink = self.dispatcher.clone();
            self.cx.update(|window, cx| {
                sink.pointer_input("mousedown", node, &button_payload(button), window, cx);
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

        pub(crate) fn eval(&self, js: &str) {
            self.dispatcher.engine.eval::<()>(js).unwrap();
        }
    }

    fn button_payload(button: u8) -> EventPayload {
        EventPayload::Mouse(MousePayload {
            button,
            ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
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

pub(crate) fn focusable_node(dispatcher: &EventDispatcher, host: &Rc<RefCell<Host>>) -> NodeId {
    let mut host = host.borrow_mut();
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

pub(crate) fn seen_text(dispatcher: &EventDispatcher) -> String {
    dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen)")
        .unwrap()
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

pub(crate) fn is_focused(host: &Host, node: NodeId) -> bool {
    state(host, node, ":focus")
}

pub(crate) fn focus_scene(
    cx: &mut TestAppContext,
) -> (
    Rc<crate::EventDispatcher>,
    Rc<std::cell::RefCell<crate::Host>>,
    gpui::WindowHandle<Frame>,
    u32,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = focusable_node(&dispatcher, &host);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    (dispatcher, host, window, node)
}
