// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The element that draws the bars of a scroll container, and the state it keeps between frames:
//! when a thumb drawn over the content shows, which thumb the pointer is over and the thumb drag.

use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    Axis, Bounds, Context, ElementId, Hitbox, HitboxBehavior, Hsla, Pixels, ScrollHandle, canvas,
    fill, px, rgb,
};

use super::hover::sync_over_bar;
use super::press::{Ctx, listen};
use super::{Bar, HIDE_DELAY, Parts, Scrollbars, TRACK, gutter_of, hit_rects, parts, thumb_color};
use crate::element::wheel::{WheelScrolled, anchor_insets};
use crate::event_sink::EventSink;
use inca_node::NodeId;
use inca_node::spec::ElementSpec;

/// What a container keeps between frames about its bars.
// Four independent flags; a state enum would need a variant per combination.
#[allow(clippy::struct_excessive_bools)]
pub(super) struct ScrollbarState {
    /// Counts the showings of a thumb, so a timer that a later showing replaced does nothing.
    generation: u64,
    /// Whether a thumb drawn over the content shows.
    pub(super) shown: bool,
    /// Whether the content overflowed at the last frame.
    overflowed: bool,
    /// Whether a press on a bar is waiting for its release.
    pub(super) pressed_on_bar: bool,
    /// The mode of the last frame.
    mode: Scrollbars,
    /// Whether the pointer lay over a bar at the last check.
    pub(super) over_bar: bool,
    /// The axis whose thumb lies under the pointer.
    pub(super) hover: Option<Axis>,
    /// The thumb drag in progress.
    pub(super) drag: Option<Drag>,
}

/// A thumb drag: the bar as the frame of the press drew it and the distance from the thumb start
/// to the pointer along the axis at the press.
#[derive(Clone, Copy)]
pub(super) struct Drag {
    pub(super) bar: Bar,
    pub(super) grab: f32,
}

impl ScrollbarState {
    fn new(mode: Scrollbars, overflowed: bool, cx: &mut Context<Self>) -> Self {
        let mut state = Self {
            generation: 0,
            shown: true,
            overflowed,
            pressed_on_bar: false,
            mode,
            over_bar: false,
            hover: None,
            drag: None,
        };
        if mode == Scrollbars::Overlay {
            state.show(cx);
        }
        state
    }

