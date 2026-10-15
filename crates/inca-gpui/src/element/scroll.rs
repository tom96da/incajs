// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::prelude::*;
use gpui::{
    App, Bounds, DispatchPhase, Global, Pixels, Point, ScrollHandle, ScrollWheelEvent, Window,
    canvas, point, px,
};

use super::spec::ElementSpec;

/// Every scroll container's offset before the wheel event being dispatched,
/// outermost first, and whether the container is inert.
#[derive(Default)]
struct WheelSnapshots(Vec<Snapshot>);

/// A container's handle, its offset before the wheel event, whether it is
/// inert and whether it is the root, whose step runs after the root's bubble
/// handler.
struct Snapshot {
    handle: ScrollHandle,
    before: Point<Pixels>,
    inert: bool,
    root: bool,
}

impl Global for WheelSnapshots {}

/// A zero-size child whose paint records its container's offset in the
/// capture phase, which runs before any bubble-phase scroll step. Children
/// paint after their parent, and a container adds this before its own
/// children, so containers record outermost first.
fn scroll_recorder(handle: ScrollHandle, inert: bool, root: bool) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            window.on_mouse_event(move |_: &ScrollWheelEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let snapshots = &mut cx.default_global::<WheelSnapshots>().0;
                let first = snapshots.is_empty();
                snapshots.push(Snapshot {
                    handle: handle.clone(),
                    before: handle.offset(),
                    inert,
                    root,
                });
                if first {
                    window.defer(cx, settle_wheel);
                }
            });
        },
    )
    .absolute()
    .size_0()
}

/// The container whose wheel step [`roll_back_extra_steps`] kept: its handle,
/// the offset before the step and the offset after it.
struct Kept {
    handle: ScrollHandle,
    before: Point<Pixels>,
    after: Point<Pixels>,
}

#[derive(Default)]
struct WheelKept(Option<Kept>);

impl Global for WheelKept {}

/// Undoes the wheel steps of every container but the innermost one that can
/// still move, and returns the kept container. `settled` true restores that
/// one too.
fn undo_steps(snapshots: Vec<Snapshot>, mut settled: bool) -> Option<Kept> {
    let mut kept = None;
    for Snapshot {
        handle,
        before,
        inert,
        ..
    } in snapshots.into_iter().rev()
    {
        // An inert container never scrolls and leaves the wheel to the next.
        if inert {
            handle.set_offset(before);
            continue;
        }
        // `gpui` clamps the offset on the next draw; compare as it will.
        let max = handle.max_offset();
        let clamp = |offset: Point<Pixels>| {
            point(
                offset.x.clamp(-max.x, px(0.)),
                offset.y.clamp(-max.y, px(0.)),
            )
        };
        // An unclamped `before` past the end would always differ from `moved`.
        if !settled && clamp(handle.offset()) != clamp(before) {
            settled = true;
            let after = handle.offset();
            kept = Some(Kept {
                handle,
                before,
                after,
            });
        } else {
            handle.set_offset(before);
        }
    }
    kept
}

/// Runs in the root's bubble phase, once `gpui`'s scroll steps have run and
/// before the JS listeners. Restores every container but the innermost one
/// that can still move, so a listener's own scroll writes stay.
pub(super) fn roll_back_extra_steps(cx: &mut App) {
    let all = std::mem::take(&mut cx.default_global::<WheelSnapshots>().0);
    // The root's own step is still to come; `settle_wheel` handles it.
    let (root, below): (Vec<_>, Vec<_>) = all.into_iter().partition(|s| s.root);
    cx.default_global::<WheelSnapshots>().0 = root;
    cx.default_global::<WheelKept>().0 = undo_steps(below, false);
}

/// Runs before the next draw, after the JS listeners. Restores the kept
/// container when a listener prevented the default and left its offset at
/// the wheel step's result.
///
/// A `gpui` that applies the wheel step later than the event breaks this.
fn settle_wheel(window: &mut Window, cx: &mut App) {
    let snapshots = std::mem::take(&mut cx.default_global::<WheelSnapshots>().0);
    let prevented = window.default_prevented();
    let kept = cx.default_global::<WheelKept>().0.take();
    // The root's snapshot, or all of them when the root's bubble phase did not run.
    undo_steps(snapshots, prevented || kept.is_some());
    if let Some(kept) = kept
        && prevented
        && kept.handle.offset() == kept.after
    {
        kept.handle.set_offset(kept.before);
    }
}

