// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::prelude::*;
use gpui::{Global, Pixels, Point, ScrollHandle, Window, div, point, px};

use crate::event_sink::EventSink;
use inca_node::NodeId;
use inca_node::spec::{ElementSpec, StyleSpec, WheelAxes};

/// The tracked scroll containers under the pointer for the wheel event being
/// dispatched, innermost first, without the root.
#[derive(Default)]
pub(super) struct WheelChain(pub(super) Vec<(NodeId, ScrollHandle)>);

impl Global for WheelChain {}

/// The wheel delta the root container may still apply. The pointer tracker
/// sets it after the listeners ran, and the root's own listener, which runs
/// last in the bubble phase and only when the pointer is over the root, takes it.
#[derive(Default)]
pub(super) struct RootStep(pub(super) Option<Point<Pixels>>);

impl Global for RootStep {}

/// The scroll change a wheel `delta` makes along the scrolling `axes`. A
/// delta on both axes moves along y when the magnitudes tie.
pub(super) fn axis_step(axes: WheelAxes, delta: Point<Pixels>) -> Point<Pixels> {
    let along = |own: bool, main: Pixels, cross: Pixels, cross_scrolls: bool| {
        if !own {
            Pixels::ZERO
        } else if main != Pixels::ZERO {
            main
        } else if !cross_scrolls {
            cross
        } else {
            Pixels::ZERO
        }
    };
    let mut x = along(axes.x, delta.x, delta.y, axes.y);
    let mut y = along(axes.y, delta.y, delta.x, axes.x);
    if x != Pixels::ZERO && y != Pixels::ZERO {
        if x.abs() > y.abs() {
            y = Pixels::ZERO;
        } else {
            x = Pixels::ZERO;
        }
    }
    point(x, y)
}

/// The offset a wheel `delta` moves `handle` to, clamped to the content of the
/// last layout. `None` when the clamped offset stays where it is.
pub(super) fn next_offset(
    axes: WheelAxes,
    handle: &ScrollHandle,
    delta: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let max = handle.max_offset();
    let clamp = |offset: Point<Pixels>| {
        point(
            offset.x.clamp(-max.x, px(0.)),
            offset.y.clamp(-max.y, px(0.)),
        )
    };
    let offset = handle.offset();
    let next = clamp(offset + axis_step(axes, delta));
    (next != clamp(offset)).then_some(next)
}

/// A zero-size child that starts the scroll range of a container with `style`
/// at the edge of its padding box. `gpui` measures the range from the smallest
/// origin of all child boxes to the farthest end of any child box, adds the
/// padding and subtracts the border box, so the anchor sits twice the border
/// width before the content box on each axis.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn scroll_anchor(style: &StyleSpec) -> impl IntoElement {
    let border = style.border_width.unwrap_or(0.0) * 2.0;
    let inset = |padding: Option<f64>| px((padding.unwrap_or(0.0) - border) as f32);
    div()
        .absolute()
        .top(inset(style.padding.top))
        .left(inset(style.padding.left))
        .size_0()
}

/// Tracks a scroll container's [`ScrollHandle`]. A container below the root
/// adds itself to the [`WheelChain`] in the bubble phase, which runs innermost
/// first. The root applies its [`RootStep`] in its own listener.
pub(super) fn wire_scroll<Elem, E>(
    element: Elem,
    handle: Option<ScrollHandle>,
    spec: &ElementSpec,
    root: bool,
    dispatch: Option<&E>,
) -> Elem
where
    Elem: StatefulInteractiveElement + ParentElement + FluentBuilder,
    E: EventSink + Clone + 'static,
{
    let id = spec.id;
    element.when_some(handle, |el, handle| {
        let el = el.track_scroll(&handle);
        if !root {
            return el.on_scroll_wheel(move |_, _, cx| {
                cx.default_global::<WheelChain>()
                    .0
                    .push((id, handle.clone()));
            });
        }
        el.when_some(dispatch.cloned(), |el, sink| {
            el.on_scroll_wheel(move |_, window, cx| {
                if let Some(delta) = cx.default_global::<RootStep>().0.take()
                    && let Some(next) = sink
                        .wheel_axes(id)
                        .and_then(|axes| next_offset(axes, &handle, delta))
                {
                    handle.set_offset(next);
                    window.refresh();
                }
            })
        })
    })
}

