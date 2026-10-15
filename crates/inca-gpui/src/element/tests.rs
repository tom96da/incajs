// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::style::apply_style;
use super::*;
use crate::event_sink::{EventPayload, EventSink, noop_sink_methods};
use crate::tree::AttributeValue;
use crate::tree::{NodeId, VirtualTree};
use gpui::prelude::*;
use gpui::{App, MouseUpEvent, ScrollHandle, Window, px};
use gpui::{Length, StyleRefinement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Context, Render, TestAppContext, VisualTestContext};

mod build;
mod style;

pub(super) fn num(n: f64) -> AttributeValue {
    AttributeValue::Number(n)
}

pub(super) fn text(s: &str) -> AttributeValue {
    AttributeValue::String(s.to_owned())
}

/// `(event, target, mouse_target, pressed_target)` of each report.
pub(super) type Calls = Rc<RefCell<Vec<(String, NodeId, Option<NodeId>, Option<NodeId>)>>>;

// Records what the tracker reports; a release is `"release"`.
#[derive(Clone)]
pub(super) struct Probe {
    pub(super) tracks: bool,
    pub(super) calls: Calls,
    /// `(event, Window::default_prevented)` at each `pointer_input`.
    pub(super) prevented: Rc<RefCell<Vec<(String, bool)>>>,
    pub(super) layouts: Rc<RefCell<HashMap<NodeId, NodeLayout>>>,
    pub(super) handles: Rc<RefCell<HashMap<NodeId, ScrollHandle>>>,
}

impl EventSink for Probe {
    fn tracks_pointer(&self) -> bool {
        self.tracks
    }

    fn fire(
        &self,
        start: NodeId,
        event: &str,
        _: &EventPayload,
        _: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.calls
            .borrow_mut()
            .push((format!("fire {event}"), start, mouse_target(cx), None));
        false
    }

    fn focus_handle(&self, _: NodeId) -> Option<gpui::FocusHandle> {
        None
    }

    fn scroll_handle(&self, id: NodeId) -> Option<ScrollHandle> {
        Some(self.handles.borrow_mut().entry(id).or_default().clone())
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

    noop_sink_methods!(
        pointer_moved,
        pointer_left,
        modifiers_changed,
        hover_changed,
        pointer_over,
        pointer_pressed,
        key_input,
    );
}

pub(super) struct ProbeView(pub(super) VirtualTree, pub(super) Probe);

impl Render for ProbeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        render_tree_with_events(&self.0, 0, &self.1).unwrap()
    }
}

pub(super) struct Scene {
    pub(super) cx: VisualTestContext,
    pub(super) calls: Calls,
    pub(super) prevented: Rc<RefCell<Vec<(String, bool)>>>,
}

pub(super) fn probe() -> Probe {
    Probe {
        tracks: true,
        calls: Calls::default(),
        prevented: Rc::default(),
        layouts: Rc::default(),
        handles: Rc::default(),
    }
}

// Node 0 (200x200) holds node 1 (100x100), which holds node 2 (50x50),
// all at the origin.
pub(super) fn probe_tree(cx: &mut TestAppContext, tracks: bool) -> Scene {
    let mut tree = VirtualTree::new();
    let ids: Vec<_> = (0..3).map(|_| tree.create_node("div").unwrap()).collect();
    for (id, side) in ids.iter().zip([200.0, 100.0, 50.0]) {
        tree.set_style(*id, "width", side).unwrap();
        tree.set_style(*id, "height", side).unwrap();
    }
    tree.append_child(ids[0], ids[1]).unwrap();
    tree.append_child(ids[1], ids[2]).unwrap();
    let mut probe = probe();
    probe.tracks = tracks;
    let calls = Rc::clone(&probe.calls);
    let prevented = Rc::clone(&probe.prevented);
    let window = cx.add_window(|_, _| ProbeView(tree, probe));
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    Scene {
        cx: VisualTestContext::from_window(window.into(), cx),
        calls,
        prevented,
    }
}

pub(super) fn boxed(tree: &mut VirtualTree, parent: Option<NodeId>, w: f64, h: f64) -> NodeId {
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "width", w).unwrap();
    tree.set_style(id, "height", h).unwrap();
    if let Some(parent) = parent {
        tree.append_child(parent, id).unwrap();
    }
    id
}
