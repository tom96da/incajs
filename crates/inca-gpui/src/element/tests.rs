// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::style::apply_style;
use super::*;
use crate::event_sink::{EventPayload, EventSink};
use crate::tree::AttributeValue;
use crate::tree::{NodeId, VirtualTree};
use gpui::prelude::*;
use gpui::{App, MouseUpEvent, ScrollHandle, Window, px};
use gpui::{Length, StyleRefinement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Context, Pixels, Point, Render, TestAppContext, VisualTestContext};

mod layout;
mod pointer;
mod records;
mod spec;
mod style;

fn num(n: f64) -> AttributeValue {
    AttributeValue::Number(n)
}

fn text(s: &str) -> AttributeValue {
    AttributeValue::String(s.to_owned())
}

/// `(event, target, mouse_target, pressed_target)` of each report.
type Calls = Rc<RefCell<Vec<(String, NodeId, Option<NodeId>, Option<NodeId>)>>>;

// Records what the tracker reports; a release is `"release"`.
#[derive(Clone)]
struct Probe {
    tracks: bool,
    calls: Calls,
    /// `(event, Window::default_prevented)` at each `pointer_input`.
    prevented: Rc<RefCell<Vec<(String, bool)>>>,
    layouts: Rc<RefCell<HashMap<NodeId, NodeLayout>>>,
    handles: Rc<RefCell<HashMap<NodeId, ScrollHandle>>>,
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

    fn pointer_moved(&self, _: Point<Pixels>, _: &mut App) {}

    fn pointer_left(&self) {}

    fn modifiers_changed(&self, _: gpui::Modifiers, _: &mut Window, _: &mut App) {}

    fn hover_changed(&self, _: NodeId, _: bool, _: &mut Window, _: &mut App) {}

    fn pointer_over(&self, _: NodeId, _: &mut Window, _: &mut App) {}

    fn pointer_pressed(&self) {}

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

    fn key_input(
        &self,
        _: &gpui::Keystroke,
        _: bool,
        _: &EventPayload,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

struct ProbeView(VirtualTree, Probe);

impl Render for ProbeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        render_tree_with_events(&self.0, 0, &self.1).unwrap()
    }
}

struct Scene {
    cx: VisualTestContext,
    calls: Calls,
    prevented: Rc<RefCell<Vec<(String, bool)>>>,
}

fn probe() -> Probe {
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
fn probe_tree(cx: &mut TestAppContext, tracks: bool) -> Scene {
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
