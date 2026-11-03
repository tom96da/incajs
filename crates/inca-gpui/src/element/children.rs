// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reports the box of each rendered child of an element to its sink.

use gpui::prelude::*;
use gpui::{Bounds, Pixels, ScrollHandle, point, px};

use super::build::NodeLayout;
use super::scroll::snapped_extra;
use super::spec::{DisplaySpec, ElementSpec, ElementTag};
use crate::event_sink::EventSink;
use crate::tree::NodeId;

/// The children of `spec` that become `gpui` elements, in order. An empty
/// text child renders nothing.
pub(super) fn rendered_children(spec: &ElementSpec) -> impl Iterator<Item = &ElementSpec> {
    spec.children
        .iter()
        .filter(|child| child.tag != ElementTag::Text(String::new()))
}

/// Reports the box of each rendered child of `spec` to `sink` once `element`
/// has laid its children out. A child with `display: none` has no box and
/// reports nothing.
pub(super) fn record_children<E>(
    element: gpui::Div,
    spec: &ElementSpec,
    handle: Option<ScrollHandle>,
    sink: E,
) -> gpui::Div
where
    E: EventSink + 'static,
{
    // The border of each rendered child; `None` for `display: none`.
    #[allow(clippy::cast_possible_truncation)]
    let children: Vec<(NodeId, Option<Pixels>)> = rendered_children(spec)
        .map(|child| {
            let shown = child.style.display != Some(DisplaySpec::None);
            let border = px(child.style.border_width.unwrap_or(0.0) as f32);
            (child.id, shown.then_some(border))
        })
        .collect();
    element.when(!children.is_empty(), |el| {
        el.on_children_prepainted(move |listed, window, _| {
            // `gpui` hands a scroll-tracked container's bounds to its handle
            // and passes the listener none.
            let raw: Vec<Bounds<Pixels>> = match &handle {
                Some(handle) => (0..).map_while(|ix| handle.bounds_for_item(ix)).collect(),
                None => listed,
            };
            let offset = handle
                .as_ref()
                .map(ScrollHandle::offset)
                .unwrap_or_default();
            // `gpui` snaps the whole accumulated offset, and the raw bounds
            // already hold the snapped offset of the ancestors.
            let outer = window.element_offset();
            let offset = snapped_extra(window, outer + offset, outer);
            // The tracker and the scroll anchor come before the children.
            let first = raw.len().saturating_sub(children.len());
            for (&(id, border), bounds) in children.iter().zip(&raw[first..]) {
                if let Some(border) = border {
                    let bounds = Bounds::new(bounds.origin + offset, bounds.size);
                    let scroll = point(-offset.x, -offset.y);
                    sink.record_layout(
                        id,
                        NodeLayout {
                            bounds,
                            border,
                            scroll,
                        },
                    );
                }
            }
        })
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::VirtualTree;
    use crate::tests::*;
    use gpui::{TestAppContext, point};

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
        let label = tree.create_node("#text").unwrap();
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
}
