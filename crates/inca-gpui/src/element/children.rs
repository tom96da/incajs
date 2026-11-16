// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reports the box of each rendered child of an element to its sink.

use gpui::prelude::*;
use gpui::{Bounds, Pixels, ScrollHandle, Size, point, px};

use super::build::NodeLayout;
use super::scrollbar::{Scrollbars, gutter_of};
use super::wheel::snapped_extra;
use crate::event_sink::EventSink;
use inca_node::scroll::Rect;
use inca_node::spec::{DisplaySpec, ElementSpec, ElementTag};
use inca_node::tree::NodeId;

/// The children of `spec` that become `gpui` elements, in order. An empty
/// text child renders nothing.
pub(super) fn rendered_children(spec: &ElementSpec) -> impl Iterator<Item = &ElementSpec> {
    spec.children
        .iter()
        .filter(|child| child.tag != ElementTag::Text(String::new()))
}

/// A rendered child, with its border and gutter unless it has `display: none`.
type Recorded = (NodeId, Option<(Pixels, Size<Pixels>)>);

/// Whether the container `spec` with `handle` gets a bar element after its children: it has a
/// handle and an axis that scrolls.
pub(super) fn has_bar(spec: &ElementSpec, handle: Option<&ScrollHandle>) -> bool {
    let axes = spec.style.scroll_axes();
    handle.is_some() && (axes.x || axes.y)
}

/// The strips the bars of the node `spec` reserve in `mode`: `width` for the vertical bar and
/// `height` for the horizontal bar. Zero for a node that gets no bar, which is a node without
/// a scroll handle in `sink` or without a scrolling axis. The host records the document element
/// with it.
#[must_use]
pub fn node_gutter<E: EventSink>(spec: &ElementSpec, sink: &E, mode: Scrollbars) -> Size<Pixels> {
    // `scroll_handle` keeps a handle at the first ask, so only a scroll container asks.
    let handle = spec
        .style
        .is_scroll_container()
        .then(|| sink.scroll_handle(spec.id))
        .flatten();
    if has_bar(spec, handle.as_ref()) {
        gutter_of(spec.style.scroll_keyword_axes(), mode)
    } else {
        Size::default()
    }
}

/// Reports the box of each rendered child of `spec` to `sink` once `element`
/// has laid its children out. A child with `display: none` has no box and
/// reports nothing. `trailing` counts the elements the container holds after
/// its children, which is 1 when it got a bar element.
pub(super) fn record_children<E>(
    element: gpui::Div,
    spec: &ElementSpec,
    handle: Option<ScrollHandle>,
    sink: E,
    mode: Scrollbars,
    trailing: usize,
) -> gpui::Div
where
    E: EventSink + 'static,
{
    // The border and the gutter of each rendered child; `None` for `display: none`.
    #[allow(clippy::cast_possible_truncation)]
    let children: Vec<Recorded> = rendered_children(spec)
        .map(|child| {
            let shown = child.style.display != Some(DisplaySpec::None);
            let border = px(child.style.border_width.unwrap_or(0.0) as f32);
            let gutter = if shown {
                node_gutter(child, &sink, mode)
            } else {
                Size::default()
            };
            (child.id, shown.then_some((border, gutter)))
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
            // The tracker and the scroll anchor come before the children, and the bar element
            // after them.
            let first = raw.len().saturating_sub(children.len() + trailing);
            for (&(id, shown), bounds) in children.iter().zip(&raw[first..]) {
                if let Some((border, gutter)) = shown {
                    let bounds = Bounds::new(bounds.origin + offset, bounds.size);
                    let scroll = point(-offset.x, -offset.y);
                    sink.record_layout(
                        id,
                        NodeLayout {
                            bounds,
                            border,
                            scroll,
                            gutter,
                        },
                    );
                }
            }
        })
    })
}

/// The box `bounds` shrunk by `inset` on every side, in window coordinates. A
/// negative size reads as 0.
#[must_use]
pub fn rect_of(bounds: Bounds<Pixels>, inset: Pixels) -> Rect {
    Rect {
        x: f32::from(bounds.origin.x + inset),
        y: f32::from(bounds.origin.y + inset),
        width: f32::from(bounds.size.width - inset * 2.0).max(0.0),
        height: f32::from(bounds.size.height - inset * 2.0).max(0.0),
    }
}

