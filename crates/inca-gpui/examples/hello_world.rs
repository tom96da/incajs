// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Recreates the look of gpui's own `hello_world` example (see
//! `examples/gpui/hello_world.rs`) — a bordered box, a text label, and a row
//! of six colored squares — but built through inca-gpui's `element.rs`
//! conversion instead of calling gpui's builder API directly. Builds the
//! tree via the Rust `VirtualTree` API, not through `QuickJS`, to isolate the
//! render conversion from the JS bridge.

#![allow(clippy::unwrap_used)]

use gpui::{App, Bounds, Context, Window, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;

use inca_gpui::{NodeId, VirtualTree, render_tree};

struct HelloWorld {
    tree: VirtualTree,
    root: NodeId,
}

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        render_tree(&self.tree, self.root).unwrap()
    }
}

fn build_tree() -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();

    let outer = tree.create_node("div");
    tree.set_style(outer, "display", "flex").unwrap();
    tree.set_style(outer, "flex_direction", "column").unwrap();
    tree.set_style(outer, "gap", 12.0).unwrap();
    tree.set_style(outer, "width", 500.0).unwrap();
    tree.set_style(outer, "height", 500.0).unwrap();
    tree.set_style(outer, "justify_content", "center").unwrap();
    tree.set_style(outer, "align_items", "center").unwrap();
    tree.set_style(outer, "background", f64::from(0x505050))
        .unwrap();
    tree.set_style(outer, "border_width", 1.0).unwrap();
    tree.set_style(outer, "border_color", f64::from(0x0000ff))
        .unwrap();
    tree.set_style(outer, "text_color", f64::from(0xffffff))
        .unwrap();
    tree.set_style(outer, "text_size", 20.0).unwrap();

    let label = tree.create_node("text");
    tree.set_attribute(label, "value", "Hello, inca!").unwrap();
    tree.append_child(outer, label).unwrap();

    let inner = tree.create_node("div");
    tree.set_style(inner, "display", "flex").unwrap();
    tree.set_style(inner, "flex_direction", "row").unwrap();
    tree.set_style(inner, "gap", 8.0).unwrap();
    tree.append_child(outer, inner).unwrap();

    let colors = [0xff0000, 0x00ff00, 0x0000ff, 0xffff00, 0x000000, 0xffffff];
    for color in colors {
        let square = tree.create_node("div");
        tree.set_style(square, "width", 32.0).unwrap();
        tree.set_style(square, "height", 32.0).unwrap();
        tree.set_style(square, "background", f64::from(color))
            .unwrap();
        tree.set_style(square, "border_width", 1.0).unwrap();
        tree.set_style(square, "border_color", f64::from(0xffffff))
            .unwrap();
        tree.set_style(square, "corner_radius", 4.0).unwrap();
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
