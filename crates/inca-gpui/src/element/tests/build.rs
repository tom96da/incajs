// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::{num as n, text as s, *};
use inca_test_macros::gpui_cases;

use gpui::{
    Context, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, TestAppContext,
    VisualTestContext, point, size,
};

#[gpui::test]
fn container_with_fixed_size_lays_out_at_that_size(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_style(root, "width", 120.0).unwrap();
    tree.set_style(root, "height", 80.0).unwrap();

    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        render_tree(&tree, root).unwrap()
    });

    let bounds = cx
        .debug_bounds("node-0")
        .expect("root container should be tagged with a debug selector");
    assert_eq!(bounds.size.width, px(120.0));
    assert_eq!(bounds.size.height, px(80.0));
}

#[gpui::test]
fn text_leaf_renders_without_panicking(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("text").unwrap();
    tree.set_attribute(root, "value", "hello").unwrap();

    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        render_tree(&tree, root).unwrap()
    });
}

/// Bounds of a container styled `root_props` (first) and its `children`
/// (at most two), in node order.
fn layout(
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

#[gpui::test]
fn padding_insets_the_child(cx: &mut TestAppContext) {
    let bounds = layout(
        cx,
        &[("width", n(200.)), ("height", n(100.)), ("padding", n(10.))],
        &[&[("width", n(50.)), ("height", n(30.))]],
    );
    assert_eq!(bounds[1].origin - bounds[0].origin, point(px(10.), px(10.)));
    assert_eq!(bounds[0].size, size(px(200.), px(100.)));
}

#[gpui::test]
fn padding_grows_a_container_sized_by_its_content(cx: &mut TestAppContext) {
    let bounds = layout(
        cx,
        &[("padding_top", n(4.)), ("padding_bottom", n(6.))],
        &[&[("height", n(30.))]],
    );
    assert_eq!(bounds[0].size.height, px(40.));
}

#[gpui::test]
fn a_negative_margin_pulls_the_child_out(cx: &mut TestAppContext) {
    let bounds = layout(
        cx,
        &[("width", n(200.)), ("height", n(100.))],
        &[&[
            ("width", n(50.)),
            ("height", n(30.)),
            ("margin_left", n(-5.)),
        ]],
    );
    assert_eq!(bounds[1].origin.x - bounds[0].origin.x, px(-5.));
}

#[gpui::test]
fn margin_offsets_the_child(cx: &mut TestAppContext) {
    let bounds = layout(
        cx,
        &[("width", n(200.)), ("height", n(100.))],
        &[&[
            ("width", n(50.)),
            ("height", n(30.)),
            ("margin", n(2.)),
            ("margin_left", n(7.)),
            ("margin_top", n(3.)),
        ]],
    );
    assert_eq!(bounds[1].origin - bounds[0].origin, point(px(7.), px(3.)));
}

#[gpui::test]
fn an_auto_margin_takes_the_free_space(cx: &mut TestAppContext) {
    let row = [
        ("display", s("flex")),
        ("flex_direction", s("row")),
        ("width", n(200.)),
        ("height", n(100.)),
    ];
    let pushed = layout(
        cx,
        &row,
        &[&[
            ("width", n(50.)),
            ("height", n(30.)),
            ("margin_left", s("auto")),
        ]],
    );
    assert_eq!(pushed[1].origin.x - pushed[0].origin.x, px(150.));
    let centered = layout(
        cx,
        &row,
        &[&[
            ("width", n(50.)),
            ("height", n(30.)),
            ("margin_x", s("auto")),
        ]],
    );
    assert_eq!(centered[1].origin.x - centered[0].origin.x, px(75.));
}

#[gpui::test]
fn min_width_widens_and_min_height_heightens(cx: &mut TestAppContext) {
    let bounds = layout(
        cx,
        &[
            ("width", n(50.)),
            ("height", n(20.)),
            ("min_width", n(80.)),
            ("min_height", n(35.)),
        ],
        &[],
    );
    assert_eq!(bounds[0].size, size(px(80.), px(35.)));
}

#[gpui::test]
fn max_width_narrows_and_max_height_shortens(cx: &mut TestAppContext) {
    let bounds = layout(
        cx,
        &[
            ("width", n(200.)),
            ("height", n(100.)),
            ("max_width", n(120.)),
            ("max_height", n(60.)),
        ],
        &[],
    );
    assert_eq!(bounds[0].size, size(px(120.), px(60.)));
}

fn row() -> [(&'static str, AttributeValue); 4] {
    [
        ("display", s("flex")),
        ("flex_direction", s("row")),
        ("width", n(200.)),
        ("height", n(50.)),
    ]
}

#[gpui::test]
fn flex_grow_fills_the_remaining_space(cx: &mut TestAppContext) {
    let bounds = layout(
        cx,
        &row(),
        &[
            &[("width", n(50.)), ("flex_grow", n(1.))],
            &[("width", n(50.))],
        ],
    );
    assert_eq!(bounds[1].size.width, px(150.));
    assert_eq!(bounds[2].size.width, px(50.));
    assert_eq!(bounds[2].origin.x - bounds[0].origin.x, px(150.));
}

struct TreeView(VirtualTree);

impl Render for TreeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        render_tree(&self.0, 0).unwrap()
    }
}

