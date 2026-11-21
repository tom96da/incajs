// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test helpers that the tests of several modules share.

#![allow(clippy::unwrap_used)]

use crate::element::*;
use crate::event_sink::{EventPayload, EventSink, noop_sink_methods};
use crate::focus::FocusRegistry;
use crate::input::{InputLayout, InputRequest, InputState};
use gpui::Pixels;
use gpui::point;
use gpui::prelude::*;
use gpui::size;
use gpui::{App, MouseUpEvent, ScrollHandle, Window, px};
use gpui::{Context, Render, TestAppContext, VisualTestContext};
use inca_node::tree::AttributeValue;
use inca_node::tree::{NodeId, VirtualTree};
use inca_style::StyleCache;
use inca_style::set_style;
use inca_style::spec::{StyleSpec, build_spec, wheel_axes};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// The root of a tree `rooted_tree` builds: the first node created. The document is node 1.
pub(crate) const ROOT: NodeId = 0;

/// A tree whose document holds the root, with `count` nodes attached under the root, each
/// `tabindex="0"`.
pub(crate) fn rooted_tree(count: usize) -> (VirtualTree, Vec<NodeId>) {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    let document = tree.create_node("#document").unwrap();
    tree.set_root(root);
    tree.set_document(document);
    tree.append_child(document, root).unwrap();
    let ids = (0..count)
        .map(|_| {
            let id = tree.create_node("div").unwrap();
            tree.set_attribute(id, "tabindex", "0").unwrap();
            tree.append_child(root, id).unwrap();
            id
        })
        .collect();
    (tree, ids)
}

pub(crate) fn focus_all(
    cx: &mut gpui::VisualTestContext,
    registry: &mut FocusRegistry,
    tree: &VirtualTree,
    ids: &[NodeId],
    focus: NodeId,
) {
    for &id in ids {
        registry.mark_tab_dirty(id);
    }
    registry.request_focus(focus, false);
    cx.update(|window, cx| {
        let _ = registry.apply_pending(tree, &StyleCache::new(), window, cx);
    });
}

pub(crate) fn tree_with(tag: &str, tabindex: Option<AttributeValue>) -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node(tag).unwrap();
    if let Some(value) = tabindex {
        tree.set_attribute(id, "tabindex", value).unwrap();
    }
    (tree, id)
}

pub(crate) fn num(n: f64) -> AttributeValue {
    AttributeValue::Number(n)
}

pub(crate) fn text(s: &str) -> AttributeValue {
    AttributeValue::String(s.to_owned())
}

/// `(event, target, mouse_target, pressed_target)` of each report.
pub(crate) type Calls = Rc<RefCell<Vec<(String, NodeId, Option<NodeId>, Option<NodeId>)>>>;

// Records what the tracker reports; a release is `"release"`.
#[derive(Clone)]
pub(crate) struct Probe {
    pub(crate) tracks: bool,
    pub(crate) calls: Calls,
    /// `(event, Window::default_prevented)` at each `pointer_input`.
    pub(crate) prevented: Rc<RefCell<Vec<(String, bool)>>>,
    /// `(event, payload)` of each `pointer_input` and `fire`.
    pub(crate) payloads: Rc<RefCell<Vec<(String, EventPayload)>>>,
    pub(crate) layouts: Rc<RefCell<HashMap<NodeId, NodeLayout>>>,
    pub(crate) handles: Rc<RefCell<HashMap<NodeId, ScrollHandle>>>,
    /// The tree the view renders, read for `wheel_axes`.
    pub(crate) tree: Rc<RefCell<VirtualTree>>,
    /// The text state of each `input` node.
    pub(crate) inputs: Rc<RefCell<HashMap<NodeId, InputState>>>,
    /// The requests the platform's text input sent.
    pub(crate) requests: Rc<RefCell<Vec<(NodeId, InputRequest)>>>,
    /// The node whose focus handle `focus_handle` gives.
    pub(crate) focus: Rc<RefCell<Option<(NodeId, gpui::FocusHandle)>>>,
    /// The mode the view renders its scrollbars in.
    pub(crate) mode: Scrollbars,
    /// The mode the view renders in once set, in place of `mode`.
    pub(crate) mode_now: Rc<Cell<Option<Scrollbars>>>,
    /// The `(node, hovered)` of each `hover_changed`.
    pub(crate) hovers: Rc<RefCell<Vec<(NodeId, bool)>>>,
    /// The `(call, node)` of each `bar_pressed` and `bar_released`.
    pub(crate) bars: Rc<RefCell<Vec<(String, NodeId)>>>,
    /// How many times the view rendered.
    pub(crate) renders: Rc<Cell<usize>>,
}

