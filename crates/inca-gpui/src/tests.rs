// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Test helpers that the tests of several modules share.

#![allow(clippy::unwrap_used)]

use crate::element::*;
use crate::event_sink::{EventPayload, EventSink, noop_sink_methods};
use crate::focus::FocusRegistry;
use crate::input::{InputLayout, InputRequest, InputState};
use crate::selector::{closest_selector, matches_selector};
use crate::tree::{AttributeValue, TEXT_TAG};
use crate::tree::{NodeId, VirtualTree};
use gpui::Pixels;
use gpui::point;
use gpui::prelude::*;
use gpui::size;
use gpui::{App, MouseUpEvent, ScrollHandle, Window, px};
use gpui::{Context, Render, TestAppContext, VisualTestContext};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A tree under construction. Every node an `add` call makes gets an `id` attribute of `name`.
pub(crate) struct Fixture {
    pub(crate) tree: VirtualTree,
    pub(crate) root: NodeId,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.set_attribute(root, "id", "root").unwrap();
        Fixture { tree, root }
    }

    pub(crate) fn add(&mut self, parent: NodeId, tag: &str, name: &str) -> NodeId {
        let node = self.tree.create_node(tag).unwrap();
        self.tree.append_child(parent, node).unwrap();
        self.tree.set_attribute(node, "id", name).unwrap();
        node
    }

    pub(crate) fn attr(&mut self, node: NodeId, key: &str, value: impl Into<AttributeValue>) {
        self.tree.set_attribute(node, key, value).unwrap();
    }

    /// A `<style>` element under the root that holds `css`, and its text node.
    pub(crate) fn sheet(&mut self, css: &str) -> (NodeId, NodeId) {
        let style = self.add(self.root, "style", "sheet");
        let text_node = self.tree.create_node("#text").unwrap();
        self.attr(text_node, "value", css);
        self.tree.append_child(style, text_node).unwrap();
        (style, text_node)
    }

    /// A `div` under the root with `name` as its id and its class.
    pub(crate) fn boxed(&mut self, name: &str) -> NodeId {
        let node = self.add(self.root, "div", name);
        self.attr(node, "class", name);
        node
    }

    pub(crate) fn computed(&self, node: NodeId, key: &str) -> Option<AttributeValue> {
        self.tree.computed_style(node).get(key).cloned()
    }

    pub(crate) fn matches(&self, node: NodeId, selector: &str) -> bool {
        matches_selector(&self.tree, node, selector).unwrap()
    }

    pub(crate) fn closest(&self, node: NodeId, selector: &str) -> Option<NodeId> {
        closest_selector(&self.tree, node, selector).unwrap()
    }
}

/// `root > main > (p1, p2)` and `root > foot`, with the attributes the selector rows read.
pub(crate) struct Page {
    pub(crate) f: Fixture,
    pub(crate) main: NodeId,
    pub(crate) p1: NodeId,
    pub(crate) p2: NodeId,
    pub(crate) p3: NodeId,
    pub(crate) foot: NodeId,
}

pub(crate) fn page() -> Page {
    let mut f = Fixture::new();
    let main = f.add(f.root, "section", "main");
    f.attr(main, "class", "card");
    f.attr(main, "data-k", "ab-cd ef");
    let p1 = f.add(main, "p", "p1");
    let p2 = f.add(main, "span", "p2");
    let p3 = f.add(main, "p", "p3");
    let foot = f.add(f.root, "footer", "foot");
    Page {
        f,
        main,
        p1,
        p2,
        p3,
        foot,
    }
}

impl Page {
    pub(crate) fn node(&self, name: &str) -> NodeId {
        match name {
            "root" => self.f.root,
            "main" => self.main,
            "p1" => self.p1,
            "p2" => self.p2,
            "p3" => self.p3,
            _ => self.foot,
        }
    }
}

/// `parent > [comment, text node, a, b]`.
pub(crate) struct Mixed {
    pub(crate) f: Fixture,
    pub(crate) parent: NodeId,
    pub(crate) comment: NodeId,
    pub(crate) a: NodeId,
    pub(crate) text_node: NodeId,
    pub(crate) b: NodeId,
}

pub(crate) fn mixed() -> Mixed {
    let mut f = Fixture::new();
    let parent = f.add(f.root, "div", "parent");
    let comment = f.add(parent, "#comment", "comment");
    let text_node = f.add(parent, "#text", "te");
    f.attr(text_node, "value", "t");
    let a = f.add(parent, "span", "a");
    let b = f.add(parent, "span", "b");
    Mixed {
        f,
        parent,
        comment,
        a,
        text_node,
        b,
    }
}

