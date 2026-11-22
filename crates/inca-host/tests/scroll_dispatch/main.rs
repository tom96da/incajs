// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for scrolling of scroll containers: a real `VirtualTree` rendered through
//! `render_tree_with_events`, scrolled by `ScrollEvent`s, by script offsets and by the
//! reveal of focused or scrolled-to nodes.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Bounds, Context, Pixels, Point, Render, ScrollDelta, ScrollEvent, TestAppContext,
    VisualTestContext, Window, point, prelude::*, px,
};
use inca_gpui::{EventSink, NodeLayout, Scrollbars, render_tree_with_events};
use inca_host::wiring::{self, Rig, Wired};
use inca_jsenv::{Engine, EngineError};
use inca_node::NodeId;

use inca_style::set_style;

struct Root {
    host: Rig,
    dispatcher: inca_window::EventDispatcher,
    /// The mode the scrollbars are rendered in.
    mode: Scrollbars,
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dispatcher.begin_frame(window, cx);
        let host = self.host.document.borrow();
        let element =
            render_tree_with_events(&host.tree, &host.style, 0, &self.dispatcher, self.mode)
                .unwrap();
        // The walk records only the children of node 0, which is 100x100.
        let bounds = Bounds::new(Point::default(), gpui::size(px(100.), px(100.)));
        let (border, scroll) = (px(0.), Point::default());
        let layout = NodeLayout {
            bounds,
            border,
            scroll,
            gutter: gpui::Size::default(),
        };
        self.dispatcher.record_layout(0, layout);
        element
    }
}

struct Scene {
    host: Rig,
    engine: Rc<Engine>,
    dispatcher: Wired,
    cx: VisualTestContext,
    reported: Rc<RefCell<Vec<EngineError>>>,
}

impl Scene {
    /// Renders node 0 of `host` after running `js`, which registers listeners.
    fn new(cx: &mut TestAppContext, host: Rig, js: &str) -> Self {
        Self::with_bindings(cx, host, js, "")
    }

    /// Like [`Self::new`], with the scrollbars rendered in `mode`.
    fn in_mode(cx: &mut TestAppContext, host: Rig, js: &str, mode: Scrollbars) -> Self {
        Self::build(cx, host, js, "", mode)
    }

    /// Like [`Self::new`], with `before_draw` run before the first draw.
    fn with_bindings(cx: &mut TestAppContext, host: Rig, js: &str, before_draw: &str) -> Self {
        Self::build(cx, host, js, before_draw, Scrollbars::Overlay)
    }

    fn build(
        cx: &mut TestAppContext,
        host: Rig,
        js: &str,
        before_draw: &str,
        mode: Scrollbars,
    ) -> Self {
        let engine = wiring::engine_for(&host);
        engine.eval::<()>("globalThis.ran = [];").unwrap();
        engine.eval::<()>(js).unwrap();
        engine.eval::<()>(before_draw).unwrap();
        let (report, reported) = wiring::capture_reporter();
        let dispatcher = host.dispatcher(&engine, report);
        let window = cx.add_window(|_, _| Root {
            host: host.clone(),
            dispatcher: dispatcher.dispatcher().clone(),
            mode,
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
        self.cx.simulate_event(ScrollEvent {
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
fn add_div(host: &mut Rig, parent: Option<NodeId>, height: f64, scrolls: bool) -> NodeId {
    let node = if parent.is_some() {
        host.document.borrow_mut().tree.create_node("div").unwrap()
    } else {
        host.document.borrow_mut().root()
    };
    set_style(&mut host.document.borrow_mut().tree, node, "width", 100.0).unwrap();
    set_style(&mut host.document.borrow_mut().tree, node, "height", height).unwrap();
    if scrolls {
        set_style(
            &mut host.document.borrow_mut().tree,
            node,
            "overflow_y",
            "scroll",
        )
        .unwrap();
    }
    if let Some(parent) = parent {
        host.document
            .borrow_mut()
            .tree
            .append_child(parent, node)
            .unwrap();
    }
    node
}

/// Node 0 scrolls (100px over 300px of content, node 2).
fn one_container() -> Rig {
    let mut host = Rig::default();
    let container = add_div(&mut host, None, 100.0, true);
    add_div(&mut host, Some(container), 300.0, false);
    host
}

/// Node 0 scrolls 200px over node 2, which scrolls 200px over node 3, and
/// node 4 fills the rest of node 0.
fn nested_containers() -> Rig {
    let mut host = Rig::default();
    let outer = add_div(&mut host, None, 100.0, true);
    let inner = add_div(&mut host, Some(outer), 100.0, true);
    add_div(&mut host, Some(inner), 300.0, false);
    add_div(&mut host, Some(outer), 200.0, false);
    host
}

/// A container of 100x100 with `styles` over a 300x300 child.
fn styled_container(styles: &[(&str, &str)]) -> Rig {
    let host = Rig::default();
    let root = host.document.borrow_mut().root();
    set_style(&mut host.document.borrow_mut().tree, root, "width", 100.0).unwrap();
    set_style(&mut host.document.borrow_mut().tree, root, "height", 100.0).unwrap();
    for (key, value) in styles {
        set_style(&mut host.document.borrow_mut().tree, root, *key, *value).unwrap();
    }
    let child = host.document.borrow_mut().tree.create_node("div").unwrap();
    set_style(&mut host.document.borrow_mut().tree, child, "width", 300.0).unwrap();
    set_style(&mut host.document.borrow_mut().tree, child, "height", 300.0).unwrap();
    host.document
        .borrow_mut()
        .tree
        .append_child(root, child)
        .unwrap();
    host
}

mod events;
mod reveal;
mod wheel;