impl EventSink for Probe {
    fn tracks_pointer(&self) -> bool {
        self.tracks
    }

    fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        _: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.payloads
            .borrow_mut()
            .push((event.to_owned(), payload.clone()));
        self.calls
            .borrow_mut()
            .push((format!("fire {event}"), start, mouse_target(cx), None));
        false
    }

    fn focus_handle(&self, id: NodeId) -> Option<gpui::FocusHandle> {
        let focus = self.focus.borrow();
        let (node, handle) = focus.as_ref()?;
        (*node == id).then(|| handle.clone())
    }

    fn scroll_handle(&self, id: NodeId) -> Option<ScrollHandle> {
        Some(self.handles.borrow_mut().entry(id).or_default().clone())
    }

    fn wheel_axes(&self, id: NodeId) -> Option<inca_style::WheelAxes> {
        wheel_axes(&self.tree.borrow(), &StyleCache::new(), id)
    }

    fn clear_layout(&self) {
        self.layouts.borrow_mut().clear();
    }

    fn record_layout(&self, id: NodeId, layout: NodeLayout) {
        self.layouts.borrow_mut().insert(id, layout);
    }

    fn pointer_input(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        let button = match payload {
            EventPayload::Mouse(mouse) => mouse.button,
            _ => 0,
        };
        self.calls.borrow_mut().push((
            event.to_owned(),
            target,
            mouse_target(cx),
            pressed_target(cx, button),
        ));
        self.prevented
            .borrow_mut()
            .push((event.to_owned(), window.default_prevented()));
        self.payloads
            .borrow_mut()
            .push((event.to_owned(), payload.clone()));
    }

    fn pointer_released(
        &self,
        _: &MouseUpEvent,
        pressed: Option<NodeId>,
        released: NodeId,
        _: &mut Window,
        _: &mut App,
    ) {
        self.calls
            .borrow_mut()
            .push(("release".to_owned(), released, Some(released), pressed));
    }

    fn hover_changed(&self, id: NodeId, hovered: bool, _: &mut Window, _: &mut App) {
        self.hovers.borrow_mut().push((id, hovered));
    }

    fn bar_pressed(&self, id: NodeId, _: &EventPayload, _: &mut Window, _: &mut App) {
        self.bars.borrow_mut().push(("bar_pressed".to_owned(), id));
    }

    fn bar_released(&self, id: NodeId, _: &EventPayload, _: &mut Window, _: &mut App) {
        self.bars.borrow_mut().push(("bar_released".to_owned(), id));
    }

    fn bar_drag(&self, id: NodeId, active: bool, window: &mut Window, _: &mut App) {
        let name = if active { "bar_drag" } else { "bar_drag_end" };
        self.bars.borrow_mut().push((name.to_owned(), id));
        if !active {
            // The bridge restyles at the end of a drag, which requests a redraw.
            window.refresh();
        }
    }

    fn input(&self, id: NodeId) -> Option<InputState> {
        self.inputs.borrow().get(&id).cloned()
    }

    fn record_input(&self, id: NodeId, layout: InputLayout) {
        self.inputs
            .borrow_mut()
            .entry(id)
            .or_default()
            .record(layout);
    }

    fn input_request(&self, id: NodeId, request: InputRequest, _: &mut Window, _: &mut App) {
        self.requests.borrow_mut().push((id, request));
    }

    noop_sink_methods!(
        pointer_moved,
        pointer_left,
        modifiers_changed,
        pointer_over,
        key_input,
    );
}

pub(crate) struct ProbeView(pub(crate) Probe);

impl Render for ProbeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.renders.set(self.0.renders.get() + 1);
        let mode = self.0.mode_now.get().unwrap_or(self.0.mode);
        render_tree_with_events(&self.0.tree.borrow(), &StyleCache::new(), 0, &self.0, mode)
            .unwrap()
    }
}

pub(crate) struct Scene {
    pub(crate) cx: VisualTestContext,
    pub(crate) calls: Calls,
    pub(crate) prevented: Rc<RefCell<Vec<(String, bool)>>>,
    pub(crate) payloads: Rc<RefCell<Vec<(String, EventPayload)>>>,
    pub(crate) focus: Rc<RefCell<Option<(NodeId, gpui::FocusHandle)>>>,
}

