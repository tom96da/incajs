// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Keeps a container hovered while the pointer lies over one of its bars.

use gpui::{App, Global, Point, Window};

use super::press::Ctx;
use crate::element::pointer::report_hover;
use crate::event_sink::EventSink;
use inca_node::NodeId;

/// The container whose bar lies under the pointer for the input being handled. A move clears it
/// at its capture, and the bars set it again at a move and at paint.
#[derive(Default)]
pub(in crate::element) struct OverBar(pub(in crate::element) Option<NodeId>);

impl Global for OverBar {}

impl OverBar {
    /// Whether the bar of `id` lies under the pointer.
    pub(in crate::element) fn holds(cx: &App, id: NodeId) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|over| over.0 == Some(id))
    }
}

/// Records whether `pointer` (`None` when it left the window) lies over a bar of the container,
/// and reports the hover that the bar's box hides from the container's own box. Returns whether
/// the pointer lies over a bar. A bar that another box covers at the pointer does not count. The
/// hit test of a frame is final after its prepaint, so call this from paint.
pub(super) fn sync_over_bar<E: EventSink>(
    ctx: &Ctx<E>,
    pointer: Option<Point<gpui::Pixels>>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(sink) = &ctx.sink else {
        return false;
    };
    let over = pointer.is_some_and(|p| ctx.rects.iter().any(|rect| rect.contains(&p)))
        && ctx.over_bar(window);
    let was = ctx
        .state
        .update(cx, |state, _| std::mem::replace(&mut state.over_bar, over));
    let id = ctx.id;
    if over {
        cx.default_global::<OverBar>().0 = Some(id);
    } else if OverBar::holds(cx, id) {
        cx.default_global::<OverBar>().0 = None;
    }
    if over && !was {
        report_hover(sink, id, true, window, cx);
    } else if was && !over && !pointer.is_some_and(|p| ctx.frame.contains(&p)) {
        report_hover(sink, id, false, window, cx);
    }
    over
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gpui::{MouseExitEvent, TestAppContext, point, px};

    use super::*;
    use crate::Scrollbars;
    use crate::element::scrollbar::HIDE_DELAY;
    use crate::element::scrollbar::view::ScrollbarState;
    use crate::tests::{BarScene, boxed, capture_entities, nested_bar_tree};
    use inca_node::set_style;

    const BOTH: [(&str, &str); 1] = [("overflow", "scroll")];
    const Y_SCROLL: [(&str, &str); 2] = [("overflow_x", "hidden"), ("overflow_y", "scroll")];
    /// On the track of the scroller (node 1).
    const TRACK: (f32, f32) = (92., 50.);
    const CONTENT: (f32, f32) = (50., 50.);
    /// On the sibling (node 3), outside the scroller.
    const SIBLING: (f32, f32) = (20., 120.);
    const OUTSIDE: (f32, f32) = (150., 150.);

    /// The hover reports of the scroller, in order.
    fn reports(scene: &BarScene) -> Vec<bool> {
        let hovers = scene.probe.hovers.borrow();
        hovers
            .iter()
            .filter(|(id, _)| *id == 1)
            .map(|(_, hovered)| *hovered)
            .collect()
    }

    fn classic(cx: &mut TestAppContext) -> BarScene {
        BarScene::new(cx, Scrollbars::Classic, nested_bar_tree(&BOTH))
    }

    /// Moves to `at` and draws the frame that reports the hover.
    fn hover(scene: &mut BarScene, at: (f32, f32)) {
        scene.move_to(at);
        scene.frame();
    }

    #[gpui::test]
    fn a_move_over_a_classic_track_leaves_the_container_hovered(cx: &mut TestAppContext) {
        let mut scene = classic(cx);
        hover(&mut scene, CONTENT);
        hover(&mut scene, TRACK);
        assert!(!reports(&scene).contains(&false));
    }

    #[gpui::test]
    fn a_thumb_shown_under_a_stationary_pointer_keeps_the_container_hovered(
        cx: &mut TestAppContext,
    ) {
        let mut scene = BarScene::new(cx, Scrollbars::Overlay, nested_bar_tree(&Y_SCROLL));
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        scene.frame();
        hover(&mut scene, (95., 50.));
        scene.wheel((95., 50.), -40.);
        assert!(!reports(&scene).contains(&false));
    }

    #[gpui::test]
    fn the_pointer_leaving_the_window_from_a_bar_ends_the_hover(cx: &mut TestAppContext) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut scene = classic(cx);
        hover(&mut scene, TRACK);
        scene.cx.simulate_event(MouseExitEvent {
            position: point(px(100.), px(50.)),
            ..Default::default()
        });
        let state = states.borrow()[0].clone();
        assert!(!scene.cx.update(|_, cx| state.read(cx).over_bar));
        assert!(!scene.cx.update(|_, cx| OverBar::holds(cx, 1)));
        assert_eq!(reports(&scene).last(), Some(&false));
    }

    #[gpui::test]
    fn a_move_from_a_track_to_a_sibling_ends_the_hover(cx: &mut TestAppContext) {
        let mut scene = classic(cx);
        hover(&mut scene, TRACK);
        hover(&mut scene, SIBLING);
        assert_eq!(reports(&scene).last(), Some(&false));
    }

    #[gpui::test]
    fn a_move_from_a_track_into_the_content_reports_no_end_of_the_hover(cx: &mut TestAppContext) {
        let mut scene = classic(cx);
        hover(&mut scene, TRACK);
        hover(&mut scene, CONTENT);
        assert!(!reports(&scene).contains(&false));
    }

    #[gpui::test]
    fn a_move_from_outside_onto_a_track_starts_the_hover_once(cx: &mut TestAppContext) {
        let mut scene = classic(cx);
        hover(&mut scene, OUTSIDE);
        scene.probe.hovers.borrow_mut().clear();
        hover(&mut scene, TRACK);
        assert_eq!(reports(&scene), [true]);
    }

    #[gpui::test]
    fn a_bar_under_the_bar_of_a_later_sibling_reports_no_hover(cx: &mut TestAppContext) {
        let mut tree = nested_bar_tree(&BOTH);
        let cover = boxed(&mut tree, Some(0), 100., 100.);
        set_style(&mut tree, cover, "margin_top", -150.0).unwrap();
        set_style(&mut tree, cover, "overflow", "scroll").unwrap();
        boxed(&mut tree, Some(cover), 300., 300.);
        let mut scene = BarScene::new(cx, Scrollbars::Classic, tree);
        scene.probe.hovers.borrow_mut().clear();
        hover(&mut scene, TRACK);
        assert_eq!(reports(&scene), [false]);
    }

    #[gpui::test]
    fn a_bar_that_appears_under_a_stationary_pointer_starts_the_hover(cx: &mut TestAppContext) {
        let mut scene = BarScene::new(cx, Scrollbars::Classic, nested_bar_tree(&[]));
        hover(&mut scene, TRACK);
        scene.probe.hovers.borrow_mut().clear();
        {
            let mut tree = scene.probe.tree.borrow_mut();
            set_style(&mut tree, 1, "overflow", "scroll").unwrap();
        }
        scene.frame();
        assert_eq!(reports(&scene), [true]);
    }
}