    /// Shows the thumb and hides it again after `HIDE_DELAY`, unless a later showing replaced
    /// this one.
    pub(super) fn show(&mut self, cx: &mut Context<Self>) {
        self.shown = true;
        self.generation += 1;
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HIDE_DELAY).await;
            this.update(cx, |state, cx| {
                if state.generation == generation && state.drag.is_none() {
                    state.shown = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
}

/// The key of the state of the container `id`.
pub(super) fn state_key(id: NodeId) -> ElementId {
    ElementId::NamedInteger("scrollbar".into(), u64::from(id))
}

/// The quads to paint, in paint order: the tracks, the corner, then the thumbs. A thumb with no
/// reserved strip shows on a classic platform, and on an overlay platform while `state` shows it.
pub(super) fn paint_plan(
    parts: &Parts,
    state: &ScrollbarState,
    mode: Scrollbars,
) -> Vec<(Bounds<Pixels>, Hsla)> {
    let track = Hsla::from(rgb(TRACK));
    let tracks = parts
        .bars()
        .filter(|bar| bar.strip)
        .map(|bar| (bar.track, track));
    let corner = parts.corner.map(|corner| (corner, track));
    let thumbs = parts.bars().filter_map(|bar| {
        let thumb = bar.thumb?;
        let hover = state.hover == Some(bar.axis)
            || state.drag.is_some_and(|drag| drag.bar.axis == bar.axis);
        let shown = bar.strip || mode == Scrollbars::Classic || state.shown;
        shown.then(|| (thumb, thumb_color(hover, bar.strip)))
    });
    tracks.chain(corner).chain(thumbs).collect()
}

/// What the prepaint of the bars hands to their paint.
struct Frame<E> {
    plan: Vec<(Bounds<Pixels>, Hsla)>,
    ctx: Rc<Ctx<E>>,
}

/// The element that draws the bars of the container `spec`, and answers to the pointer over
/// them. It is a zero-size child placed where the scroll anchor is, after the children, so it
/// paints over them and its listeners run before theirs.
///
/// `sink` reports the pointer over a bar when `track` holds. A press on a bar is claimed
/// whatever `track` says.
#[allow(clippy::cast_possible_truncation)]
pub(in crate::element) fn scrollbar<E: EventSink + Clone + 'static>(
    spec: &ElementSpec,
    handle: ScrollHandle,
    mode: Scrollbars,
    sink: Option<E>,
    track: bool,
) -> impl IntoElement {
    let id = spec.id;
    let inert = spec.inert;
    let axes = spec.style.scroll_axes();
    let gutter = gutter_of(spec.style.scroll_keyword_axes(), mode);
    let border = px(spec.style.border_width.unwrap_or(0.0) as f32);
    let sink = sink.filter(|_| track && !inert);
    let (top, left) = anchor_insets(&spec.style);
    canvas(
        move |_, window, cx| {
            let frame = handle.bounds();
            let max = handle.max_offset();
            let parts = parts(frame, border, gutter, axes, max, handle.offset());
            let overflow = (axes.x && max.x > px(0.)) || (axes.y && max.y > px(0.));
            let state = window.use_keyed_state(state_key(id), cx, |_, cx| {
                ScrollbarState::new(mode, overflow, cx)
            });
            let wheel = cx.default_global::<WheelScrolled>().0.remove(&id);
            state.update(cx, |state, cx| {
                let switched = mode != state.mode;
                if mode == Scrollbars::Overlay
                    && (wheel || (overflow && !state.overflowed) || switched)
                {
                    state.show(cx);
                }
                state.mode = mode;
                state.overflowed = overflow;
            });
            let drag_ended = state.update(cx, |state, cx| {
                let ended = state.drag.is_some_and(|drag| {
                    let bar = match drag.bar.axis {
                        Axis::Horizontal => parts.x,
                        Axis::Vertical => parts.y,
                    };
                    bar.is_none_or(|bar| bar.thumb.is_none())
                });
                if ended {
                    state.drag = None;
                    if mode == Scrollbars::Overlay {
                        state.show(cx);
                    }
                }
                ended
            });
            // The sink restyles and fires events, which a prepaint must not run.
            if drag_ended && let Some(sink) = sink.clone() {
                window.defer(cx, move |window, cx| sink.bar_drag(id, false, window, cx));
            }
            let (plan, rects) = {
                let state = state.read(cx);
                (
                    paint_plan(&parts, state, mode),
                    hit_rects(&parts, mode, state.shown),
                )
            };
            let hitboxes: Vec<Hitbox> = if inert {
                Vec::new()
            } else {
                rects
                    .iter()
                    .map(|(_, rect)| {
                        window.insert_hitbox(*rect, HitboxBehavior::BlockMouseExceptScroll)
                    })
                    .collect()
            };
            let ctx = Rc::new(Ctx {
                id,
                state,
                handle: handle.clone(),
                parts,
                mode,
                frame,
                rects: rects.into_iter().map(|(_, rect)| rect).collect(),
                hitboxes,
                sink,
            });
            Frame { plan, ctx }
        },
        move |_, frame, window, cx| {
            for (bounds, colour) in frame.plan {
                window.paint_quad(fill(bounds, colour));
            }
            if frame.ctx.sink.is_some() {
                sync_over_bar(&frame.ctx, Some(window.mouse_position()), window, cx);
            }
            if !inert {
                listen(window, frame.ctx);
            }
        },
    )
    .absolute()
    .top(top)
    .left(left)
    .size_0()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{Entity, Size, TestAppContext, point, px, size};
    use inca_node::spec::WheelAxes;
    use rstest::rstest;

    use super::*;
    use crate::tests::{BarScene, bar_tree, capture_entities};
    use inca_node::set_style;

    type States = Rc<RefCell<Vec<Entity<ScrollbarState>>>>;

    const Y: WheelAxes = WheelAxes { x: false, y: true };
    /// Scrolls on y alone, so the container has one bar.
    const Y_ONLY: [(&str, &str); 2] = [("overflow_x", "hidden"), ("overflow_y", "scroll")];
    const Y_AUTO: [(&str, &str); 2] = [("overflow_x", "hidden"), ("overflow_y", "auto")];

    fn scene(
        cx: &mut TestAppContext,
        mode: Scrollbars,
        props: &[(&str, &str)],
    ) -> (BarScene, States) {
        let states = capture_entities(cx);
        (BarScene::new(cx, mode, bar_tree(props)), states)
    }

    /// The paint plan of node 0 in `mode`, with the strip `gutter`.
    fn plan(
        scene: &mut BarScene,
        states: &States,
        mode: Scrollbars,
        gutter: Size<Pixels>,
    ) -> Vec<(Bounds<Pixels>, Hsla)> {
        let handle = scene.handle(0);
        let parts = parts(
            handle.bounds(),
            px(0.),
            gutter,
            Y,
            handle.max_offset(),
            handle.offset(),
        );
        let state = states.borrow()[0].clone();
        scene
            .cx
            .update(|_, cx| paint_plan(&parts, state.read(cx), mode))
    }

    /// Whether the overlay thumb of node 0 is in the plan.
    fn thumb_shown(scene: &mut BarScene, states: &States) -> bool {
        !plan(scene, states, Scrollbars::Overlay, Size::default()).is_empty()
    }

    /// Lets the timers of `delay` run and redraws.
    fn wait(scene: &mut BarScene, delay: Duration) {
        scene.cx.executor().advance_clock(delay);
        scene.cx.run_until_parked();
        scene.frame();
    }

    #[gpui::test]
    fn an_overlay_thumb_shows_at_the_first_draw(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        assert!(thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn an_overlay_thumb_hides_after_the_delay(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        wait(&mut scene, HIDE_DELAY);
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn the_hide_repaints_the_container(cx: &mut TestAppContext) {
        let (scene, _states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        let before = scene.probe.renders.get();
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        assert!(scene.probe.renders.get() > before);
    }

    #[gpui::test]
    fn a_script_write_of_the_offset_shows_no_overlay_thumb(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        wait(&mut scene, HIDE_DELAY);
        scene.handle(0).set_offset(point(px(0.), px(-20.)));
        scene.frame();
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn overflow_that_appears_after_the_thumb_hid_shows_it(cx: &mut TestAppContext) {
        let states: States = capture_entities(cx);
        let mut tree = bar_tree(&Y_ONLY);
        set_style(&mut tree, 1, "height", 50.0).unwrap();
        let mut scene = BarScene::new(cx, Scrollbars::Overlay, tree);
        wait(&mut scene, HIDE_DELAY);
        set_style(&mut scene.probe.tree.borrow_mut(), 1, "height", 300.0).unwrap();
        scene.frame();
        assert!(thumb_shown(&mut scene, &states));
        wait(&mut scene, HIDE_DELAY);
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn a_frame_that_keeps_its_overflow_after_the_thumb_hid_shows_nothing(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        wait(&mut scene, HIDE_DELAY);
        scene.frame();
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn a_wheel_step_that_moves_the_content_shows_the_overlay_thumb(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        wait(&mut scene, HIDE_DELAY);
        scene.wheel((50., 50.), -40.);
        assert!(thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn a_wheel_step_at_the_end_of_the_range_shows_the_overlay_thumb(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        scene.handle(0).set_offset(point(px(0.), px(-200.)));
        wait(&mut scene, HIDE_DELAY);
        scene.wheel((50., 50.), -40.);
        assert!(thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn pointer_movement_over_the_container_shows_no_overlay_thumb(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        wait(&mut scene, HIDE_DELAY);
        scene.move_to((50., 50.));
        scene.frame();
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn a_frame_after_a_wheel_step_shows_nothing_more(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        wait(&mut scene, HIDE_DELAY);
        scene.wheel((50., 50.), -40.);
        wait(&mut scene, HIDE_DELAY);
        scene.frame();
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn a_second_showing_restarts_the_delay(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        let almost = Duration::from_millis(999);
        wait(&mut scene, almost);
        scene.wheel((50., 50.), -10.);
        wait(&mut scene, Duration::from_millis(1));
        assert!(thumb_shown(&mut scene, &states));
        wait(&mut scene, almost);
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn a_timer_that_fires_after_the_container_left_the_tree_does_nothing(cx: &mut TestAppContext) {
        let states: States = capture_entities(cx);
        let mut tree = bar_tree(&[]);
        let scroller = tree.create_node("div").unwrap();
        for (key, value) in [("width", 100.0), ("height", 100.0)] {
            set_style(&mut tree, scroller, key, value).unwrap();
        }
        set_style(&mut tree, scroller, "overflow_y", "scroll").unwrap();
        tree.append_child(0, scroller).unwrap();
        let tall = tree.create_node("div").unwrap();
        set_style(&mut tree, tall, "height", 300.0).unwrap();
        tree.append_child(scroller, tall).unwrap();
        let mut scene = BarScene::new(cx, Scrollbars::Overlay, tree);
        scene
            .probe
            .tree
            .borrow_mut()
            .remove_child(0, scroller)
            .unwrap();
        scene.frame();
        states.borrow_mut().clear();
        let before = scene.probe.renders.get();
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        assert_eq!(scene.probe.renders.get(), before);
    }

    #[gpui::test]
    fn a_container_switched_from_classic_to_overlay_hides_its_thumb_after_the_delay(
        cx: &mut TestAppContext,
    ) {
        let (mut scene, states) = scene(cx, Scrollbars::Classic, &Y_ONLY);
        scene.probe.mode_now.set(Some(Scrollbars::Overlay));
        scene.frame();
        wait(&mut scene, HIDE_DELAY);
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn a_classic_auto_thumb_shows_at_the_first_draw(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Classic, &Y_AUTO);
        let plan = plan(&mut scene, &states, Scrollbars::Classic, Size::default());
        assert_eq!(plan.len(), 1);
    }

    #[gpui::test]
    fn a_classic_auto_thumb_stays_after_the_delay(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Classic, &Y_AUTO);
        scene.handle(0).set_offset(point(px(0.), px(-20.)));
        wait(&mut scene, HIDE_DELAY);
        let plan = plan(&mut scene, &states, Scrollbars::Classic, Size::default());
        assert_eq!(plan.len(), 1);
    }

    #[rstest]
    #[case::fits(0., 1)]
    #[case::overflows(200., 2)]
    fn a_strip_bar_paints_a_thumb_only_when_the_content_overflows(
        #[case] max: f32,
        #[case] quads: usize,
    ) {
        let state = ScrollbarState {
            generation: 0,
            shown: false,
            overflowed: false,
            pressed_on_bar: false,
            mode: Scrollbars::Overlay,
            over_bar: false,
            hover: None,
            drag: None,
        };
        let parts = parts(
            Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))),
            px(0.),
            size(px(15.), px(0.)),
            Y,
            point(px(0.), px(max)),
            point(px(0.), px(0.)),
        );
        assert_eq!(paint_plan(&parts, &state, Scrollbars::Overlay).len(), quads);
    }

    #[gpui::test]
    fn a_bar_with_no_strip_paints_nothing_when_the_content_fits(cx: &mut TestAppContext) {
        let states: States = capture_entities(cx);
        let mut tree = bar_tree(&Y_ONLY);
        set_style(&mut tree, 1, "height", 50.0).unwrap();
        let mut scene = BarScene::new(cx, Scrollbars::Overlay, tree);
        assert!(plan(&mut scene, &states, Scrollbars::Overlay, Size::default()).is_empty());
    }

    #[gpui::test]
    fn an_overlay_thumb_does_not_hide_during_a_drag(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        scene.press((95., 10.), gpui::MouseButton::Left);
        wait(&mut scene, HIDE_DELAY);
        assert!(thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn an_overlay_thumb_hides_after_the_delay_that_the_release_of_a_drag_starts(
        cx: &mut TestAppContext,
    ) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        scene.press((95., 10.), gpui::MouseButton::Left);
        wait(&mut scene, HIDE_DELAY);
        scene.release((95., 10.), gpui::MouseButton::Left);
        let renders = scene.probe.renders.get();
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        assert!(scene.probe.renders.get() > renders);
        assert!(!thumb_shown(&mut scene, &states));
    }

    #[gpui::test]
    fn the_end_of_a_drag_at_a_paint_starts_the_delay_of_an_overlay_thumb(cx: &mut TestAppContext) {
        let (mut scene, states) = scene(cx, Scrollbars::Overlay, &Y_ONLY);
        scene.press((95., 10.), gpui::MouseButton::Left);
        wait(&mut scene, HIDE_DELAY);
        set_style(&mut scene.probe.tree.borrow_mut(), 1, "height", 50.0).unwrap();
        scene.frame();
        wait(&mut scene, HIDE_DELAY);
        let state = states.borrow()[0].clone();
        assert!(!scene.cx.update(|_, cx| state.read(cx).shown));
    }

    #[gpui::test]
    fn a_drag_in_a_classic_container_starts_no_timer(cx: &mut TestAppContext) {
        let (mut scene, _states) = scene(cx, Scrollbars::Classic, &Y_ONLY);
        scene.press((92., 10.), gpui::MouseButton::Left);
        scene.release((92., 10.), gpui::MouseButton::Left);
        let renders = scene.probe.renders.get();
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        assert_eq!(scene.probe.renders.get(), renders);
    }

    #[gpui::test]
    fn the_end_of_a_drag_at_a_paint_starts_no_timer_in_a_classic_container(
        cx: &mut TestAppContext,
    ) {
        let (mut scene, _states) = scene(cx, Scrollbars::Classic, &Y_ONLY);
        scene.press((92., 10.), gpui::MouseButton::Left);
        set_style(&mut scene.probe.tree.borrow_mut(), 1, "height", 50.0).unwrap();
        scene.frame();
        let renders = scene.probe.renders.get();
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        assert_eq!(scene.probe.renders.get(), renders);
    }
}
