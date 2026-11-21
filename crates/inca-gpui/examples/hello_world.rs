// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Recreates the look of gpui's own `hello_world` example (see
//! `examples/gpui/hello_world.rs`): a bordered box, a text label, and a row
//! of six colored squares. It is built through inca-gpui's `element/build.rs`
//! conversion from a `VirtualTree` made with the Rust API, which exercises the
//! render conversion without `QuickJS`.

#![allow(clippy::unwrap_used)]

use gpui::{App, Bounds, Context, Window, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;

use inca_gpui::render_tree;
use inca_node::{NodeId, VirtualTree};
use inca_style::StyleCache;
use inca_style::set_style;

struct HelloWorld {
    tree: VirtualTree,
    root: NodeId,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        render_tree(&self.tree, &StyleCache::new(), self.root).unwrap()
    }
}

fn build_tree() -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();

    let outer = tree.create_node("div").unwrap();
    set_style(&mut tree, outer, "display", "flex").unwrap();
    set_style(&mut tree, outer, "flex_direction", "column").unwrap();
    set_style(&mut tree, outer, "gap", 12.0).unwrap();
    set_style(&mut tree, outer, "width", 500.0).unwrap();
    set_style(&mut tree, outer, "height", 500.0).unwrap();
    set_style(&mut tree, outer, "justify_content", "center").unwrap();
    set_style(&mut tree, outer, "align_items", "center").unwrap();
    set_style(&mut tree, outer, "background", f64::from(0x505050)).unwrap();
    set_style(&mut tree, outer, "border_width", 1.0).unwrap();
    set_style(&mut tree, outer, "border_color", f64::from(0x0000ff)).unwrap();
    set_style(&mut tree, outer, "text_color", f64::from(0xffffff)).unwrap();
    set_style(&mut tree, outer, "text_size", 20.0).unwrap();

    let label = tree.create_node("#text").unwrap();
    tree.set_data(label, "Hello, inca!").unwrap();
    tree.append_child(outer, label).unwrap();

    let inner = tree.create_node("div").unwrap();
    set_style(&mut tree, inner, "display", "flex").unwrap();
    set_style(&mut tree, inner, "flex_direction", "row").unwrap();
    set_style(&mut tree, inner, "gap", 8.0).unwrap();
    tree.append_child(outer, inner).unwrap();

    let colors = [0xff0000, 0x00ff00, 0x0000ff, 0xffff00, 0x000000, 0xffffff];
    for color in colors {
        let square = tree.create_node("div").unwrap();
        set_style(&mut tree, square, "width", 32.0).unwrap();
        set_style(&mut tree, square, "height", 32.0).unwrap();
        set_style(&mut tree, square, "background", f64::from(color)).unwrap();
        set_style(&mut tree, square, "border_width", 1.0).unwrap();
        set_style(&mut tree, square, "border_color", f64::from(0xffffff)).unwrap();
        set_style(&mut tree, square, "corner_radius", 4.0).unwrap();
        tree.append_child(inner, square).unwrap();
    }

    (tree, outer)
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                let (tree, root) = build_tree();
                cx.new(|_| HelloWorld { tree, root })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

fn main() {
    env_logger::init();
    run_example();
}