/// The root of a tree `rooted_tree` builds: the first node created.
pub(crate) const ROOT: NodeId = 0;

/// A rooted tree with `count` nodes attached under the root, each
/// `tabindex="0"`.
pub(crate) fn rooted_tree(count: usize) -> (VirtualTree, Vec<NodeId>) {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
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
        let _ = registry.apply_pending(tree, window, cx);
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

pub(crate) fn style_of(props: &[(&str, AttributeValue)]) -> StyleSpec {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        tree.set_style(id, *key, value.clone()).unwrap();
    }
    build_spec(&tree, id).unwrap().style
}

/// The style a node gets from `props`, and what [`style_warning`] says
/// about each prop, in order.
pub(crate) fn style_and_warnings(props: &[(&str, AttributeValue)]) -> (StyleSpec, Vec<String>) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        tree.set_style(id, *key, value.clone()).unwrap();
    }
    let warnings = props
        .iter()
        .filter_map(|(key, value)| style_warning(key, value))
        .collect();
    (build_spec(&tree, id).unwrap().style, warnings)
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

    fn wheel_axes(&self, id: NodeId) -> Option<gpui::Point<bool>> {
        wheel_axes(&self.tree.borrow(), id)
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
        hover_changed,
        pointer_over,
        key_input,
    );
}

pub(crate) struct ProbeView(pub(crate) Probe);

impl Render for ProbeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        render_tree_with_events(&self.0.tree.borrow(), 0, &self.0).unwrap()
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
        tree.set_style(*id, "width", side).unwrap();
        tree.set_style(*id, "height", side).unwrap();
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

pub(crate) fn boxed(tree: &mut VirtualTree, parent: Option<NodeId>, w: f64, h: f64) -> NodeId {
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "width", w).unwrap();
    tree.set_style(id, "height", h).unwrap();
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
        tree.set_style(root, *key, value.clone()).unwrap();
    }
    for props in children {
        let child = tree.create_node("div").unwrap();
        for (key, value) in *props {
            tree.set_style(child, *key, value.clone()).unwrap();
        }
        tree.append_child(root, child).unwrap();
    }
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        render_tree(&tree, root).unwrap()
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
        render_tree_with_events(tree, 0, probe).unwrap()
    });
}

pub(crate) fn rect(x: f32, y: f32, w: f32, h: f32) -> gpui::Bounds<Pixels> {
    gpui::Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

pub(crate) fn sides<T: Copy>(edges: EdgesSpec<T>) -> [Option<T>; 4] {
    [edges.top, edges.right, edges.bottom, edges.left]
}

pub(crate) fn overflow_of(props: &[(&str, &str)]) -> (Option<OverflowSpec>, Option<OverflowSpec>) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        tree.set_style(id, *key, *value).unwrap();
    }
    let style = build_spec(&tree, id).unwrap().style;
    (style.overflow_x, style.overflow_y)
}

/// A tree node of `tag`.
pub(crate) fn node(tree: &mut VirtualTree, tag: &str) -> NodeId {
    tree.create_node(tag).unwrap()
}

/// A `div` parent with `count` `span` children.
pub(crate) fn family(tree: &mut VirtualTree, count: usize) -> (NodeId, Vec<NodeId>) {
    let parent = node(tree, "div");
    let kids = (0..count)
        .map(|_| {
            let kid = node(tree, "span");
            tree.append_child(parent, kid).unwrap();
            kid
        })
        .collect();
    (parent, kids)
}

pub(crate) fn children(tree: &VirtualTree, id: NodeId) -> Vec<NodeId> {
    tree.get(id).unwrap().children().to_vec()
}

pub(crate) fn not_a_child(parent: NodeId, child: NodeId) -> crate::tree::TreeError {
    crate::tree::TreeError::NotAChild { parent, child }
}

/// `tag[class=a, tabindex=1, width=4] > (span > #text "hi", #text "t")`.
pub(crate) fn sample(tree: &mut VirtualTree, tag: &str) -> NodeId {
    let root = node(tree, tag);
    tree.set_attribute(root, "class", "a").unwrap();
    tree.set_attribute(root, "tabindex", 1.0).unwrap();
    tree.set_style(root, "width", 4.0).unwrap();
    let span = node(tree, "span");
    let inner = node(tree, TEXT_TAG);
    tree.set_attribute(inner, "value", "hi").unwrap();
    let tail = node(tree, TEXT_TAG);
    tree.set_attribute(tail, "value", "t").unwrap();
    tree.append_child(root, span).unwrap();
    tree.append_child(span, inner).unwrap();
    tree.append_child(root, tail).unwrap();
    root
}
