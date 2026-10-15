// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::scroll::{Align, Rect, rect_of, scroll_chain, scroll_delta};
use super::style::apply_style;
use super::*;
use crate::event_sink::{EventPayload, EventSink, noop_sink_methods};
use crate::tree::AttributeValue;
use crate::tree::{NodeId, VirtualTree};
use gpui::prelude::*;
use gpui::{App, Bounds, MouseUpEvent, ScrollHandle, Window, point, px};
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

const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

// The box spans 100..200 on both axes.
const VIEW: Rect = rect(100.0, 100.0, 100.0, 100.0);

fn dy(top: f32, height: f32, block: Align) -> f32 {
    scroll_delta(rect(100.0, top, 10.0, height), VIEW, block, Align::Nearest).1
}

#[test]
fn start_end_and_center_align_the_matching_edges() {
    assert_eq!(dy(130.0, 20.0, Align::Start), 30.0);
    assert_eq!(dy(130.0, 20.0, Align::End), -50.0);
    assert_eq!(dy(130.0, 20.0, Align::Center), -10.0);
}

#[test]
fn nearest_keeps_a_node_that_shows() {
    assert_eq!(dy(130.0, 20.0, Align::Nearest), 0.0);
    assert_eq!(dy(100.0, 100.0, Align::Nearest), 0.0);
}

#[test]
fn nearest_brings_the_near_edge_in_for_a_node_outside_the_box() {
    assert_eq!(dy(40.0, 20.0, Align::Nearest), -60.0);
    assert_eq!(dy(260.0, 20.0, Align::Nearest), 80.0);
}

#[test]
fn nearest_moves_a_partly_shown_node_the_least() {
    assert_eq!(dy(90.0, 20.0, Align::Nearest), -10.0);
    assert_eq!(dy(190.0, 20.0, Align::Nearest), 10.0);
}

#[test]
fn nearest_aligns_the_start_of_a_longer_node_that_ends_after_the_box() {
    assert_eq!(dy(150.0, 300.0, Align::Nearest), 50.0);
    assert_eq!(dy(260.0, 300.0, Align::Nearest), 160.0);
}

#[test]
fn nearest_aligns_the_end_of_a_longer_node_that_starts_before_the_box() {
    assert_eq!(dy(60.0, 130.0, Align::Nearest), -10.0);
    assert_eq!(dy(20.0, 120.0, Align::Nearest), -60.0);
    assert_eq!(dy(-50.0, 140.0, Align::Nearest), -110.0);
}

#[test]
fn nearest_keeps_a_node_that_covers_the_box() {
    assert_eq!(dy(50.0, 300.0, Align::Nearest), 0.0);
}

#[test]
fn reveal_keeps_a_node_that_shows() {
    assert_eq!(dy(130.0, 20.0, Align::Reveal), 0.0);
}

#[test]
fn reveal_centres_a_node_outside_the_box() {
    assert_eq!(dy(40.0, 20.0, Align::Reveal), -100.0);
    assert_eq!(dy(260.0, 20.0, Align::Reveal), 120.0);
    assert_eq!(dy(200.0, 20.0, Align::Reveal), 60.0);
    assert_eq!(dy(80.0, 20.0, Align::Reveal), -60.0);
}

#[test]
fn reveal_moves_a_partly_shown_node_the_least() {
    assert_eq!(dy(90.0, 20.0, Align::Reveal), -10.0);
    assert_eq!(dy(190.0, 20.0, Align::Reveal), 10.0);
    assert_eq!(dy(150.0, 300.0, Align::Reveal), 50.0);
}

#[test]
fn center_outside_keeps_a_node_that_shows_fully_or_partly() {
    let center_outside = |top, height| dy(top, height, Align::CenterOutside);
    assert_eq!(center_outside(130.0, 20.0), 0.0);
    assert_eq!(center_outside(90.0, 20.0), 0.0);
    assert_eq!(center_outside(190.0, 20.0), 0.0);
    assert_eq!(center_outside(150.0, 300.0), 0.0);
}

#[test]
fn center_outside_centres_a_node_outside_the_box() {
    assert_eq!(dy(40.0, 20.0, Align::CenterOutside), -100.0);
    assert_eq!(dy(260.0, 20.0, Align::CenterOutside), 120.0);
    assert_eq!(dy(200.0, 20.0, Align::CenterOutside), 60.0);
}

#[test]
fn a_zero_size_node_inside_or_on_the_edge_of_the_box_shows() {
    for top in [100.0, 150.0, 200.0] {
        assert_eq!(dy(top, 0.0, Align::Reveal), 0.0);
        assert_eq!(dy(top, 0.0, Align::CenterOutside), 0.0);
    }
}

#[test]
fn a_zero_size_node_outside_the_box_centres() {
    assert_eq!(dy(60.0, 0.0, Align::Reveal), -90.0);
    assert_eq!(dy(260.0, 0.0, Align::CenterOutside), 110.0);
}

#[test]
fn a_box_of_zero_height_centres_a_node_outside_it() {
    let view = rect(100.0, 100.0, 100.0, 0.0);
    let node = rect(100.0, 140.0, 10.0, 20.0);
    assert_eq!(
        scroll_delta(node, view, Align::Reveal, Align::Reveal).1,
        50.0
    );
}

#[test]
fn a_border_wider_than_half_the_box_leaves_a_zero_size_view() {
    let bounds = Bounds::new(point(px(10.0), px(10.0)), gpui::size(px(10.0), px(6.0)));
    let view = rect_of(bounds, px(8.0));
    assert_eq!((view.width, view.height), (0.0, 0.0));
    assert_eq!((view.x, view.y), (18.0, 18.0));
}

#[test]
fn the_axes_take_their_own_alignment() {
    let node = rect(260.0, 40.0, 20.0, 20.0);
    let delta = scroll_delta(node, VIEW, Align::Start, Align::End);
    assert_eq!(delta, (80.0, -60.0));
}

#[test]
fn a_container_sees_the_node_shifted_by_the_scroll_of_the_one_before() {
    let node = rect(0.0, 500.0, 10.0, 20.0);
    let chain = [
        (1, rect(0.0, 0.0, 50.0, 100.0)),
        (2, rect(0.0, 0.0, 50.0, 300.0)),
    ];
    let mut seen = Vec::new();
    scroll_chain(node, &chain, Align::Start, Align::Nearest, |id, dx, dy| {
        seen.push((id, dx, dy));
        // The inner container reaches only half of the way.
        (dx, if id == 1 { dy / 2.0 } else { dy })
    });
    assert_eq!(seen, [(1, 0.0, 500.0), (2, 0.0, 250.0)]);
}