/// How far a wheel `delta` over a 100x100 container with `props`
/// moves its 300x300 child, without a dispatcher.
fn content_shift(
    cx: &mut TestAppContext,
    props: &[(&str, &str)],
    delta: Point<Pixels>,
) -> Point<Pixels> {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    let child = tree.create_node("div").unwrap();
    tree.set_style(root, "width", 100.0).unwrap();
    tree.set_style(root, "height", 100.0).unwrap();
    for (key, value) in props {
        tree.set_style(root, *key, *value).unwrap();
    }
    tree.set_style(child, "width", 300.0).unwrap();
    tree.set_style(child, "height", 300.0).unwrap();
    tree.append_child(root, child).unwrap();

    let window = cx.add_window(|_, _| TreeView(tree));
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.), px(10.)),
        delta: ScrollDelta::Pixels(delta),
        ..Default::default()
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let container = cx.debug_bounds("node-0").unwrap();
    let content = cx.debug_bounds("node-1").unwrap();
    content.origin - container.origin
}

#[gpui::test]
fn an_x_only_container_scrolls_along_x_without_a_dispatcher(cx: &mut TestAppContext) {
    let shift = content_shift(
        cx,
        &[("overflow_x", "scroll"), ("overflow_y", "hidden")],
        point(px(-50.), px(0.)),
    );
    assert_eq!(shift, point(px(-50.), px(0.)));
}

#[gpui::test]
fn a_scroll_container_scrolls_without_a_dispatcher(cx: &mut TestAppContext) {
    let shift = content_shift(cx, &[("overflow_y", "scroll")], point(px(0.), px(-50.)));
    assert_eq!(shift, point(px(0.), px(-50.)));
}

fn mounted(cx: &mut TestAppContext, tree: VirtualTree) -> (Probe, gpui::AnyWindowHandle) {
    let probe = probe_of(tree);
    let view = probe.clone();
    let window = cx.add_window(|_, _| ProbeView(view));
    (probe, window.into())
}

fn frame(cx: &mut TestAppContext, window: gpui::AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

fn draw(cx: &mut gpui::VisualTestContext, tree: &VirtualTree, probe: &Probe) {
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        render_tree_with_events(tree, 0, probe).unwrap()
    });
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> gpui::Bounds<Pixels> {
    gpui::Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

#[gpui::test]
fn nested_boxes_record_window_bounds_and_border(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 200., 200.);
    let mid = boxed(&mut tree, Some(root), 100., 100.);
    tree.set_style(mid, "border_width", 2.0).unwrap();
    tree.set_style(root, "padding", 10.0).unwrap();
    let leaf = boxed(&mut tree, Some(mid), 50., 40.);
    let probe = probe();
    draw(cx.add_empty_window(), &tree, &probe);
    let layouts = probe.layouts.borrow();
    let mid = layouts[&mid];
    assert_eq!(mid.bounds, rect(10., 10., 100., 100.));
    assert_eq!(mid.border, px(2.));
    let leaf = layouts[&leaf];
    assert_eq!(leaf.bounds, rect(12., 12., 50., 40.));
    assert_eq!(leaf.border, px(0.));
    assert!(!layouts.contains_key(&root), "the root is the host's");
}

#[gpui::test]
fn the_parents_scroll_offset_moves_the_children(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "scroll").unwrap();
    let first = boxed(&mut tree, Some(root), 100., 300.);
    let second = boxed(&mut tree, Some(root), 100., 20.);
    let (probe, window) = mounted(cx, tree);
    frame(cx, window);
    assert_eq!(
        probe.layouts.borrow()[&first].bounds,
        rect(0., 0., 100., 300.)
    );
    probe.handles.borrow()[&root].set_offset(point(px(0.), px(-50.)));
    frame(cx, window);
    let layouts = probe.layouts.borrow();
    assert_eq!(layouts[&first].bounds, rect(0., -50., 100., 300.));
    assert_eq!(layouts[&second].bounds, rect(0., 250., 100., 20.));
}