/// Tracks a scroll container's [`ScrollHandle`] and records its offset for
/// [`settle_wheel`], which keeps an inert container at that offset. Without a
/// handle `gpui` scrolls it on its own.
pub(super) fn wire_scroll<Elem>(
    element: Elem,
    handle: Option<ScrollHandle>,
    spec: &ElementSpec,
    root: bool,
) -> Elem
where
    Elem: StatefulInteractiveElement + ParentElement + FluentBuilder,
{
    element.when_some(handle, |el, handle| {
        el.track_scroll(&handle)
            .child(scroll_recorder(handle, spec.inert, root))
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

/// A box in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Where the node's edge or centre lands in the visible box, per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// The start edges meet.
    Start,
    /// The centres meet.
    Center,
    /// The end edges meet.
    End,
    /// A node that shows stays put. Any other node moves the least that
    /// brings its near edge in. A node longer than the box aligns its end when
    /// its start lies before the box, and its start when its end lies after
    /// it. A node that covers the whole box stays put.
    Nearest,
    /// A node that shows stays put. A node that lies outside the box
    /// centres. A node that shows partly follows `Nearest`.
    Reveal,
    /// A node that shows fully or partly stays put. A node that lies outside
    /// the box centres.
    CenterOutside,
}

fn axis_delta(node_start: f32, node_len: f32, view_start: f32, view_len: f32, align: Align) -> f32 {
    let (node_end, view_end) = (node_start + node_len, view_start + view_len);
    let center = (node_start + node_len / 2.0) - (view_start + view_len / 2.0);
    let nearest = || {
        let (before, after) = (node_start < view_start, node_end > view_end);
        let long = node_len > view_len;
        if before && after {
            0.0
        } else if (before && !long) || (after && long) {
            node_start - view_start
        } else if before || after {
            node_end - view_end
        } else {
            0.0
        }
    };
    match align {
        Align::Start => node_start - view_start,
        Align::End => node_end - view_end,
        Align::Center => center,
        Align::Nearest => nearest(),
        Align::Reveal | Align::CenterOutside => {
            if node_start >= view_start && node_end <= view_end {
                0.0
            } else if node_end <= view_start || node_start >= view_end {
                center
            } else if align == Align::Reveal {
                nearest()
            } else {
                0.0
            }
        }
    }
}

/// The scroll change `(dx, dy)` that moves `node` to `inline` (horizontal)
/// and `block` (vertical) alignment in `view`. A positive value scrolls
/// towards the end of the axis.
#[must_use]
pub fn scroll_delta(node: Rect, view: Rect, block: Align, inline: Align) -> (f32, f32) {
    (
        axis_delta(node.x, node.width, view.x, view.width, inline),
        axis_delta(node.y, node.height, view.y, view.height, block),
    )
}

/// Scrolls each container of `chain`, innermost first, to show `node`.
/// `chain` pairs a container with its visible box. `scroll` receives the
/// container and the wanted change, and returns the change it applied. The
/// node moves by the applied change before the next container is handled.
pub fn scroll_chain<I: Copy>(
    mut node: Rect,
    chain: &[(I, Rect)],
    block: Align,
    inline: Align,
    mut scroll: impl FnMut(I, f32, f32) -> (f32, f32),
) {
    for &(container, view) in chain {
        let (dx, dy) = scroll_delta(node, view, block, inline);
        let (applied_x, applied_y) = scroll(container, dx, dy);
        node.x -= applied_x;
        node.y -= applied_y;
    }
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

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::element::tests::*;
    use crate::event_sink::EventSink;
    use crate::tree::{NodeId, VirtualTree};
    use gpui::{Bounds, Pixels, Point, TestAppContext, VisualTestContext, point, px};

    const fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    // The box spans 100..200 on both axes.
    const VIEW: Rect = rect(100.0, 100.0, 100.0, 100.0);

    fn dy(top: f32, height: f32, block: Align) -> f32 {
        scroll_delta(rect(100.0, top, 10.0, height), VIEW, block, Align::Nearest).1
    }

    #[test]
    fn start_end_and_center_align_the_matching_edges() {
        assert_eq!(dy(130.0, 20.0, Align::Start), 30.0);
        assert_eq!(dy(130.0, 20.0, Align::End), -50.0);
        assert_eq!(dy(130.0, 20.0, Align::Center), -10.0);
    }

    #[test]
    fn nearest_keeps_a_node_that_shows() {
        assert_eq!(dy(130.0, 20.0, Align::Nearest), 0.0);
        assert_eq!(dy(100.0, 100.0, Align::Nearest), 0.0);
    }

    #[test]
    fn nearest_brings_the_near_edge_in_for_a_node_outside_the_box() {
        assert_eq!(dy(40.0, 20.0, Align::Nearest), -60.0);
        assert_eq!(dy(260.0, 20.0, Align::Nearest), 80.0);
    }

    #[test]
    fn nearest_moves_a_partly_shown_node_the_least() {
        assert_eq!(dy(90.0, 20.0, Align::Nearest), -10.0);
        assert_eq!(dy(190.0, 20.0, Align::Nearest), 10.0);
    }

    #[test]
    fn nearest_aligns_the_start_of_a_longer_node_that_ends_after_the_box() {
        assert_eq!(dy(150.0, 300.0, Align::Nearest), 50.0);
        assert_eq!(dy(260.0, 300.0, Align::Nearest), 160.0);
    }

    #[test]
    fn nearest_aligns_the_end_of_a_longer_node_that_starts_before_the_box() {
        assert_eq!(dy(60.0, 130.0, Align::Nearest), -10.0);
        assert_eq!(dy(20.0, 120.0, Align::Nearest), -60.0);
        assert_eq!(dy(-50.0, 140.0, Align::Nearest), -110.0);
    }

    #[test]
    fn nearest_keeps_a_node_that_covers_the_box() {
        assert_eq!(dy(50.0, 300.0, Align::Nearest), 0.0);
    }

    #[test]
    fn reveal_keeps_a_node_that_shows() {
        assert_eq!(dy(130.0, 20.0, Align::Reveal), 0.0);
    }

    #[test]
    fn reveal_centers_a_node_outside_the_box() {
        assert_eq!(dy(40.0, 20.0, Align::Reveal), -100.0);
        assert_eq!(dy(260.0, 20.0, Align::Reveal), 120.0);
        assert_eq!(dy(200.0, 20.0, Align::Reveal), 60.0);
        assert_eq!(dy(80.0, 20.0, Align::Reveal), -60.0);
    }

    #[test]
    fn reveal_moves_a_partly_shown_node_the_least() {
        assert_eq!(dy(90.0, 20.0, Align::Reveal), -10.0);
        assert_eq!(dy(190.0, 20.0, Align::Reveal), 10.0);
        assert_eq!(dy(150.0, 300.0, Align::Reveal), 50.0);
    }

    #[test]
    fn center_outside_keeps_a_node_that_shows_fully_or_partly() {
        let center_outside = |top, height| dy(top, height, Align::CenterOutside);
        assert_eq!(center_outside(130.0, 20.0), 0.0);
        assert_eq!(center_outside(90.0, 20.0), 0.0);
        assert_eq!(center_outside(190.0, 20.0), 0.0);
        assert_eq!(center_outside(150.0, 300.0), 0.0);
    }

    #[test]
    fn a_zero_size_node_inside_or_on_the_edge_of_the_box_shows() {
        for top in [100.0, 150.0, 200.0] {
            assert_eq!(dy(top, 0.0, Align::Reveal), 0.0);
            assert_eq!(dy(top, 0.0, Align::CenterOutside), 0.0);
        }
    }

    #[test]
    fn a_zero_size_node_outside_the_box_centers() {
        assert_eq!(dy(60.0, 0.0, Align::Reveal), -90.0);
        assert_eq!(dy(260.0, 0.0, Align::CenterOutside), 110.0);
    }

    #[test]
    fn a_box_of_zero_height_centers_a_node_outside_it() {
        let view = rect(100.0, 100.0, 100.0, 0.0);
        let node = rect(100.0, 140.0, 10.0, 20.0);
        assert_eq!(
            scroll_delta(node, view, Align::Reveal, Align::Reveal).1,
            50.0
        );
    }

    #[test]
    fn a_border_wider_than_half_the_box_leaves_a_zero_size_view() {
        let bounds = Bounds::new(point(px(10.0), px(10.0)), gpui::size(px(10.0), px(6.0)));
        let view = rect_of(bounds, px(8.0));
        assert_eq!((view.width, view.height), (0.0, 0.0));
        assert_eq!((view.x, view.y), (18.0, 18.0));
    }

    #[test]
    fn the_axes_take_their_own_alignment() {
        let node = rect(260.0, 40.0, 20.0, 20.0);
        let delta = scroll_delta(node, VIEW, Align::Start, Align::End);
        assert_eq!(delta, (80.0, -60.0));
    }

    #[test]
    fn a_container_sees_the_node_shifted_by_the_scroll_of_the_one_before() {
        let node = rect(0.0, 500.0, 10.0, 20.0);
        let chain = [
            (1, rect(0.0, 0.0, 50.0, 100.0)),
            (2, rect(0.0, 0.0, 50.0, 300.0)),
        ];
        let mut seen = Vec::new();
        scroll_chain(node, &chain, Align::Start, Align::Nearest, |id, dx, dy| {
            seen.push((id, dx, dy));
            // The inner container reaches only half of the way.
            (dx, if id == 1 { dy / 2.0 } else { dy })
        });
        assert_eq!(seen, [(1, 0.0, 500.0), (2, 0.0, 250.0)]);
    }

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
}
