// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The drag of a thumb and the hover state of a thumb.

use gpui::{App, Axis, MouseMoveEvent, Pixels, Point, Window, point, px};

use super::press::Ctx;
use super::view::Drag;
use super::{Scrollbars, drag_offset, thumb_zone};
use crate::event_sink::EventSink;

impl<E> Ctx<E> {
    /// The drag a left press at `position` starts: on the drag zone of a thumb that has room to
    /// move.
    pub(super) fn drag_at(&self, position: Point<Pixels>) -> Option<Drag> {
        self.parts.bars().find_map(|bar| {
            let thumb = bar.thumb?;
            let zone = thumb_zone(bar)?;
            if bar.travel <= 0. || !zone.contains(&position) {
                return None;
            }
            Some(Drag {
                bar: *bar,
                grab: coord(bar.axis, position) - coord(bar.axis, thumb.origin),
            })
        })
    }

    /// The axis whose drawn thumb lies under `position` on an area that answers to the pointer.
    pub(super) fn thumb_at(&self, position: Point<Pixels>, window: &Window) -> Option<Axis> {
        let answers =
            self.over_bar(window) && self.rects.iter().any(|rect| rect.contains(&position));
        let bar = self
            .parts
            .bars()
            .find(|bar| bar.thumb.is_some_and(|thumb| thumb.contains(&position)));
        bar.filter(|_| answers).map(|bar| bar.axis)
    }

    /// Ends the drag in progress. A thumb drawn over the content hides after its delay again.
    pub(super) fn end_drag(&self, window: &mut Window, cx: &mut App)
    where
        E: EventSink,
    {
        let mode = self.mode;
        self.state.update(cx, |state, cx| {
            state.drag = None;
            if mode == Scrollbars::Overlay {
                state.show(cx);
            }
            cx.notify();
        });
        if let Some(sink) = &self.sink {
            sink.bar_drag(self.id, false, window, cx);
        }
    }

    /// Sets the hover state of the thumb to `axis`, and repaints on a change.
    pub(super) fn set_hover(&self, axis: Option<Axis>, cx: &mut App) {
        self.state.update(cx, |state, cx| {
            if state.hover != axis {
                state.hover = axis;
                cx.notify();
            }
        });
    }

    /// Moves the content along a drag at the pointer move `event`. A move with no button held
    /// ends the drag. Then the hover state of the thumb follows the pointer.
    pub(super) fn drag_move(&self, event: &MouseMoveEvent, window: &mut Window, cx: &mut App)
    where
        E: EventSink,
    {
        let dragging = self.state.read(cx).drag;
        if let Some(drag) = dragging {
            if event.pressed_button.is_none() {
                self.end_drag(window, cx);
            } else {
                let pointer = coord(drag.bar.axis, event.position);
                let position = -px(drag_offset(&drag.bar, drag.grab, pointer));
                let offset = self.handle.offset();
                self.handle.set_offset(match drag.bar.axis {
                    Axis::Horizontal => point(position, offset.y),
                    Axis::Vertical => point(offset.x, position),
                });
                window.refresh();
            }
        }
        self.set_hover(self.thumb_at(event.position, window), cx);
    }
}

