// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for `inca_bridge::dispatch`'s event dispatch: a real
//! `VirtualTree`, connected to a real `QuickJS` callback via the
//! `__inca_callbacks__` convention (see `EventDispatcher`'s doc comment),
//! rendered through `render_tree_with_events`.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, Modifiers, MouseButton, MouseDownEvent, MouseExitEvent, MouseUpEvent, Render,
    ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, Window, point, prelude::*,
    px,
};
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{NodeId, render_tree_with_events};
use inca_jsenv::{Engine, EngineError};

fn install_click_counter(engine: &Engine) {
    engine
        .eval::<()>(
            r"
            globalThis.__inca_callbacks__ = {
                0: (event) => {
                    globalThis.clicks = (globalThis.clicks || 0) + 1;
                    globalThis.lastEvent = event;
                }
            };
            ",
        )
        .unwrap();
}

fn clicks(engine: &Engine) -> f64 {
    engine.eval::<f64>("globalThis.clicks || 0").unwrap()
}

struct ClickableRoot {
    host: Rc<RefCell<Host>>,
    node: NodeId,
    dispatcher: EventDispatcher,
}

impl Render for ClickableRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher).unwrap()
    }
}

fn build_tree_listening_for(event: &str) -> (Rc<RefCell<Host>>, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let node = host_mut.tree.create_node("div").unwrap();
    host_mut.tree.set_style(node, "width", 100.0).unwrap();
    host_mut.tree.set_style(node, "height", 100.0).unwrap();
    host_mut.listeners.register(node, event, 0);
    drop(host_mut);
    (host, node)
}

/// A row 300 wide holding pane A (0..100) and pane B (100..200), each 100x100.
/// Callback 0 records `[type, movementX, movementY]` into `globalThis.seen`.
/// `a_events` are registered on A and `root_events` on the row; B has no
/// listener.
struct Panes {
    host: Rc<RefCell<Host>>,
    root: NodeId,
    a: NodeId,
    b: NodeId,
    engine: Rc<Engine>,
    dispatcher: EventDispatcher,
}

fn build_panes(a_events: &[&str], root_events: &[&str]) -> Panes {
    let host = Rc::new(RefCell::new(Host::default()));
    let (root, a, b) = {
        let mut host = host.borrow_mut();
        let root = host.tree.create_node("div").unwrap();
        host.tree.set_style(root, "display", "flex").unwrap();
        host.tree.set_style(root, "flex_direction", "row").unwrap();
        host.tree.set_style(root, "width", 300.0).unwrap();
        host.tree.set_style(root, "height", 100.0).unwrap();
        let mut pane = || {
            let pane = host.tree.create_node("div").unwrap();
            host.tree.set_style(pane, "width", 100.0).unwrap();
            host.tree.set_style(pane, "height", 100.0).unwrap();
            host.tree.append_child(root, pane).unwrap();
            pane
        };
        let (a, b) = (pane(), pane());
        for event in a_events {
            host.listeners.register(a, *event, 0);
        }
        for event in root_events {
            host.listeners.register(root, *event, 0);
        }
        (root, a, b)
    };
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            r"
            globalThis.seen = [];
            globalThis.__inca_callbacks__ = { 0: (e) => {
                globalThis.seen.push([e.type, e.movementX, e.movementY]); } };
            ",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
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
fn mount_panes(cx: &mut TestAppContext, panes: &Panes) -> VisualTestContext {
    let window = cx.add_window(|_, _| ClickableRoot {
        host: Rc::clone(&panes.host),
        node: panes.root,
        dispatcher: panes.dispatcher.clone(),
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    VisualTestContext::from_window(window.into(), cx)
}

/// A parent over a child, both listening for `events`; callback 0 records
/// `[type, currentTarget, eventId]` into `globalThis.seen`.
fn mount_recording_pair(
    cx: &mut TestAppContext,
    events: &[&str],
) -> (VisualTestContext, Rc<Engine>, NodeId, NodeId) {
    let host = Rc::new(RefCell::new(Host::default()));
    let (parent, child) = {
        let mut host = host.borrow_mut();
        let parent = host.tree.create_node("div").unwrap();
        host.tree.set_style(parent, "width", 100.0).unwrap();
        host.tree.set_style(parent, "height", 100.0).unwrap();
        let child = host.tree.create_node("div").unwrap();
        host.tree.set_style(child, "width", 100.0).unwrap();
        host.tree.set_style(child, "height", 100.0).unwrap();
        host.tree.append_child(parent, child).unwrap();
        for event in events {
            host.listeners.register(parent, *event, 0);
            host.listeners.register(child, *event, 0);
        }
        (parent, child)
    };
    let engine = Rc::new(Engine::new().unwrap());
    engine
        .eval::<()>(
            r"
            globalThis.seen = [];
            globalThis.__inca_callbacks__ = { 0: (e) => {
                globalThis.seen.push([e.type, e.currentTarget, e.eventId]); } };
            ",
        )
        .unwrap();
    let dispatcher = EventDispatcher::new(Rc::clone(&engine), Rc::clone(&host));
    let window = cx.add_window(|_, _| ClickableRoot {
        host,
        node: parent,
        dispatcher,
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

fn take_ids(engine: &Engine) -> Vec<(String, NodeId, u64)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

mod click;
mod fields;
mod hover;
mod ids;
mod movement;
mod press;
