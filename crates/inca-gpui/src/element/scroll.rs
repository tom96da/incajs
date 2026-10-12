// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::prelude::*;
use gpui::{
    App, DispatchPhase, Global, Pixels, Point, ScrollHandle, ScrollWheelEvent, Window, canvas,
    point, px,
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