#[gpui::test]
fn a_hidden_container_ignores_the_wheel(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_x", "hidden").unwrap();
    tree.set_style(root, "overflow_y", "hidden").unwrap();
    let _child = boxed(&mut tree, Some(root), 100., 300.);
    let (probe, window) = mounted(cx, tree);
    frame(cx, window);
    let mut vcx = VisualTestContext::from_window(window, cx);
    vcx.simulate_event(ScrollWheelEvent {
        position: point(px(10.), px(10.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-50.))),
        ..Default::default()
    });
    vcx.update(|window, cx| window.draw(cx).clear(cx));
    let handle = probe.handles.borrow()[&root].clone();
    assert_eq!(handle.offset(), point(px(0.), px(0.)));
}

#[gpui::test]
fn a_hidden_container_moves_its_content_by_a_script_offset(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_x", "hidden").unwrap();
    tree.set_style(root, "overflow_y", "hidden").unwrap();
    let child = boxed(&mut tree, Some(root), 100., 300.);
    let (probe, window) = mounted(cx, tree);
    frame(cx, window);
    let mut vcx = VisualTestContext::from_window(window, cx);
    let handle = probe.handles.borrow()[&root].clone();
    handle.set_offset(point(px(0.), px(-50.)));
    vcx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(handle.max_offset().y, px(200.));
    assert_eq!(
        probe.layouts.borrow()[&child].bounds,
        rect(0., -50., 100., 300.)
    );
}

#[gpui::test]
fn a_bordered_scroll_range_ends_at_the_padding(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 400., 400.);
    let scroller = boxed(&mut tree, Some(root), 120., 100.);
    tree.set_style(scroller, "padding", 10.0).unwrap();
    tree.set_style(scroller, "border_width", 3.0).unwrap();
    tree.set_style(scroller, "overflow", "scroll").unwrap();
    for _ in 0..2 {
        let child = boxed(&mut tree, Some(scroller), 300., 120.);
        tree.set_style(child, "flex_shrink", 0.0).unwrap();
    }
    let (probe, window) = mounted(cx, tree);
    frame(cx, window);
    let handle = probe.handles.borrow()[&scroller].clone();
    assert_eq!(handle.max_offset(), point(px(206.), px(166.)));
}

#[gpui::test]
fn the_scroll_range_holds_the_start_margins_of_the_first_child(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 400., 400.);
    let scroller = boxed(&mut tree, Some(root), 120., 100.);
    tree.set_style(scroller, "padding", 10.0).unwrap();
    tree.set_style(scroller, "overflow", "scroll").unwrap();
    let child = boxed(&mut tree, Some(scroller), 300., 240.);
    tree.set_style(child, "margin_top", 7.0).unwrap();
    tree.set_style(child, "margin_left", 7.0).unwrap();
    let (probe, window) = mounted(cx, tree);
    frame(cx, window);
    let handle = probe.handles.borrow()[&scroller].clone();
    assert_eq!(handle.max_offset(), point(px(207.), px(167.)));
}

#[gpui_cases]
#[gpui::test]
#[case::below_half(-50.4, -50.5)]
#[case::quarter(-50.25, -50.)]
#[case::three_quarters(-50.75, -50.5)]
fn a_fractional_scroll_offset_is_snapped_to_the_device_pixel(
    cx: &mut TestAppContext,
    offset: f32,
    drawn: f32,
) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "scroll").unwrap();
    let child = boxed(&mut tree, Some(root), 100., 300.);
    let (probe, window) = mounted(cx, tree);
    // The test window has a scale factor of 2, so the grid is 0.5px.
    frame(cx, window);
    probe.handles.borrow()[&root].set_offset(point(px(0.), px(offset)));
    frame(cx, window);
    assert_eq!(
        probe.layouts.borrow()[&child].bounds,
        rect(0., drawn, 100., 300.)
    );
}

/// Draws an outer and an inner scroll container at `scale`, scrolled
/// by `outer` and `inner`, and returns the leaf's recorded y.
fn nested_leaf_y(cx: &mut TestAppContext, scale: f32, outer: f32, inner: f32) -> Pixels {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "scroll").unwrap();
    let mid = boxed(&mut tree, Some(root), 100., 300.);
    tree.set_style(mid, "overflow_y", "scroll").unwrap();
    let leaf = boxed(&mut tree, Some(mid), 100., 600.);
    let (probe, window) = mounted(cx, tree);
    cx.simulate_window_scale_factor_change(window, scale);
    frame(cx, window);
    probe.handles.borrow()[&root].set_offset(point(px(0.), px(outer)));
    probe.handles.borrow()[&mid].set_offset(point(px(0.), px(inner)));
    frame(cx, window);
    probe.layouts.borrow()[&leaf].bounds.origin.y
}

#[gpui::test]
fn nested_fractional_offsets_are_snapped_as_one_sum(cx: &mut TestAppContext) {
    // Drawn at the snap of the summed offset.
    assert_eq!(nested_leaf_y(cx, 2., -10.3, -10.3), px(-20.5));
    assert_eq!(nested_leaf_y(cx, 1., -10.3, -10.3), px(-21.));
}