pub(crate) fn probe() -> Probe {
    Probe {
        tracks: true,
        calls: Calls::default(),
        prevented: Rc::default(),
        payloads: Rc::default(),
        layouts: Rc::default(),
        handles: Rc::default(),
        tree: Rc::default(),
        inputs: Rc::default(),
        requests: Rc::default(),
        focus: Rc::default(),
        mode: Scrollbars::Overlay,
        mode_now: Rc::default(),
        hovers: Rc::default(),
        bars: Rc::default(),
        renders: Rc::default(),
    }
}

/// A probe whose view renders `tree`.
pub(crate) fn probe_of(tree: VirtualTree) -> Probe {
    Probe {
        tree: Rc::new(RefCell::new(tree)),
        ..probe()
    }
}

// Node 0 (200x200) holds node 1 (100x100), which holds node 2 (50x50),
// all at the origin.
pub(crate) fn probe_tree(cx: &mut TestAppContext, tracks: bool) -> Scene {
    let mut tree = VirtualTree::new();
    let ids: Vec<_> = (0..3).map(|_| tree.create_node("div").unwrap()).collect();
    for (id, side) in ids.iter().zip([200.0, 100.0, 50.0]) {
        set_style(&mut tree, *id, "width", side).unwrap();
        set_style(&mut tree, *id, "height", side).unwrap();
    }
    tree.append_child(ids[0], ids[1]).unwrap();
    tree.append_child(ids[1], ids[2]).unwrap();
    let mut probe = probe_of(tree);
    probe.tracks = tracks;
    let calls = Rc::clone(&probe.calls);
    let prevented = Rc::clone(&probe.prevented);
    let payloads = Rc::clone(&probe.payloads);
    let focus = Rc::clone(&probe.focus);
    let window = cx.add_window(|_, _| ProbeView(probe));
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    Scene {
        cx: VisualTestContext::from_window(window.into(), cx),
        calls,
        prevented,
        payloads,
        focus,
    }
}

/// The entities of type `T` that [`capture_entities`] collected.
pub(crate) type Entities<T> = Rc<RefCell<Vec<gpui::Entity<T>>>>;

/// Every entity of type `T` created from now on, in creation order. Call it before the first draw
/// of the window.
pub(crate) fn capture_entities<T: 'static>(cx: &mut TestAppContext) -> Entities<T> {
    let entities = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&entities);
    cx.update(|cx: &mut App| {
        cx.observe_new(move |_: &mut T, _, cx| seen.borrow_mut().push(cx.entity()))
            .detach();
    });
    entities
}

/// A 100x100 root styled `root_props` that holds a 300x300 child (node 1).
pub(crate) fn bar_tree(root_props: &[(&str, &str)]) -> VirtualTree {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    for (key, value) in root_props {
        set_style(&mut tree, root, *key, *value).unwrap();
    }
    boxed(&mut tree, Some(root), 300., 300.);
    tree
}

/// A 200x200 root (node 0) that holds a 100x100 scroller styled `props` (node 1) and a 50x50
/// sibling (node 3) below it. The scroller holds a 300x300 child (node 2).
pub(crate) fn nested_bar_tree(props: &[(&str, &str)]) -> VirtualTree {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 200., 200.);
    let scroller = boxed(&mut tree, Some(root), 100., 100.);
    for (key, value) in props {
        set_style(&mut tree, scroller, *key, *value).unwrap();
    }
    boxed(&mut tree, Some(scroller), 300., 300.);
    boxed(&mut tree, Some(root), 50., 50.);
    tree
}

/// A window that renders the probe's tree in the probe's mode, drawn once.
pub(crate) struct BarScene {
    pub(crate) probe: Probe,
    pub(crate) cx: VisualTestContext,
}

impl BarScene {
    pub(crate) fn new(cx: &mut TestAppContext, mode: Scrollbars, tree: VirtualTree) -> Self {
        Self::with_probe(
            cx,
            Probe {
                mode,
                ..probe_of(tree)
            },
        )
    }

    pub(crate) fn with_probe(cx: &mut TestAppContext, probe: Probe) -> Self {
        let view = probe.clone();
        let window = cx.add_window(|_, _| ProbeView(view));
        let mut scene = Self {
            probe,
            cx: VisualTestContext::from_window(window.into(), cx),
        };
        scene.frame();
        scene
    }

