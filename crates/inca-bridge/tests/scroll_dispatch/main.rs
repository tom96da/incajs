// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for wheel scrolling of scroll containers: a real
//! `VirtualTree` rendered through `render_tree_with_events`, scrolled by
//! `ScrollWheelEvent`s.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Bounds, Context, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, TestAppContext,
    VisualTestContext, Window, point, prelude::*, px,
};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{EventSink, NodeId, NodeLayout, render_tree_with_events};
use inca_jsenv::{Engine, EngineError};

use inca_bridge::test_support;

struct Root {
    host: Rc<RefCell<Host>>,
    dispatcher: EventDispatcher,
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dispatcher.begin_frame(window, cx);
        let host = self.host.borrow();
        let element = render_tree_with_events(&host.tree, 0, &self.dispatcher).unwrap();
        // The walk records only the children of node 0, which is 100x100.
        let bounds = Bounds::new(Point::default(), gpui::size(px(100.), px(100.)));
        let (border, scroll) = (px(0.), Point::default());
        let layout = NodeLayout {
            bounds,
            border,
            scroll,
        };
        self.dispatcher.record_layout(0, layout);
        element
    }
}

struct Scene {
    host: Rc<RefCell<Host>>,
    engine: Rc<Engine>,
    dispatcher: EventDispatcher,
    cx: VisualTestContext,
    reported: Rc<RefCell<Vec<EngineError>>>,
}

impl Scene {
    /// Renders node 0 of `host` after running `js`, which registers listeners.
    fn new(cx: &mut TestAppContext, host: Host, js: &str) -> Self {
        Self::with_bindings(cx, host, js, "")
    }

    /// Like [`Self::new`], with `before_draw` run before the first draw.
    fn with_bindings(cx: &mut TestAppContext, host: Host, js: &str, before_draw: &str) -> Self {
        let host = Rc::new(RefCell::new(host));
        let engine = test_support::engine_for(&host);
        engine.eval::<()>("globalThis.ran = [];").unwrap();
        engine.eval::<()>(js).unwrap();
        engine.eval::<()>(before_draw).unwrap();
        let (report, reported) = test_support::capture_reporter();
        let dispatcher =
            EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host)).with_reporter(report);
        let window = cx.add_window(|_, _| Root {
            host: Rc::clone(&host),
            dispatcher: dispatcher.clone(),
        });
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        Self {
            host,
            engine,
            dispatcher,
            cx: VisualTestContext::from_window(window.into(), cx),
            reported,
        }
    }

    /// One wheel event over the top-left corner; a negative `y` scrolls down.
    fn wheel(&mut self, y: f32) {
        self.wheel_at(10., y);
    }

    /// Like [`Self::wheel`], with the pointer `pointer_y` from the top.
    fn wheel_at(&mut self, pointer_y: f32, y: f32) {
        self.wheel_delta(pointer_y, ScrollDelta::Pixels(point(px(0.), px(y))));
    }

    /// One wheel event with `delta`, then a draw so offsets are clamped.
    fn wheel_delta(&mut self, pointer_y: f32, delta: ScrollDelta) {
        self.cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(pointer_y)),
            delta,
            ..Default::default()
        });
        self.cx.run_until_parked();
        self.cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// Runs `code` in the app's JS context.
    fn js(&self, code: &str) {
        self.engine.eval::<()>(code).unwrap();
    }

    fn offset(&self, node: NodeId) -> Point<Pixels> {
        self.dispatcher.scroll_handle(node).unwrap().offset()
    }

    fn draw(&mut self) {
        self.cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }

    /// Draws one frame without a refresh.
    fn frame(&mut self) {
        self.cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn line_height(&mut self) -> Pixels {
        self.cx.update(|window, _| window.line_height())
    }

    fn ran(&self) -> String {
        self.engine.eval("JSON.stringify(ran)").unwrap()
    }
}

/// JS registering `f`, a function source, as a listener of `event` on `node`.
fn listen(node: NodeId, event: &str, f: &str, capture: bool, passive: bool) -> String {
    format!("__inca_test_listen__({node}, '{event}', {f}, {capture}, {passive});\n")
}

/// JS registering `f` as a `wheel` listener of `node`.
fn on_wheel(node: NodeId, f: &str) -> String {
    listen(node, "wheel", f, false, false)
}

/// Adds a sized div under `parent`, or styles the root when there is none.
fn add_div(host: &mut Host, parent: Option<NodeId>, height: f64, scrolls: bool) -> NodeId {
    let node = if parent.is_some() {
        host.tree.create_node("div").unwrap()
    } else {
        host.root
    };
    host.tree.set_style(node, "width", 100.0).unwrap();
    host.tree.set_style(node, "height", height).unwrap();
    if scrolls {
        host.tree.set_style(node, "overflow_y", "scroll").unwrap();
    }
    if let Some(parent) = parent {
        host.tree.append_child(parent, node).unwrap();
    }
    node
}

/// Node 0 scrolls (100px over 300px of content, node 1).
fn one_container() -> Host {
    let mut host = Host::default();
    let container = add_div(&mut host, None, 100.0, true);
    add_div(&mut host, Some(container), 300.0, false);
    host
}

/// Node 0 scrolls 200px over node 1, which scrolls 200px over node 2, and
/// node 3 fills the rest of node 0.
fn nested_containers() -> Host {
    let mut host = Host::default();
    let outer = add_div(&mut host, None, 100.0, true);
    let inner = add_div(&mut host, Some(outer), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(outer), 200.0, false);
    host
}

/// A container of 100x100 with `styles` over a 300x300 child.
fn styled_container(styles: &[(&str, &str)]) -> Host {
    let mut host = Host::default();
    let root = host.root;
    host.tree.set_style(root, "width", 100.0).unwrap();
    host.tree.set_style(root, "height", 100.0).unwrap();
    for (key, value) in styles {
        host.tree.set_style(root, *key, *value).unwrap();
    }
    let child = host.tree.create_node("div").unwrap();
    host.tree.set_style(child, "width", 300.0).unwrap();
    host.tree.set_style(child, "height", 300.0).unwrap();
    host.tree.append_child(root, child).unwrap();
    host
}

/// Three scroll containers, each 100px over 200px of further content: node 0
/// holds node 1, which holds node 2, which holds node 3 (300px).
fn triple_containers() -> Host {
    let mut host = Host::default();
    let outer = add_div(&mut host, None, 100.0, true);
    let middle = add_div(&mut host, Some(outer), 100.0, true);
    let inner = add_div(&mut host, Some(middle), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(middle), 200.0, false);
    add_div(&mut host, Some(outer), 200.0, false);
    host
}

mod events;
mod wheel;
