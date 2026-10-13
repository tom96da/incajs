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
type Snapshot = (ScrollHandle, Point<Pixels>, bool, bool);

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
                snapshots.push((handle.clone(), handle.offset(), inert, root));
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
#[derive(Default)]
struct WheelKept(Option<(ScrollHandle, Point<Pixels>, Point<Pixels>)>);

impl Global for WheelKept {}

/// Undoes the wheel steps of every container but the innermost one that can
/// still move, and returns the kept container. `settled` true restores that
/// one too.
fn undo_steps(
    snapshots: Vec<Snapshot>,
    mut settled: bool,
) -> Option<(ScrollHandle, Point<Pixels>, Point<Pixels>)> {
    let mut kept = None;
    for (handle, before, inert, _) in snapshots.into_iter().rev() {
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
            kept = Some((handle, before, after));
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
    let (root, below): (Vec<_>, Vec<_>) = all.into_iter().partition(|s| s.3);
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
    if let Some((handle, before, after)) = kept
        && prevented
        && handle.offset() == after
    {
        handle.set_offset(before);
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