    pub(crate) fn frame(&mut self) {
        self.cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// The scroll handle of `node`, which the first frame created.
    pub(crate) fn handle(&self, node: NodeId) -> ScrollHandle {
        self.probe.handles.borrow()[&node].clone()
    }

    /// Moves the pointer to `(x, y)` with no button held.
    pub(crate) fn move_to(&mut self, (x, y): (f32, f32)) {
        self.cx
            .simulate_mouse_move(point(px(x), px(y)), None, gpui::Modifiers::none());
    }

    /// Moves the pointer to `(x, y)` with the left button held.
    pub(crate) fn drag_to(&mut self, (x, y): (f32, f32)) {
        self.cx.simulate_mouse_move(
            point(px(x), px(y)),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
    }

    pub(crate) fn press(&mut self, (x, y): (f32, f32), button: gpui::MouseButton) {
        self.cx
            .simulate_mouse_down(point(px(x), px(y)), button, gpui::Modifiers::none());
    }

    pub(crate) fn release(&mut self, (x, y): (f32, f32), button: gpui::MouseButton) {
        self.cx
            .simulate_mouse_up(point(px(x), px(y)), button, gpui::Modifiers::none());
    }

    /// One wheel step of `delta` at `(x, y)`, then a draw.
    pub(crate) fn wheel(&mut self, at: (f32, f32), delta: f32) {
        self.wheel_undrawn(at, delta);
        self.frame();
    }

    /// One wheel step of `delta` at `(x, y)`. The window draws what the step refreshes.
    pub(crate) fn wheel_undrawn(&mut self, (x, y): (f32, f32), delta: f32) {
        self.cx.simulate_event(gpui::ScrollEvent {
            position: point(px(x), px(y)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(delta))),
            ..Default::default()
        });
    }

    /// The events the probe saw, as `(event, target)` pairs.
    pub(crate) fn events(&self) -> Vec<(String, NodeId)> {
        let calls = self.probe.calls.borrow();
        calls.iter().map(|c| (c.0.clone(), c.1)).collect()
    }
}

pub(crate) fn boxed(tree: &mut VirtualTree, parent: Option<NodeId>, w: f64, h: f64) -> NodeId {
    let id = tree.create_node("div").unwrap();
    set_style(tree, id, "width", w).unwrap();
    set_style(tree, id, "height", h).unwrap();
    if let Some(parent) = parent {
        tree.append_child(parent, id).unwrap();
    }
    id
}

/// Bounds of a container styled `root_props` (first) and its `children`
/// (at most two), in node order.
pub(crate) fn layout(
    cx: &mut TestAppContext,
    root_props: &[(&str, AttributeValue)],
    children: &[&[(&str, AttributeValue)]],
) -> Vec<gpui::Bounds<Pixels>> {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    for (key, value) in root_props {
        set_style(&mut tree, root, *key, value.clone()).unwrap();
    }
    for props in children {
        let child = tree.create_node("div").unwrap();
        for (key, value) in *props {
            set_style(&mut tree, child, *key, value.clone()).unwrap();
        }
        tree.append_child(root, child).unwrap();
    }
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        render_tree(&tree, &StyleCache::new(), root).unwrap()
    });
    ["node-0", "node-1", "node-2"][..=children.len()]
        .iter()
        .map(|name| cx.debug_bounds(name).unwrap())
        .collect()
}

pub(crate) fn mounted(
    cx: &mut TestAppContext,
    tree: VirtualTree,
) -> (Probe, gpui::AnyWindowHandle) {
    let probe = probe_of(tree);
    let view = probe.clone();
    let window = cx.add_window(|_, _| ProbeView(view));
    (probe, window.into())
}

pub(crate) fn frame(cx: &mut TestAppContext, window: gpui::AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

pub(crate) fn draw(cx: &mut gpui::VisualTestContext, tree: &VirtualTree, probe: &Probe) {
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        render_tree_with_events(tree, &StyleCache::new(), 0, probe, probe.mode).unwrap()
    });
}

pub(crate) fn rect(x: f32, y: f32, w: f32, h: f32) -> gpui::Bounds<Pixels> {
    gpui::Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

pub(crate) fn style_of(props: &[(&str, AttributeValue)]) -> StyleSpec {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        set_style(&mut tree, id, *key, value.clone()).unwrap();
    }
    build_spec(&tree, &StyleCache::new(), id).unwrap().style
}
