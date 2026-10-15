// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

use gpui::{Pixels, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, size};

fn mounted(cx: &mut TestAppContext, tree: VirtualTree) -> (Probe, gpui::AnyWindowHandle) {
    let probe = probe();
    let view = probe.clone();
    let window = cx.add_window(|_, _| ProbeView(tree, view));
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
fn a_hidden_container_ignores_the_wheel_and_takes_a_script_offset(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "hidden").unwrap();
    let child = boxed(&mut tree, Some(root), 100., 300.);
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
    handle.set_offset(point(px(0.), px(-50.)));
    vcx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(handle.max_offset().y, px(200.));
    assert_eq!(
        probe.layouts.borrow()[&child].bounds,
        rect(0., -50., 100., 300.)
    );
}

#[gpui::test]
fn a_fractional_scroll_offset_is_snapped_to_the_device_pixel(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "scroll").unwrap();
    let child = boxed(&mut tree, Some(root), 100., 300.);
    let (probe, window) = mounted(cx, tree);
    // The test window has a scale factor of 2, so the grid is 0.5px.
    for (offset, drawn) in [(-50.4, -50.5), (-50.25, -50.), (-50.75, -50.5)] {
        frame(cx, window);
        probe.handles.borrow()[&root].set_offset(point(px(0.), px(offset)));
        frame(cx, window);
        assert_eq!(
            probe.layouts.borrow()[&child].bounds,
            rect(0., drawn, 100., 300.)
        );
    }
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

#[gpui::test]
fn a_fractional_scroll_offset_snaps_to_whole_pixels_at_scale_one(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 100., 100.);
    tree.set_style(root, "overflow_y", "scroll").unwrap();
    let child = boxed(&mut tree, Some(root), 100., 300.);
    let (probe, window) = mounted(cx, tree);
    cx.simulate_window_scale_factor_change(window, 1.);
    for (offset, drawn) in [(-50.4, -50.), (-50.6, -51.)] {
        frame(cx, window);
        probe.handles.borrow()[&root].set_offset(point(px(0.), px(offset)));
        frame(cx, window);
        assert_eq!(
            probe.layouts.borrow()[&child].bounds,
            rect(0., drawn, 100., 300.)
        );
    }
}

#[gpui::test]
fn a_display_none_child_has_no_record_and_keeps_its_siblings_aligned(cx: &mut TestAppContext) {
    let mut tree = VirtualTree::new();
    let root = boxed(&mut tree, None, 200., 200.);
    let hidden = boxed(&mut tree, Some(root), 100., 100.);
    tree.set_style(hidden, "display", "none").unwrap();
    let shown = boxed(&mut tree, Some(root), 30., 30.);
    let inside = boxed(&mut tree, Some(hidden), 10., 10.);
    let probe = probe();
    draw(cx.add_empty_window(), &tree, &probe);
    let layouts = probe.layouts.borrow();
    assert_eq!(layouts[&shown].bounds, rect(0., 0., 30., 30.));
    assert!(!layouts.contains_key(&hidden) && !layouts.contains_key(&inside));
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