impl NodeLayout {
    /// The client rectangle in window coordinates: the border box inside the border and beside
    /// the strips of the bars, each size floored at 0. A scrolled node aligns to it.
    #[must_use]
    pub fn client_rect(&self) -> Rect {
        let rect = rect_of(self.bounds, self.border);
        Rect {
            width: (rect.width - f32::from(self.gutter.width)).max(0.0),
            height: (rect.height - f32::from(self.gutter.height)).max(0.0),
            ..rect
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::event_sink::noop_sink_methods;
    use crate::tests::*;
    use gpui::{Bounds, TestAppContext, point};
    use inca_node::VirtualTree;
    use inca_test_macros::gpui_cases;
    use rstest::rstest;

    #[test]
    fn a_border_wider_than_half_the_box_leaves_a_zero_size_view() {
        let bounds = Bounds::new(point(px(10.0), px(10.0)), gpui::size(px(10.0), px(6.0)));
        let view = rect_of(bounds, px(8.0));
        assert_eq!((view.width, view.height), (0.0, 0.0));
        assert_eq!((view.x, view.y), (18.0, 18.0));
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

    /// A sink that gives no scroll handle, otherwise a probe.
    #[derive(Clone)]
    struct NoHandles(Probe);

    impl EventSink for NoHandles {
        fn tracks_pointer(&self) -> bool {
            self.0.tracks_pointer()
        }

        fn fire(
            &self,
            start: NodeId,
            event: &str,
            payload: &crate::EventPayload,
            window: &mut gpui::Window,
            cx: &mut gpui::App,
        ) -> bool {
            self.0.fire(start, event, payload, window, cx)
        }

        fn focus_handle(&self, _: NodeId) -> Option<gpui::FocusHandle> {
            None
        }

        fn scroll_handle(&self, _: NodeId) -> Option<ScrollHandle> {
            None
        }

        fn wheel_axes(&self, _: NodeId) -> Option<inca_node::WheelAxes> {
            None
        }

        noop_sink_methods!(
            clear_layout,
            record_layout,
            pointer_moved,
            pointer_left,
            modifiers_changed,
            hover_changed,
            pointer_over,
            pointer_input,
            pointer_released,
            key_input,
            input,
            record_input,
            input_request,
            bar_pressed,
            bar_released,
        );
    }

    fn classic_probe(tree: VirtualTree) -> Probe {
        Probe {
            mode: Scrollbars::Classic,
            ..probe_of(tree)
        }
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::scroll("scroll", 15., 15.)]
    #[case::auto("auto", 0., 0.)]
    #[case::visible("visible", 0., 0.)]
    fn a_child_records_the_strips_of_its_scroll_value(
        cx: &mut TestAppContext,
        overflow: &str,
        width: f32,
        height: f32,
    ) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 200., 200.);
        let child = boxed(&mut tree, Some(root), 50., 50.);
        tree.set_style(child, "overflow", overflow).unwrap();
        let probe = classic_probe(tree);
        BarScene::with_probe(cx, probe.clone());
        assert_eq!(
            probe.layouts.borrow()[&child].gutter,
            gpui::size(px(width), px(height))
        );
    }

    #[test]
    fn a_child_with_no_scroll_handle_records_no_strips() {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 200., 200.);
        tree.set_style(root, "overflow", "scroll").unwrap();
        let spec = inca_node::build_spec(&tree, root).unwrap();
        let sink = NoHandles(classic_probe(tree));
        assert_eq!(
            node_gutter(&spec, &sink, Scrollbars::Classic),
            Size::default()
        );
    }

    #[gpui::test]
    fn a_plain_child_gets_no_scroll_handle(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 200., 200.);
        let scroller = boxed(&mut tree, Some(root), 50., 50.);
        tree.set_style(scroller, "overflow", "scroll").unwrap();
        let plain = boxed(&mut tree, Some(root), 50., 50.);
        let probe = classic_probe(tree);
        BarScene::with_probe(cx, probe.clone());
        let handles = probe.handles.borrow();
        assert!(handles.contains_key(&scroller) && !handles.contains_key(&plain));
    }

    #[gpui::test]
    fn the_bar_element_leaves_the_recorded_box_of_the_last_child(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = boxed(&mut tree, None, 100., 100.);
        tree.set_style(root, "overflow", "scroll").unwrap();
        let child = boxed(&mut tree, Some(root), 300., 300.);
        let probe = classic_probe(tree);
        BarScene::with_probe(cx, probe.clone());
        assert_eq!(
            probe.layouts.borrow()[&child].bounds,
            rect(0., 0., 300., 300.)
        );
    }

    #[rstest]
    #[case::plain(rect(10., 20., 100., 80.), 2., (15., 12.), (12., 22., 81., 64.))]
    #[case::narrower_than_the_strip(rect(0., 0., 10., 6.), 4., (15., 15.), (4., 4., 0., 0.))]
    fn the_client_rectangle_leaves_out_the_border_and_the_strips(
        #[case] bounds: Bounds<Pixels>,
        #[case] border: f32,
        #[case] gutter: (f32, f32),
        #[case] want: (f32, f32, f32, f32),
    ) {
        let layout = NodeLayout {
            bounds,
            border: px(border),
            scroll: gpui::Point::default(),
            gutter: gpui::size(px(gutter.0), px(gutter.1)),
        };
        let rect = layout.client_rect();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), want);
    }
}
