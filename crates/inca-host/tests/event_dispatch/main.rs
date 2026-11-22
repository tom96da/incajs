// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Wiring tests of the pointer: the wiring of JS and the window with a real `VirtualTree`,
//! connected to a real `QuickJS` callback through `inca_host::wiring::Wired`, rendered through
//! `render_tree_with_events`.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

use std::rc::Rc;

use gpui::{
    Context, Modifiers, MouseButton, MouseDownEvent, MouseExitEvent, MouseUpEvent, Render,
    ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, Window, point, prelude::*, px,
};
use inca_gpui::{Scrollbars, render_tree_with_events};
use inca_host::wiring::{self, Rig, Wired};
use inca_jsenv::{Engine, EngineError};
use inca_node::NodeId;

use inca_style::set_style;

/// Counts `event` on `node` into `globalThis.clicks` and keeps the last one
/// in `globalThis.lastEvent`.
fn install_click_counter(engine: &Engine, node: NodeId, event: &str) {
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

fn clicks(engine: &Engine) -> f64 {
    engine.eval::<f64>("globalThis.clicks || 0").unwrap()
}

struct ClickableRoot {
    host: Rig,
    node: NodeId,
    dispatcher: inca_window::EventDispatcher,
}

impl Render for ClickableRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.document.borrow();
        render_tree_with_events(
            &host.tree,
            &host.style,
            self.node,
            &self.dispatcher,
            Scrollbars::Overlay,
        )
        .unwrap()
    }
}

fn build_clickable_tree() -> (Rig, NodeId) {
    let host = Rig::default();
    let mut host_mut = host.document.borrow_mut();
    let node = host_mut.tree.create_node("div").unwrap();
    set_style(&mut host_mut.tree, node, "width", 100.0).unwrap();
    set_style(&mut host_mut.tree, node, "height", 100.0).unwrap();
    drop(host_mut);
    (host, node)
}

/// Mounts `node` of `host`, dispatching into `engine`, and draws the first
/// frame.
fn mount_clickable(
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
struct Panes {
    host: Rig,
    root: NodeId,
    a: NodeId,
    b: NodeId,
    engine: Rc<Engine>,
    dispatcher: Wired,
}

fn build_panes(a_events: &[&str], root_events: &[&str]) -> Panes {
    build_panes_recording(
        a_events,
        root_events,
        "(e) => { globalThis.seen.push([e.type, e.movementX, e.movementY]); }",
    )
}

/// [`build_panes`] with `record`, a JS function, as every listener.
fn build_panes_recording(a_events: &[&str], root_events: &[&str], record: &str) -> Panes {
    let host = Rig::default();
    let (root, a, b) = {
        let mut host = host.document.borrow_mut();
        let root = host.tree.create_node("div").unwrap();
        set_style(&mut host.tree, root, "display", "flex").unwrap();
        set_style(&mut host.tree, root, "flex_direction", "row").unwrap();
        set_style(&mut host.tree, root, "width", 300.0).unwrap();
        set_style(&mut host.tree, root, "height", 100.0).unwrap();
        let mut pane = || {
            let pane = host.tree.create_node("div").unwrap();
            set_style(&mut host.tree, pane, "width", 100.0).unwrap();
            set_style(&mut host.tree, pane, "height", 100.0).unwrap();
            host.tree.append_child(root, pane).unwrap();
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
fn mount_panes(cx: &mut TestAppContext, panes: &Panes) -> VisualTestContext {
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
fn mount_recording_pair(
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
fn mount_recording_pair_with(
    cx: &mut TestAppContext,
    events: &[&str],
    record: &str,
) -> (VisualTestContext, Rc<Engine>, NodeId, NodeId) {
    let host = Rig::default();
    let (parent, child) = {
        let mut host = host.document.borrow_mut();
        let parent = host.tree.create_node("div").unwrap();
        set_style(&mut host.tree, parent, "width", 100.0).unwrap();
        set_style(&mut host.tree, parent, "height", 100.0).unwrap();
        let child = host.tree.create_node("div").unwrap();
        set_style(&mut host.tree, child, "width", 100.0).unwrap();
        set_style(&mut host.tree, child, "height", 100.0).unwrap();
        host.tree.append_child(parent, child).unwrap();
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

fn take_ids(engine: &Engine) -> Vec<(String, NodeId, u64)> {
    let seen = engine
        .eval::<String>("JSON.stringify(globalThis.seen.splice(0))")
        .unwrap();
    serde_json::from_str(&seen).unwrap()
}

/// The fields of `globalThis.lastEvent` that `want` names, as the listener
/// read them.
fn event_fields(engine: &Engine, want: &serde_json::Value) -> serde_json::Value {
    let keys =
        serde_json::to_string(&want.as_object().unwrap().keys().collect::<Vec<_>>()).unwrap();
    let json = engine
        .eval::<String>(&format!(
            "JSON.stringify(Object.fromEntries({keys}.map((k) => [k, globalThis.lastEvent[k]])))"
        ))
        .unwrap();
    serde_json::from_str(&json).unwrap()
}

mod click;
mod disabled;
mod fields;
mod hover;
mod ids;
mod movement;
mod press;