/// The coordinate of `point` along `axis`.
fn coord(axis: Axis, point: Point<Pixels>) -> f32 {
    match axis {
        Axis::Vertical => f32::from(point.y),
        Axis::Horizontal => f32::from(point.x),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use gpui::{Hsla, MouseButton, MouseExitEvent, TestAppContext, point, px, rgb};
    use inca_style::spec::WheelAxes;
    use inca_test_macros::gpui_cases;

    use super::*;
    use crate::Scrollbars;
    use crate::element::scrollbar::view::{ScrollbarState, paint_plan};
    use crate::element::scrollbar::{HIDE_DELAY, THUMB_HOVER, gutter_of, parts};
    use crate::tests::{BarScene, capture_entities, nested_bar_tree};
    use inca_style::set_style;

    const BOTH: [(&str, &str); 1] = [("overflow", "scroll")];
    const Y_SCROLL: [(&str, &str); 2] = [("overflow_x", "hidden"), ("overflow_y", "scroll")];
    const LEFT: MouseButton = MouseButton::Left;
    /// On the thumb of the vertical bar of `Y_SCROLL`, which spans y 3 to 34.
    const THUMB: (f32, f32) = (92., 10.);
    /// A drag of 20 along the track scrolls `200 * 20 / 63`.
    const DRAGGED: f32 = 63.49;

    fn classic(cx: &mut TestAppContext, props: &[(&str, &str)]) -> BarScene {
        BarScene::new(cx, Scrollbars::Classic, nested_bar_tree(props))
    }

    /// The scroll offset of the scroller (node 1) as `(x, y)`, rounded to 0.01.
    fn offset(scene: &BarScene) -> (f32, f32) {
        let offset = scene.handle(1).offset();
        let hundredths = |value: gpui::Pixels| (f32::from(value) * 100.).round() / 100.;
        (hundredths(offset.x), hundredths(offset.y))
    }

    fn bars(scene: &BarScene) -> Vec<(String, u32)> {
        scene.probe.bars.borrow().clone()
    }

    #[gpui::test]
    fn dragging_the_vertical_thumb_sets_the_offset(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.drag_to((92., 30.));
        assert_eq!(offset(&scene), (0., -DRAGGED));
    }

    /// Drags the horizontal thumb of a scroller that starts scrolled by 30 on y.
    fn drag_the_horizontal_thumb(cx: &mut TestAppContext) -> BarScene {
        let mut scene = classic(cx, &BOTH);
        scene.handle(1).set_offset(point(px(0.), px(-30.)));
        scene.frame();
        scene.press((10., 92.), LEFT);
        scene.drag_to((30., 92.));
        scene
    }

    #[gpui::test]
    fn dragging_the_horizontal_thumb_sets_x(cx: &mut TestAppContext) {
        let scene = drag_the_horizontal_thumb(cx);
        assert_eq!(offset(&scene).0, -75.44);
    }

    #[gpui::test]
    fn dragging_the_horizontal_thumb_keeps_y(cx: &mut TestAppContext) {
        let scene = drag_the_horizontal_thumb(cx);
        assert_eq!(offset(&scene).1, -30.);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::thumb(92.)]
    #[case::inset_before(86.)]
    #[case::inset_after(98.)]
    fn a_press_across_the_strip_beside_the_thumb_drags_like_one_on_it(
        cx: &mut TestAppContext,
        x: f32,
    ) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press((x, 10.), LEFT);
        scene.drag_to((x, 40.));
        assert_eq!(offset(&scene), (0., -95.24));
    }

    #[gpui::test]
    fn a_press_on_the_track_beyond_the_thumb_starts_no_drag(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press((92., 60.), LEFT);
        scene.drag_to((92., 80.));
        assert_eq!(offset(&scene), (0., 0.));
    }

    #[gpui::test]
    fn a_press_just_beyond_the_thumb_end_starts_no_drag(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press((92., 36.), LEFT);
        assert_eq!(bars(&scene), [("bar_pressed".to_owned(), 1)]);
    }

    #[gpui::test]
    fn a_press_on_a_thumb_that_fills_its_region_starts_no_drag(cx: &mut TestAppContext) {
        let props = [
            ("overflow_x", "hidden"),
            ("overflow_y", "scroll"),
            ("height", "20px"),
        ];
        let mut scene = classic(cx, &props);
        scene.press(THUMB, LEFT);
        assert_eq!(bars(&scene), [("bar_pressed".to_owned(), 1)]);
    }

    #[gpui::test]
    fn a_press_on_a_bar_with_no_overflow_starts_no_drag(cx: &mut TestAppContext) {
        let mut tree = nested_bar_tree(&Y_SCROLL);
        set_style(&mut tree, 2, "height", 50.0).unwrap();
        let mut scene = BarScene::new(cx, Scrollbars::Classic, tree);
        scene.press(THUMB, LEFT);
        assert_eq!(bars(&scene), [("bar_pressed".to_owned(), 1)]);
    }

    #[gpui::test]
    fn a_drag_move_redraws_without_further_input(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.move_to(THUMB);
        scene.press(THUMB, LEFT);
        scene.drag_to((92., 30.));
        let y = f32::from(scene.probe.layouts.borrow()[&2].scroll.y);
        assert!((63.0..64.0).contains(&y), "{y}");
    }

    #[gpui::test]
    fn a_drag_ends_at_the_next_paint_when_its_axis_has_no_thumb(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        set_style(&mut scene.probe.tree.borrow_mut(), 2, "height", 50.0).unwrap();
        scene.frame();
        assert_eq!(bars(&scene).last(), Some(&("bar_drag_end".to_owned(), 1)));
    }

    #[gpui::test]
    fn the_sink_hears_the_end_of_a_drag_after_the_paint_not_during_it(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        set_style(&mut scene.probe.tree.borrow_mut(), 2, "height", 50.0).unwrap();
        let renders = scene.probe.renders.get();
        scene.frame();
        scene.cx.run_until_parked();
        // A call made while drawing cannot request the redraw that the bridge's restyle needs.
        assert_eq!(scene.probe.renders.get(), renders + 2);
    }

    #[gpui::test]
    fn a_move_with_no_button_held_ends_the_drag(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.move_to((92., 30.));
        assert_eq!(offset(&scene), (0., 0.));
        assert_eq!(bars(&scene).last(), Some(&("bar_drag_end".to_owned(), 1)));
    }

    #[gpui::test]
    fn a_release_off_the_thumb_ends_the_drag(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.release((50., 50.), LEFT);
        scene.drag_to((92., 30.));
        assert_eq!(offset(&scene), (0., 0.));
    }

    #[gpui::test]
    fn a_drag_is_reported_after_the_press_that_starts_it(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        let want = [("bar_pressed", 1), ("bar_drag", 1)];
        assert_eq!(bars(&scene), want.map(|(name, id)| (name.to_owned(), id)));
    }

    #[gpui::test]
    fn the_end_of_a_drag_is_reported_after_the_release(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.release(THUMB, LEFT);
        let want = [("bar_released", 1), ("bar_drag_end", 1)];
        assert_eq!(
            bars(&scene)[2..],
            want.map(|(name, id)| (name.to_owned(), id))
        );
    }

    #[gpui::test]
    fn a_press_on_the_track_ends_a_drag_whose_release_was_lost(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.press((92., 60.), LEFT);
        assert!(bars(&scene).contains(&("bar_drag_end".to_owned(), 1)));
    }

    #[gpui::test]
    fn a_right_press_on_the_thumb_keeps_a_live_drag(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.press(THUMB, MouseButton::Right);
        assert!(!bars(&scene).contains(&("bar_drag_end".to_owned(), 1)));
    }

    #[gpui::test]
    fn a_right_press_in_the_content_keeps_a_live_drag(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.press((50., 50.), MouseButton::Right);
        assert!(!bars(&scene).contains(&("bar_drag_end".to_owned(), 1)));
    }

    #[gpui::test]
    fn the_release_of_another_button_keeps_the_drag(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        scene.press(THUMB, LEFT);
        scene.release(THUMB, MouseButton::Right);
        scene.drag_to((92., 30.));
        assert_eq!(offset(&scene), (0., -DRAGGED));
    }

    /// The colors of the quads that node 1 paints.
    fn colors(
        scene: &mut BarScene,
        states: &crate::tests::Entities<ScrollbarState>,
        mode: Scrollbars,
    ) -> Vec<Hsla> {
        let handle = scene.handle(1);
        let axes = WheelAxes { x: false, y: true };
        let gutter = gutter_of(axes, mode);
        let parts = parts(
            handle.bounds(),
            px(0.),
            gutter,
            axes,
            handle.max_offset(),
            handle.offset(),
        );
        let state = states.borrow()[0].clone();
        let plan = scene
            .cx
            .update(|_, cx| paint_plan(&parts, state.read(cx), mode));
        plan.into_iter().map(|(_, color)| color).collect()
    }

    fn hover(
        scene: &mut BarScene,
        states: &crate::tests::Entities<ScrollbarState>,
    ) -> Option<Axis> {
        let state = states.borrow()[0].clone();
        scene.cx.update(|_, cx| state.read(cx).hover)
    }

    #[gpui::test]
    fn a_move_over_the_thumb_sets_the_hover_color(cx: &mut TestAppContext) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut scene = classic(cx, &Y_SCROLL);
        scene.move_to(THUMB);
        let hover = Hsla::from(rgb(THUMB_HOVER));
        assert!(colors(&mut scene, &states, Scrollbars::Classic).contains(&hover));
    }

    #[gpui::test]
    fn a_move_over_the_thumb_repaints_the_container(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_SCROLL);
        let renders = scene.probe.renders.get();
        scene.move_to(THUMB);
        assert!(scene.probe.renders.get() > renders);
    }

    #[gpui::test]
    fn a_scrollbar_above_the_thumb_takes_the_hover_from_it(cx: &mut TestAppContext) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut tree = nested_bar_tree(&Y_SCROLL);
        let cover = crate::tests::boxed(&mut tree, Some(0), 100., 100.);
        set_style(&mut tree, cover, "margin_top", -150.0).unwrap();
        // The track of a scroller blocks the pointer for what lies beneath it.
        set_style(&mut tree, cover, "overflow_y", "scroll").unwrap();
        set_style(&mut tree, cover, "overflow_x", "hidden").unwrap();
        let mut scene = BarScene::new(cx, Scrollbars::Classic, tree);
        scene.move_to(THUMB);
        assert_eq!(hover(&mut scene, &states), None);
    }

    #[gpui::test]
    fn a_move_off_the_thumb_clears_the_hover(cx: &mut TestAppContext) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut scene = classic(cx, &Y_SCROLL);
        scene.move_to(THUMB);
        scene.move_to((50., 50.));
        assert_eq!(hover(&mut scene, &states), None);
    }

    #[gpui::test]
    fn a_pointer_leaving_the_window_clears_the_hover(cx: &mut TestAppContext) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut scene = classic(cx, &Y_SCROLL);
        scene.move_to(THUMB);
        scene.cx.simulate_event(MouseExitEvent {
            position: point(px(100.), px(10.)),
            ..Default::default()
        });
        assert_eq!(hover(&mut scene, &states), None);
    }

    /// An overlay scene whose thumb hid.
    fn hidden_overlay(
        cx: &mut TestAppContext,
    ) -> (BarScene, crate::tests::Entities<ScrollbarState>) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut scene = BarScene::new(cx, Scrollbars::Overlay, nested_bar_tree(&Y_SCROLL));
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        scene.frame();
        (scene, states)
    }

    #[gpui::test]
    fn the_place_of_a_hidden_overlay_thumb_takes_no_hover(cx: &mut TestAppContext) {
        let (mut scene, states) = hidden_overlay(cx);
        scene.move_to((95., 10.));
        assert_eq!(hover(&mut scene, &states), None);
    }

    #[gpui::test]
    fn the_place_of_a_hidden_overlay_thumb_starts_no_drag(cx: &mut TestAppContext) {
        let (mut scene, _states) = hidden_overlay(cx);
        scene.press((95., 10.), LEFT);
        assert!(bars(&scene).is_empty());
    }

    #[gpui::test]
    fn a_shown_overlay_thumb_takes_the_hover_color(cx: &mut TestAppContext) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut scene = BarScene::new(cx, Scrollbars::Overlay, nested_bar_tree(&Y_SCROLL));
        scene.move_to((95., 10.));
        let hover = Hsla::from(rgb(THUMB_HOVER));
        assert!(colors(&mut scene, &states, Scrollbars::Overlay).contains(&hover));
    }
}
