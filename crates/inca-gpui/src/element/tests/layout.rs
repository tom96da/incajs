// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{num as n, text as s, *};

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
    let shift = content_shift(cx, &[("overflow_x", "scroll")], point(px(-50.), px(0.)));
    assert_eq!(shift, point(px(-50.), px(0.)));
}

#[gpui::test]
fn a_scroll_container_scrolls_without_a_dispatcher(cx: &mut TestAppContext) {
    let shift = content_shift(cx, &[("overflow_y", "scroll")], point(px(0.), px(-50.)));
    assert_eq!(shift, point(px(0.), px(-50.)));
}
