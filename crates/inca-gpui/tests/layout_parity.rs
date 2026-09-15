// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Layout-parity test for `inca_gpui::element`'s `VirtualTree` →
//! `gpui` conversion: builds the same box structure two ways — directly
//! with `gpui`'s own builder API, and through a
//! `VirtualTree` via `render_tree` — and asserts the computed layout (sizes
//! and positions, not pixel content) matches. Modeled on
//! `examples/gpui/hello_world.rs`'s shape (a bordered, centered column
//! containing a text line and a row of six bordered squares), not a literal
//! copy of it (that's a binary example, not importable, and its
//! Tailwind-scale style values aren't guaranteed to equal particular pixel
//! amounts) — this uses its own literal pixel values instead, applied to
//! both sides identically.

#![allow(clippy::unwrap_used)]

use gpui::prelude::*;
use gpui::{
    AlignItems, AnyElement, Bounds, Display, FlexDirection, JustifyContent, Pixels, TestAppContext,
    div, point, px, size,
};

use inca_gpui::{NodeId, VirtualTree, render_tree};

/// Selectors matching [`build_tree`]'s node creation order, used to tag the
/// hand-written reference tree the same way `render_tree` tags its own
/// containers (see `build_element`'s doc comment).
const OUTER: &str = "node-0";
const INNER: &str = "node-2";
const SQUARES: [&str; 6] = ["node-3", "node-4", "node-5", "node-6", "node-7", "node-8"];

fn selectors() -> Vec<&'static str> {
    let mut all = vec![OUTER, INNER];
    all.extend(SQUARES);
    all
}

/// A bordered, centered column containing a text line and a row of six
/// bordered squares — built purely through the `VirtualTree`/`set_style`
/// API, no JS involved.
fn build_tree() -> (VirtualTree, NodeId) {
    let mut tree = VirtualTree::new();

    let outer = tree.create_node("div"); // node-0
    tree.set_style(outer, "display", "flex").unwrap();
    tree.set_style(outer, "flex_direction", "column").unwrap();
    tree.set_style(outer, "gap", 12.0).unwrap();
    tree.set_style(outer, "width", 500.0).unwrap();
    tree.set_style(outer, "height", 500.0).unwrap();
    tree.set_style(outer, "justify_content", "center").unwrap();
    tree.set_style(outer, "align_items", "center").unwrap();
    tree.set_style(outer, "border_width", 2.0).unwrap();

    let label = tree.create_node("text"); // node-1
    tree.set_attribute(label, "value", "Hello, World!").unwrap();
    tree.append_child(outer, label).unwrap();

    let inner = tree.create_node("div"); // node-2
    tree.set_style(inner, "display", "flex").unwrap();
    tree.set_style(inner, "flex_direction", "row").unwrap();
    tree.set_style(inner, "gap", 8.0).unwrap();
    tree.append_child(outer, inner).unwrap();

    for _ in 0..6 {
        let square = tree.create_node("div"); // node-3..node-8
        tree.set_style(square, "width", 32.0).unwrap();
        tree.set_style(square, "height", 32.0).unwrap();
        tree.set_style(square, "border_width", 2.0).unwrap();
        tree.append_child(inner, square).unwrap();
    }

    (tree, outer)
}

fn set_uniform_border(style: &mut gpui::StyleRefinement, width: f32) {
    let width = px(width);
    style.border_widths.top = Some(width.into());
    style.border_widths.right = Some(width.into());
    style.border_widths.bottom = Some(width.into());
    style.border_widths.left = Some(width.into());
}

/// The same shape as [`build_tree`], expressed directly against `gpui`'s
/// `StyleRefinement` — independent of the conversion under test.
fn build_reference() -> AnyElement {
    let mut outer = div().debug_selector(|| OUTER.into());
    {
        let style = outer.style();
        style.display = Some(Display::Flex);
        style.flex_direction = Some(FlexDirection::Column);
        style.gap.width = Some(px(12.0).into());
        style.gap.height = Some(px(12.0).into());
        style.size.width = Some(px(500.0).into());
        style.size.height = Some(px(500.0).into());
        style.justify_content = Some(JustifyContent::Center);
        style.align_items = Some(AlignItems::Center);
        set_uniform_border(style, 2.0);
    }

    let mut inner = div().debug_selector(|| INNER.into());
    {
        let style = inner.style();
        style.display = Some(Display::Flex);
        style.flex_direction = Some(FlexDirection::Row);
        style.gap.width = Some(px(8.0).into());
        style.gap.height = Some(px(8.0).into());
    }

    for selector in SQUARES {
        let mut square = div().debug_selector(move || selector.into());
        {
            let style = square.style();
            style.size.width = Some(px(32.0).into());
            style.size.height = Some(px(32.0).into());
            set_uniform_border(style, 2.0);
        }
        inner = inner.child(square);
    }

    outer.child("Hello, World!").child(inner).into_any_element()
}

fn draw_and_collect_bounds(
    cx: &mut TestAppContext,
    build: impl FnOnce() -> AnyElement,
) -> Vec<(&'static str, Bounds<Pixels>)> {
    let cx = cx.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        build()
    });

    selectors()
        .into_iter()
        .map(|selector| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("missing debug bounds for {selector}"));
            (selector, bounds)
        })
        .collect()
}

#[gpui::test]
fn virtual_tree_layout_matches_hand_written_gpui(cx: &mut TestAppContext) {
    let reference_bounds = draw_and_collect_bounds(cx, build_reference);

    let (tree, root) = build_tree();
    let converted_bounds = draw_and_collect_bounds(cx, || render_tree(&tree, root).unwrap());

    assert_eq!(
        converted_bounds, reference_bounds,
        "layout built through VirtualTree/render_tree must match the same shape built directly with gpui"
    );
}
