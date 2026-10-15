// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

//! Shared setup for tests: an engine wired to a [`Host`] through the bindings,
//! with `__inca_test_listen__` for registering listeners, and a GPUI root that
//! renders the host's tree.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Context, Render, TestAppContext, Window, WindowHandle, prelude::*};
use inca_gpui::{NodeId, render_tree_with_events};
use inca_jsenv::{Engine, EngineError, ErrorReporter};

use crate::bindings::install;
use crate::{EventDispatcher, EventListeners, Host};

/// Defines `__inca_test_listen__(node, type, fn, capture, passive)`, which
/// registers `fn` through `__inca_native__.addEventListener`.
pub const LISTEN_JS: &str = r"
    globalThis.__inca_test_listen__ = (node, type, fn, capture = false, passive = false) => {
        globalThis.__inca_callbacks__ ??= {};
        const id = __inca_native__.addEventListener(node, type, capture, passive);
        globalThis.__inca_callbacks__[id] = fn;
    };
    ";

/// The callback ids of `(node, event)` in registration order.
#[must_use]
pub fn callback_ids(listeners: &EventListeners, node: NodeId, event: &str) -> Vec<u32> {
    listeners
        .listeners_for(node, event)
        .iter()
        .map(|l| l.callback)
        .collect()
}

/// An engine with the bindings installed on a fresh default [`Host`], returned
/// together with that host.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
#[must_use]
pub fn engine_with_bindings() -> (Engine, Rc<RefCell<Host>>) {
    let engine = Engine::new().unwrap();
    let host = Rc::new(RefCell::new(Host::default()));
    engine.with(|ctx| install(&ctx, &host)).unwrap();
    (engine, host)
}

/// An engine with the bindings installed on `host` and [`LISTEN_JS`] evaluated.
///
/// # Panics
///
/// Panics when the engine or the bindings fail to initialise.
pub fn engine_for(host: &Rc<RefCell<Host>>) -> Rc<Engine> {
    let engine = Rc::new(Engine::new().unwrap());
    engine.with(|ctx| install(&ctx, host)).unwrap();
    engine.eval::<()>(LISTEN_JS).unwrap();
    engine
}

/// A window root that renders the subtree at `node` of `host` through
/// `dispatcher`, calling [`EventDispatcher::begin_frame`] on every frame.
pub struct TestRoot {
    /// The host whose tree is rendered.
    pub host: Rc<RefCell<Host>>,
    /// The node the rendering starts at.
    pub node: NodeId,
    /// The dispatcher the rendered elements report to.
    pub dispatcher: EventDispatcher,
}

impl Render for TestRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dispatcher.begin_frame(window, cx);
        let host = self.host.borrow();
        render_tree_with_events(&host.tree, self.node, &self.dispatcher).unwrap()
    }
}

/// Opens a window of [`TestRoot`] over `node` of `host`, dispatching into
/// `engine`, and draws the first frame.
///
/// # Panics
///
/// Panics when the window cannot be updated.
pub fn mount_root(
    cx: &mut TestAppContext,
    host: &Rc<RefCell<Host>>,
    engine: &Rc<Engine>,
    node: NodeId,
) -> WindowHandle<TestRoot> {
    let dispatcher = EventDispatcher::new(Rc::clone(engine), Rc::clone(host));
    let window = cx.add_window(|_, _| TestRoot {
        host: Rc::clone(host),
        node,
        dispatcher,
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    window
}

/// [`mount_root`] followed by activating the window.
///
/// # Panics
///
/// Panics when the window cannot be updated.
pub fn mount_active(
    cx: &mut TestAppContext,
    host: &Rc<RefCell<Host>>,
    engine: &Rc<Engine>,
    node: NodeId,
) -> WindowHandle<TestRoot> {
    let window = mount_root(cx, host, engine, node);
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();
    window
}

/// A host whose tree holds `parent` over `children` nodes, each with
/// `tabindex="0"`. The parent takes focus as well when `parent_focusable` is
/// set.
///
/// # Panics
///
/// Panics when the tree rejects a node.
#[must_use]
pub fn focusable_tree(
    children: usize,
    parent_focusable: bool,
) -> (Rc<RefCell<Host>>, NodeId, Vec<NodeId>) {
    let host = Rc::new(RefCell::new(Host::default()));
    let mut host_mut = host.borrow_mut();
    let parent = host_mut.tree.create_node("div").unwrap();
    let nodes: Vec<NodeId> = (0..children)
        .map(|_| {
            let child = host_mut.tree.create_node("div").unwrap();
            host_mut.tree.append_child(parent, child).unwrap();
            child
        })
        .collect();
    let focusable = parent_focusable
        .then_some(parent)
        .into_iter()
        .chain(nodes.iter().copied());
    for node in focusable {
        host_mut.tree.set_attribute(node, "tabindex", "0").unwrap();
        host_mut.focus.mark_tab_dirty(node);
    }
    drop(host_mut);
    (host, parent, nodes)
}

/// Registers `callback`, a JS function expression, for each of `events` on
/// each of `nodes`.
///
/// # Panics
///
/// Panics when the script fails.
pub fn listen_each(
    engine: &Engine,
    nodes: &[NodeId],
    events: &[&str],
    callback: &str,
    passive: bool,
) {
    for node in nodes {
        for event in events {
            engine
                .eval::<()>(&format!(
                    "__inca_test_listen__({node}, '{event}', {callback}, false, {passive});"
                ))
                .unwrap();
        }
    }
}

/// Every failure a test reporter received, in order.
pub type Reported = Rc<RefCell<Vec<EngineError>>>;

/// A reporter that appends every failure to the returned list.
#[must_use]
pub fn capture_reporter() -> (ErrorReporter, Reported) {
    let reported: Reported = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&reported);
    (
        Rc::new(move |err: &EngineError| sink.borrow_mut().push(err.clone())),
        reported,
    )
}
