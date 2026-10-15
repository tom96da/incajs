// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

use gpui::{ScrollDelta, ScrollWheelEvent};

struct Wheels {
    probe: Probe,
    cx: VisualTestContext,
}

impl Wheels {
    /// Mounts and draws `tree`, rooted at node 0.
    fn new(cx: &mut TestAppContext, tree: VirtualTree) -> Self {
        let probe = probe();
        let view = probe.clone();
        let window = cx.add_window(|_, _| ProbeView(tree, view));
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        Self {
            probe,
            cx: VisualTestContext::from_window(window.into(), cx),
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
        self.cx.simulate_event(ScrollWheelEvent {
            position: point(px(10.), px(pointer_y)),
            delta,
            ..Default::default()
        });
        self.cx.run_until_parked();
        self.cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    fn offset(&self, node: NodeId) -> Point<Pixels> {
        self.probe.scroll_handle(node).unwrap().offset()
    }
}

/// Adds a 100px wide div under `parent`, or sizes the root when there is
/// none.
fn div(tree: &mut VirtualTree, parent: Option<NodeId>, height: f64, scrolls: bool) -> NodeId {
    let node = boxed(tree, parent, 100., height);
    if scrolls {
        tree.set_style(node, "overflow_y", "scroll").unwrap();
    }
    node
}

/// Node 0 scrolls (100px over 300px of content, node 1).
fn one_container() -> VirtualTree {
    let mut tree = VirtualTree::new();
    let container = div(&mut tree, None, 100., true);
    div(&mut tree, Some(container), 300., false);
    tree
}

/// Node 0 scrolls 200px over node 1, which scrolls 200px over node 2, and
/// node 3 fills the rest of node 0.
fn nested_containers() -> VirtualTree {
    let mut tree = VirtualTree::new();
    let outer = div(&mut tree, None, 100., true);
    let inner = div(&mut tree, Some(outer), 100., true);
    div(&mut tree, Some(inner), 300., false);
    div(&mut tree, Some(outer), 200., false);
    tree
}

/// A container of 100x100 with `styles` over a 300x300 child.
fn styled_container(styles: &[(&str, &str)]) -> VirtualTree {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    for (key, value) in styles {
        tree.set_style(root, *key, *value).unwrap();
    }
    boxed(&mut tree, Some(root), 300., 300.);
    tree
}

#[gpui::test]
fn only_the_innermost_container_scrolls(cx: &mut TestAppContext) {
    let mut scene = Wheels::new(cx, nested_containers());
    scene.wheel(-50.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn an_inner_container_at_its_limit_chains_to_the_outer(cx: &mut TestAppContext) {
    let mut scene = Wheels::new(cx, nested_containers());
    scene.wheel(-200.);
    assert_eq!(scene.offset(1).y, px(-200.));
    assert_eq!(scene.offset(0).y, px(0.));

    scene.wheel(-50.);
    assert_eq!(scene.offset(1).y, px(-200.));
    assert_eq!(scene.offset(0).y, px(-50.));
}

#[gpui::test]
fn a_wheel_over_no_container_leaves_every_offset_alone(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = div(&mut tree, None, 250., false);
    for _ in 0..2 {
        let sibling = div(&mut tree, Some(root), 100., true);
        div(&mut tree, Some(sibling), 300., false);
    }
    let mut scene = Wheels::new(cx, tree);
    scene.wheel(-50.);
    scene.wheel_at(220., -50.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(3).y, px(0.));
}

#[gpui::test]
fn only_the_container_under_the_pointer_scrolls(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = div(&mut tree, None, 200., false);
    for _ in 0..2 {
        let sibling = div(&mut tree, Some(root), 100., true);
        div(&mut tree, Some(sibling), 300., false);
    }
    let mut scene = Wheels::new(cx, tree);
    scene.wheel(-50.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(3).y, px(0.));

    scene.wheel_at(110., -30.);
    assert_eq!(scene.offset(1).y, px(-50.));
    assert_eq!(scene.offset(3).y, px(-30.));
}

#[gpui::test]
fn repeated_wheels_add_up_and_stop_at_the_end(cx: &mut TestAppContext) {
    let mut scene = Wheels::new(cx, one_container());
    scene.wheel(-50.);
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-100.));
    scene.wheel(-300.);
    assert_eq!(scene.offset(0).y, px(-200.));
    scene.wheel(-50.);
    assert_eq!(scene.offset(0).y, px(-200.));
}

#[gpui::test]
fn scrolling_up_at_the_top_moves_nothing(cx: &mut TestAppContext) {
    let mut scene = Wheels::new(cx, one_container());
    scene.wheel(50.);
    assert_eq!(scene.offset(0).y, px(0.));
}

#[gpui::test]
fn scrolling_up_chains_outward_from_an_inner_container_at_the_top(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let outer = div(&mut tree, None, 150., true);
    let inner = div(&mut tree, Some(outer), 100., true);
    div(&mut tree, Some(inner), 300., false);
    div(&mut tree, Some(outer), 200., false);
    let mut scene = Wheels::new(cx, tree);
    scene.wheel_at(120., -50.);
    assert_eq!(scene.offset(0).y, px(-50.));

    scene.wheel(30.);
    assert_eq!(scene.offset(1).y, px(0.));
    assert_eq!(scene.offset(0).y, px(-20.));
}

#[gpui::test]
fn three_nested_containers_chain_from_the_inside_out(cx: &mut TestAppContext) {
    // Three scroll containers, each 100px over 200px of further content.
    let mut tree = VirtualTree::new();
    let outer = div(&mut tree, None, 100., true);
    let middle = div(&mut tree, Some(outer), 100., true);
    let inner = div(&mut tree, Some(middle), 100., true);
    div(&mut tree, Some(inner), 300., false);
    div(&mut tree, Some(middle), 200., false);
    div(&mut tree, Some(outer), 200., false);
    let mut scene = Wheels::new(cx, tree);
    let offsets = |scene: &Wheels| [2, 1, 0].map(|node| scene.offset(node).y);

    scene.wheel(-200.);
    assert_eq!(offsets(&scene), [px(-200.), px(0.), px(0.)]);
    scene.wheel(-50.);
    assert_eq!(offsets(&scene), [px(-200.), px(-50.), px(0.)]);
    scene.wheel(-150.);
    assert_eq!(offsets(&scene), [px(-200.), px(-200.), px(0.)]);
    scene.wheel(-50.);
    assert_eq!(offsets(&scene), [px(-200.), px(-200.), px(-50.)]);
}

#[gpui::test]
fn a_diagonal_wheel_scrolls_only_the_scrolling_axis(cx: &mut TestAppContext) {
    let tree = styled_container(&[("overflow_x", "hidden"), ("overflow_y", "scroll")]);
    let mut scene = Wheels::new(cx, tree);
    scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-10.), px(-50.))));
    assert_eq!(scene.offset(0), point(px(0.), px(-50.)));
}

// Pins gpui's axis rule: the y delta moves the container even when the x
// delta is larger.
#[gpui::test]
fn an_x_dominant_wheel_moves_a_y_only_container_by_its_y_delta(cx: &mut TestAppContext) {
    let tree = styled_container(&[("overflow_x", "hidden"), ("overflow_y", "scroll")]);
    let mut scene = Wheels::new(cx, tree);
    scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-50.), px(-10.))));
    assert_eq!(scene.offset(0), point(px(0.), px(-10.)));
}

// Pins gpui's axis rule.
#[gpui::test]
fn a_vertical_wheel_scrolls_an_x_only_container_along_x(cx: &mut TestAppContext) {
    let mut scene = Wheels::new(cx, styled_container(&[("overflow_x", "scroll")]));
    scene.wheel(-50.);
    assert_eq!(scene.offset(0), point(px(-50.), px(0.)));
}