/// What the snapped offset `total` adds to bounds already placed at the
/// snapped offset `outer`.
pub(super) fn snapped_extra(
    window: &Window,
    total: Point<Pixels>,
    outer: Point<Pixels>,
) -> Point<Pixels> {
    window.pixel_snap_point(total) - window.pixel_snap_point(outer)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::event_sink::EventSink;
    use crate::tests::*;
    use gpui::{Context, Render, ScrollDelta, ScrollWheelEvent, StyleRefinement, Window};
    use gpui::{Pixels, Point, TestAppContext, VisualTestContext, point, px};
    use inca_node::tree::{NodeId, VirtualTree};
    use inca_test_macros::gpui_cases;

    struct Wheels {
        probe: Probe,
        cx: VisualTestContext,
    }

    /// Draws `ProbeView` through a cached view, which a draw reuses until the
    /// window refreshes.
    struct CachedShell(gpui::Entity<ProbeView>);

    impl Render for CachedShell {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::AnyView::from(self.0.clone()).cached(StyleRefinement::default().size_full())
        }
    }

    impl Wheels {
        /// Mounts and draws `tree`, rooted at node 0.
        fn new(cx: &mut TestAppContext, tree: VirtualTree) -> Self {
            Self::with_probe(cx, probe_of(tree))
        }

        /// Like [`Self::new`], with a probe that tracks no pointer.
        fn untracked(cx: &mut TestAppContext, tree: VirtualTree) -> Self {
            let probe = Probe {
                tracks: false,
                ..probe_of(tree)
            };
            Self::with_probe(cx, probe)
        }

        /// Like [`Self::new`], with the tree drawn through a cached view.
        fn cached(cx: &mut TestAppContext, tree: VirtualTree) -> Self {
            let probe = probe_of(tree);
            let view = probe.clone();
            let window = cx.add_window(|_, cx| CachedShell(cx.new(|_| ProbeView(view))));
            Self::draw_window(cx, probe, window.into())
        }

        fn with_probe(cx: &mut TestAppContext, probe: Probe) -> Self {
            let view = probe.clone();
            let window = cx.add_window(|_, _| ProbeView(view));
            Self::draw_window(cx, probe, window.into())
        }

        fn draw_window(
            cx: &mut TestAppContext,
            probe: Probe,
            window: gpui::AnyWindowHandle,
        ) -> Self {
            cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
            Self {
                probe,
                cx: VisualTestContext::from_window(window, cx),
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
    fn an_inert_container_leaves_the_wheel_to_its_ancestor(cx: &mut TestAppContext) {
        let mut tree = nested_containers();
        tree.set_attribute(1, "inert", true).unwrap();
        let mut scene = Wheels::new(cx, tree);
        scene.wheel(-30.);
        assert_eq!(scene.offset(1).y, px(0.));
        assert_eq!(scene.offset(0).y, px(-30.));
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
    fn an_x_dominant_wheel_moves_a_y_only_container_by_its_y_delta(cx: &mut TestAppContext) {
        let tree = styled_container(&[("overflow_x", "hidden"), ("overflow_y", "scroll")]);
        let mut scene = Wheels::new(cx, tree);
        scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-50.), px(-10.))));
        assert_eq!(scene.offset(0), point(px(0.), px(-10.)));
    }

    #[gpui::test]
    fn a_vertical_wheel_scrolls_an_x_only_container_along_x(cx: &mut TestAppContext) {
        let mut scene = Wheels::new(
            cx,
            styled_container(&[("overflow_x", "scroll"), ("overflow_y", "hidden")]),
        );
        scene.wheel(-50.);
        assert_eq!(scene.offset(0), point(px(-50.), px(0.)));
    }

    #[gpui::test]
    fn a_step_past_the_inner_end_leaves_the_outer_unmoved(cx: &mut TestAppContext) {
        let mut scene = Wheels::new(cx, nested_containers());
        scene.wheel(-150.);
        scene.wheel(-100.);
        assert_eq!(scene.offset(1).y, px(-200.));
        assert_eq!(scene.offset(0).y, px(0.));
    }

    #[gpui::test]
    fn a_wheel_scrolls_nested_containers_without_pointer_tracking(cx: &mut TestAppContext) {
        let mut scene = Wheels::untracked(cx, nested_containers());
        scene.wheel(-200.);
        scene.wheel(-50.);
        assert_eq!(scene.offset(1).y, px(-200.));
        assert_eq!(scene.offset(0).y, px(-50.));
    }

    #[gpui::test]
    fn a_line_wheel_moves_by_the_window_line_height(cx: &mut TestAppContext) {
        let mut scene = Wheels::new(cx, one_container());
        let line = scene.cx.update(|window, _| window.line_height());
        scene.wheel_delta(10., ScrollDelta::Lines(point(0., -2.)));
        assert_eq!(scene.offset(0).y, line * -2.);
    }

    #[gpui::test]
    fn a_diagonal_wheel_of_equal_axes_moves_along_y(cx: &mut TestAppContext) {
        let tree = styled_container(&[("overflow_x", "scroll"), ("overflow_y", "scroll")]);
        let mut scene = Wheels::new(cx, tree);
        scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-10.), px(-10.))));
        assert_eq!(scene.offset(0), point(px(0.), px(-10.)));
    }

    #[gpui::test]
    fn a_diagonal_wheel_moves_a_both_axes_container_along_the_larger_delta(
        cx: &mut TestAppContext,
    ) {
        let tree = styled_container(&[("overflow_x", "scroll"), ("overflow_y", "scroll")]);
        let mut scene = Wheels::new(cx, tree);
        scene.wheel_delta(10., ScrollDelta::Pixels(point(px(-20.), px(-10.))));
        assert_eq!(scene.offset(0), point(px(-20.), px(0.)));
    }

    #[gpui::test]
    fn a_wheel_outside_the_root_container_leaves_it_alone(cx: &mut TestAppContext) {
        let mut scene = Wheels::new(cx, one_container());
        scene.wheel_at(150., -50.);
        assert_eq!(scene.offset(0).y, px(0.));
    }

    #[gpui::test]
    fn a_container_left_in_the_chain_does_not_take_the_next_wheel(cx: &mut TestAppContext) {
        let mut tree = VirtualTree::new();
        let root = div(&mut tree, None, 200., false);
        for _ in 0..2 {
            let sibling = div(&mut tree, Some(root), 100., true);
            div(&mut tree, Some(sibling), 300., false);
        }
        let mut scene = Wheels::new(cx, tree);
        let first = scene.probe.scroll_handle(1).unwrap();
        scene
            .cx
            .update(|_, cx| cx.default_global::<WheelChain>().0.push((1, first)));
        scene.wheel_at(110., -30.);
        assert_eq!(scene.offset(1).y, px(0.));
        assert_eq!(scene.offset(3).y, px(-30.));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::root_container(one_container(), 1)]
    #[case::inner_container(nested_containers(), 2)]
    fn a_wheel_step_redraws_a_cached_view(
        cx: &mut TestAppContext,
        tree: VirtualTree,
        child: NodeId,
    ) {
        let mut scene = Wheels::cached(cx, tree);
        scene.wheel(-30.);
        assert_eq!(scene.probe.layouts.borrow()[&child].scroll.y, px(30.));
    }
}