#[gpui::test]
fn a_record_carries_the_snapped_scroll_its_parent_applied(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "scroll").unwrap();
    tree.set_style(root, "padding", 4.0).unwrap();
    let mid = boxed(&mut tree, Some(root), 100., 300.);
    tree.set_style(mid, "overflow_y", "scroll").unwrap();
    tree.set_style(mid, "border_width", 2.0).unwrap();
    let leaf = boxed(&mut tree, Some(mid), 100., 600.);
    let (probe, window) = mounted(cx, tree);
    cx.simulate_window_scale_factor_change(window, 2.);
    frame(cx, window);
    probe.handles.borrow()[&root].set_offset(point(px(0.), px(-10.3)));
    probe.handles.borrow()[&mid].set_offset(point(px(0.), px(-10.3)));
    frame(cx, window);
    let layouts = probe.layouts.borrow();
    // The outer scroll snaps to 10.5; the sum 20.6 snaps to 20.5.
    assert_eq!(layouts[&mid].scroll, point(px(0.), px(10.5)));
    assert_eq!(layouts[&leaf].scroll, point(px(0.), px(10.)));
}

#[gpui_cases]
#[gpui::test]
#[case::rounds_down(-50.4, -50.)]
#[case::rounds_up(-50.6, -51.)]
fn a_fractional_scroll_offset_snaps_to_whole_pixels_at_scale_one(
    cx: &mut TestAppContext,
    offset: f32,
    drawn: f32,
) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "scroll").unwrap();
    let child = boxed(&mut tree, Some(root), 100., 300.);
    let (probe, window) = mounted(cx, tree);
    cx.simulate_window_scale_factor_change(window, 1.);
    frame(cx, window);
    probe.handles.borrow()[&root].set_offset(point(px(0.), px(offset)));
    frame(cx, window);
    assert_eq!(
        probe.layouts.borrow()[&child].bounds,
        rect(0., drawn, 100., 300.)
    );
}

#[gpui::test]
fn a_display_none_subtree_has_no_layout_record(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 200., 200.);
    let hidden = boxed(&mut tree, Some(root), 100., 100.);
    tree.set_style(hidden, "display", "none").unwrap();
    let _shown = boxed(&mut tree, Some(root), 30., 30.);
    let inside = boxed(&mut tree, Some(hidden), 10., 10.);
    let probe = probe();
    draw(cx.add_empty_window(), &tree, &probe);
    let layouts = probe.layouts.borrow();
    assert!(!layouts.contains_key(&hidden) && !layouts.contains_key(&inside));
}

#[gpui::test]
fn a_display_none_child_leaves_its_sibling_at_the_start(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 200., 200.);
    let hidden = boxed(&mut tree, Some(root), 100., 100.);
    tree.set_style(hidden, "display", "none").unwrap();
    let shown = boxed(&mut tree, Some(root), 30., 30.);
    boxed(&mut tree, Some(hidden), 10., 10.);
    let probe = probe();
    draw(cx.add_empty_window(), &tree, &probe);
    let layouts = probe.layouts.borrow();
    assert_eq!(layouts[&shown].bounds, rect(0., 0., 30., 30.));
}

#[gpui::test]
fn a_text_child_is_recorded(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 200., 200.);
    let label = tree.create_node("text").unwrap();
    tree.set_attribute(label, "value", "hello").unwrap();
    tree.append_child(root, label).unwrap();
    let probe = probe();
    draw(cx.add_empty_window(), &tree, &probe);
    let layout = probe.layouts.borrow()[&label];
    assert_eq!(layout.bounds.origin, point(px(0.), px(0.)));
    assert!(layout.bounds.size.width > px(0.) && layout.bounds.size.height > px(0.));
    assert_eq!(layout.border, px(0.));
}

#[gpui::test]
fn the_record_clears_between_frames(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 200., 200.);
    let child = boxed(&mut tree, Some(root), 50., 50.);
    let probe = probe();
    let cx = cx.add_empty_window();
    draw(cx, &tree, &probe);
    assert!(probe.layouts.borrow().contains_key(&child));
    tree.remove_child(root, child).unwrap();
    draw(cx, &tree, &probe);
    assert!(probe.layouts.borrow().is_empty());
}

#[gpui::test]
fn tracking_the_pointer_leaves_layout_alone(cx: &mut TestAppContext) {
    let bounds = |cx: &mut VisualTestContext| -> Vec<_> {
        ["node-0", "node-1", "node-2"]
            .map(|selector| cx.debug_bounds(selector).unwrap())
            .to_vec()
    };
    let mut tracked = probe_tree(cx, true);
    let tracked = bounds(&mut tracked.cx);
    let mut plain = probe_tree(cx, false);
    assert_eq!(tracked, bounds(&mut plain.cx));
}
