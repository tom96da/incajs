// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

//! Shared setup for tests: cells over a default document, and a GPUI root that renders their
//! tree through a dispatcher.

use std::cell::RefCell;
use std::rc::Rc;

use async_channel::Sender;
use gpui::{Context, Render, TestAppContext, Window, WindowHandle, point, prelude::*, px};
use inca_document::{DocumentState, EventPayload, MousePayload, PointerSource, WheelPayload};
use inca_gpui::{Scrollbars, render_tree_with_events};
use inca_node::NodeId;

use crate::{Cells, EventDispatcher, WindowState};

/// Cells over a default document and a new window state, with the sender of the channel the
/// timer pump of a dispatcher waits on.
#[must_use]
pub fn cells() -> (Rc<Cells>, Sender<()>) {
    let (sender, wake) = async_channel::bounded(1);
    let cells = Cells {
        document: Rc::new(RefCell::new(DocumentState::default())),
        window: Rc::new(RefCell::new(WindowState::new(wake))),
    };
    (Rc::new(cells), sender)
}

/// A window root that renders the subtree at `node` of `cells` through `dispatcher`, calling
/// [`EventDispatcher::begin_frame`] on every frame.
pub struct TestRoot {
    /// The cells whose tree is rendered.
    pub cells: Rc<Cells>,
    /// The node the rendering starts at.
    pub node: NodeId,
    /// The dispatcher the rendered elements report to.
    pub dispatcher: EventDispatcher,
}

impl Render for TestRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.dispatcher.begin_frame(window, cx);
        let document = self.cells.document.borrow();
        render_tree_with_events(
            &document.tree,
            &document.style,
            self.node,
            &self.dispatcher,
            Scrollbars::Overlay,
        )
        .unwrap()
    }
}

/// Opens a window of [`TestRoot`] over `node` of the cells of `dispatcher` and draws the first
/// frame.
///
/// # Panics
///
/// Panics when the window cannot be updated.
pub fn mount_root(
    cx: &mut TestAppContext,
    dispatcher: &EventDispatcher,
    node: NodeId,
) -> WindowHandle<TestRoot> {
    let cells = Rc::clone(&dispatcher.cells);
    let dispatcher = dispatcher.clone();
    let window = cx.add_window(|_, _| TestRoot {
        cells,
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
    dispatcher: &EventDispatcher,
    node: NodeId,
) -> WindowHandle<TestRoot> {
    let window = mount_root(cx, dispatcher, node);
    cx.update_window(window.into(), |_, window, _cx| window.activate_window())
        .unwrap();
    window
}

#[must_use]
pub fn at(x: f32, y: f32) -> gpui::Point<gpui::Pixels> {
    point(px(x), px(y))
}

#[must_use]
pub fn mouse_at(x: f32, y: f32) -> EventPayload {
    EventPayload::Mouse(MousePayload::at(x, y, inca_document::Modifiers::none()))
}

/// A `click` from the mouse pointer.
#[must_use]
pub fn pointer_click() -> EventPayload {
    EventPayload::Mouse(MousePayload {
        pointer: Some(PointerSource::Mouse),
        ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
    })
}

#[must_use]
pub fn wheel_at(x: f32, y: f32) -> EventPayload {
    EventPayload::Wheel(WheelPayload {
        mouse: MousePayload::at(x, y, inca_document::Modifiers::none()),
        delta_x: 0.0,
        delta_y: 1.0,
        delta_z: 0.0,
        delta_mode: 0,
    })
}

#[must_use]
pub fn key_payload(modifiers: inca_document::Modifiers) -> EventPayload {
    EventPayload::Key(inca_document::KeyPayload {
        key: "a".into(),
        repeat: false,
        modifiers,
        is_composing: false,
    })
}

/// Test setup for the dispatch of a window.
///
/// # Panics
///
/// Panics when the tree rejects a node.
#[must_use]
pub fn tree_of_three(cells: &Rc<Cells>) -> [NodeId; 3] {
    let mut h = cells.document.borrow_mut();
    let outer = h.tree.create_node("div").unwrap();
    let parent = h.tree.create_node("div").unwrap();
    let child = h.tree.create_node("div").unwrap();
    h.tree.append_child(outer, parent).unwrap();
    h.tree.append_child(parent, child).unwrap();
    [outer, parent, child]
}

/// A new node under the document root, so an event at it also reaches document and window.
///
/// # Panics
///
/// Panics when the tree rejects a node.
#[must_use]
pub fn node_under_root(cells: &Rc<Cells>) -> NodeId {
    let mut h = cells.document.borrow_mut();
    let node = h.tree.create_node("div").unwrap();
    let root = h.root();
    h.tree.append_child(root, node).unwrap();
    node
}

/// Whether `node` matches the state class `class` in the drawn interaction state.
///
/// # Panics
///
/// Panics when the tree rejects a node.
#[must_use]
pub fn state(document: &DocumentState, node: NodeId, class: &str) -> bool {
    inca_style::matches_selector(&document.tree, node, class, inca_style::MatchFocus::Drawn)
        .unwrap()
}

/// Appends a `<style>` element that holds `css` under the root.
///
/// # Panics
///
/// Panics when the tree rejects a node.
pub fn add_sheet(cells: &Cells, css: &str) {
    let document = &mut *cells.document.borrow_mut();
    let [style, text] = ["style", "#text"].map(|tag| document.tree.create_node(tag).unwrap());
    document.tree.set_data(text, css).unwrap();
    document.tree.append_child(style, text).unwrap();
    document.tree.append_child(document.root(), style).unwrap();
}

#[must_use]
pub fn is_focused(document: &DocumentState, node: NodeId) -> bool {
    state(document, node, ":focus")
}

/// A view whose render runs `begin_frame`, as a window's render does.
pub struct Frame(pub Rc<EventDispatcher>);

impl gpui::Render for Frame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.0.begin_frame(window, cx);
        gpui::div()
    }
}

/// Test setup for the dispatch of a window.
///
/// # Panics
///
/// Panics when the tree rejects a node.
pub fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<Frame>) {
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

/// Applies the queued focus requests of `cells` as a frame does, and returns the transitions.
pub fn apply_focus(
    cells: &Cells,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Vec<inca_document::FocusTransition> {
    cells.apply_focus(window, cx)
}
