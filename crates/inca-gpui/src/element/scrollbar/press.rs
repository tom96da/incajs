// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The presses, releases and moves on the bars of a container. A press on a bar goes to the
//! container's sink as a bar press, and no other listener sees it.

use std::rc::Rc;

use gpui::{
    App, Bounds, DispatchPhase, Entity, Hitbox, MouseButton, MouseDownEvent, MouseExitEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, ScrollHandle, Window,
};

use super::hover::sync_over_bar;
use super::view::ScrollbarState;
use super::{Parts, Scrollbars};
use crate::element::pointer::{
    control_click_is_right, mark_target, move_over, opens_context_menu, restored_down, restored_up,
};
use crate::event_sink::{EventSink, context_menu_payload, mouse_down_payload, mouse_up_payload};
use inca_node::NodeId;

/// What the listeners of one frame need to know about the bars of a container.
pub(super) struct Ctx<E> {
    pub(super) id: NodeId,
    pub(super) state: Entity<ScrollbarState>,
    pub(super) handle: ScrollHandle,
    /// The bars as the frame drew them.
    pub(super) parts: Parts,
    pub(super) mode: Scrollbars,
    /// The border box of the container.
    pub(super) frame: Bounds<Pixels>,
    /// The areas that answer to the pointer.
    pub(super) rects: Vec<Bounds<Pixels>>,
    /// One hit box per area. Empty for an inert container.
    pub(super) hitboxes: Vec<Hitbox>,
    /// Receives the reports, `None` when the container reports nothing.
    pub(super) sink: Option<E>,
}

impl<E> Ctx<E> {
    pub(super) fn over_bar(&self, window: &Window) -> bool {
        self.hitboxes.iter().any(|hitbox| hitbox.is_hovered(window))
    }
}

/// Registers the listeners of the bars. They run in the bubble phase, after the listeners
/// of the container, so a press they stop never reaches the container.
pub(super) fn listen<E: EventSink + Clone + 'static>(window: &mut Window, ctx: Rc<Ctx<E>>) {
    let pressed = Rc::clone(&ctx);
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble {
            press(&pressed, event, window, cx);
        }
    });
    let released = Rc::clone(&ctx);
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble {
            release(&released, event, window, cx);
        }
    });
    let moved = Rc::clone(&ctx);
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble {
            movement(&moved, event, window, cx);
        }
    });
    window.on_mouse_event(move |_: &MouseExitEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble {
            ctx.set_hover(None, cx);
            sync_over_bar(&ctx, None, window, cx);
        }
    });
}

fn press<E: EventSink>(ctx: &Ctx<E>, event: &MouseDownEvent, window: &mut Window, cx: &mut App) {
    let on_bar = ctx.over_bar(window);
    ctx.state
        .update(cx, |state, _| state.pressed_on_bar = on_bar);
    if !on_bar {
        // A drag still set at a new left press lost its release.
        if event.button == MouseButton::Left && ctx.state.read(cx).drag.is_some() {
            ctx.end_drag(window, cx);
        }
        return;
    }
    mark_target(cx, ctx.id);
    let drag = ctx
        .drag_at(event.position)
        .filter(|_| event.button == MouseButton::Left);
    let mode = ctx.mode;
    // A left press that starts no drag ends one whose release was lost.
    let lost = event.button == MouseButton::Left && ctx.state.read(cx).drag.is_some();
    if drag.is_none() && lost {
        ctx.end_drag(window, cx);
    }
    if let Some(drag) = drag {
        ctx.state.update(cx, |state, cx| {
            state.drag = Some(drag);
            if mode == Scrollbars::Overlay {
                state.show(cx);
            }
            cx.notify();
        });
    }
    if let Some(sink) = &ctx.sink {
        let event = restored_down(event, cx);
        sink.bar_pressed(ctx.id, &mouse_down_payload(&event), window, cx);
        if drag.is_some() {
            sink.bar_drag(ctx.id, true, window, cx);
        }
        if opens_context_menu(event.button, event.modifiers, control_click_is_right(cx)) {
            let payload = context_menu_payload(&event);
            sink.fire(ctx.id, "contextmenu", &payload, window, cx);
        }
    }
    cx.stop_propagation();
}

fn release<E: EventSink>(ctx: &Ctx<E>, event: &MouseUpEvent, window: &mut Window, cx: &mut App) {
    let (pressed, dragging) = {
        let state = ctx.state.read(cx);
        (state.pressed_on_bar, state.drag.is_some())
    };
    if pressed || dragging {
        ctx.state
            .update(cx, |state, _| state.pressed_on_bar = false);
        if let Some(sink) = &ctx.sink {
            let event = restored_up(event, cx);
            sink.bar_released(ctx.id, &mouse_up_payload(&event), window, cx);
        }
        if dragging && event.button == MouseButton::Left {
            ctx.end_drag(window, cx);
        }
        cx.stop_propagation();
    } else if ctx.sink.is_some() && ctx.over_bar(window) {
        mark_target(cx, ctx.id);
    }
}

fn movement<E: EventSink>(ctx: &Ctx<E>, event: &MouseMoveEvent, window: &mut Window, cx: &mut App) {
    if event.pressed_button.is_none() {
        ctx.state
            .update(cx, |state, _| state.pressed_on_bar = false);
    }
    ctx.drag_move(event, window, cx);
    if let Some(sink) = &ctx.sink
        && sync_over_bar(ctx, Some(event.position), window, cx)
    {
        move_over(Some(sink), ctx.id, window, cx);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use gpui::{MouseButton, TestAppContext, point, px};

    use super::*;
    use crate::Scrollbars;
    use crate::element::scrollbar::HIDE_DELAY;
    use crate::element::scrollbar::view::ScrollbarState;
    use crate::tests::{BarScene, Probe, capture_entities, nested_bar_tree, probe_of};

    /// The scroller (node 1) of a classic bar tree: both axes scroll, so each bar reserves its
    /// strip. The thumbs lie at `THUMB`, the track beyond them at `TRACK`, the corner at
    /// `CORNER` and the content at `CONTENT`.
    const BOTH: [(&str, &str); 1] = [("overflow", "scroll")];
    /// The scroller scrolls on y alone.
    const Y_SCROLL: [(&str, &str); 2] = [("overflow_x", "hidden"), ("overflow_y", "scroll")];
    const Y_AUTO: [(&str, &str); 2] = [("overflow_x", "hidden"), ("overflow_y", "auto")];
    const THUMB: (f32, f32) = (92., 10.);
    const TRACK: (f32, f32) = (92., 50.);
    const CORNER: (f32, f32) = (92., 92.);
    const CONTENT: (f32, f32) = (50., 50.);
    /// On the strip of an overlay or an `auto` bar, over its thumb.
    const OVERLAY_THUMB: (f32, f32) = (95., 10.);
    const LEFT: MouseButton = MouseButton::Left;

    fn classic(cx: &mut TestAppContext, props: &[(&str, &str)]) -> BarScene {
        BarScene::new(cx, Scrollbars::Classic, nested_bar_tree(props))
    }

    fn overlay(cx: &mut TestAppContext, props: &[(&str, &str)]) -> BarScene {
        BarScene::new(cx, Scrollbars::Overlay, nested_bar_tree(props))
    }

    fn names(calls: &[(String, NodeId)]) -> Vec<&str> {
        calls.iter().map(|(name, _)| name.as_str()).collect()
    }

    fn bars(scene: &BarScene) -> Vec<(String, NodeId)> {
        scene.probe.bars.borrow().clone()
    }

    #[gpui::test]
    fn a_press_on_a_classic_track_goes_to_the_container_only(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(TRACK, LEFT);
        assert_eq!(bars(&scene), [("bar_pressed".to_owned(), 1)]);
        assert!(!names(&scene.events()).contains(&"mousedown"));
    }

    #[gpui::test]
    fn a_press_on_the_content_beside_a_bar_reports_a_mousedown(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(CONTENT, LEFT);
        assert_eq!(scene.events(), [("mousedown".to_owned(), 2)]);
        assert!(bars(&scene).is_empty());
    }

    #[gpui::test]
    fn a_press_on_a_track_is_claimed_when_the_sink_tracks_no_pointer(cx: &mut TestAppContext) {
        let probe = Probe {
            tracks: false,
            mode: Scrollbars::Classic,
            ..probe_of(nested_bar_tree(&BOTH))
        };
        let mut scene = BarScene::with_probe(cx, probe);
        scene.press(TRACK, LEFT);
        assert!(bars(&scene).is_empty());
        assert!(scene.events().is_empty());
    }

    #[gpui::test]
    fn a_press_on_a_content_reports_a_mousedown_when_the_sink_tracks_no_pointer(
        cx: &mut TestAppContext,
    ) {
        let probe = Probe {
            tracks: false,
            mode: Scrollbars::Classic,
            ..probe_of(nested_bar_tree(&BOTH))
        };
        let mut scene = BarScene::with_probe(cx, probe);
        scene.press(CONTENT, LEFT);
        assert_eq!(names(&scene.events()), ["mousedown"]);
    }

    #[gpui::test]
    fn a_right_press_on_the_thumb_fires_a_contextmenu_at_the_container(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(THUMB, MouseButton::Right);
        assert_eq!(bars(&scene), [("bar_pressed".to_owned(), 1)]);
        assert_eq!(scene.events(), [("fire contextmenu".to_owned(), 1)]);
    }

    #[gpui::test]
    fn a_left_press_on_the_thumb_fires_no_contextmenu(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(THUMB, LEFT);
        assert!(!names(&scene.events()).contains(&"fire contextmenu"));
    }

    #[gpui::test]
    fn a_press_on_the_corner_reaches_the_content(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(CORNER, LEFT);
        assert_eq!(scene.events(), [("mousedown".to_owned(), 2)]);
        assert!(bars(&scene).is_empty());
    }

    #[gpui::test]
    fn a_release_over_a_strip_after_a_press_in_the_content_targets_the_container(
        cx: &mut TestAppContext,
    ) {
        let mut scene = classic(cx, &BOTH);
        scene.press(CONTENT, LEFT);
        scene.release(TRACK, LEFT);
        let events = scene.events();
        assert_eq!(
            events[1..],
            [("mouseup".to_owned(), 1), ("release".to_owned(), 1)]
        );
    }

    #[gpui::test]
    fn a_release_after_a_press_on_a_bar_goes_to_the_container_only(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(TRACK, LEFT);
        scene.release(TRACK, LEFT);
        assert_eq!(
            bars(&scene),
            [
                ("bar_pressed".to_owned(), 1),
                ("bar_released".to_owned(), 1)
            ]
        );
        assert!(scene.events().is_empty());
    }

    #[gpui::test]
    fn a_move_over_a_classic_track_targets_the_container(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.move_to(TRACK);
        assert_eq!(scene.events(), [("mousemove".to_owned(), 1)]);
    }

    #[gpui::test]
    fn a_press_at_the_place_of_a_hidden_overlay_thumb_reaches_the_content(cx: &mut TestAppContext) {
        let mut scene = overlay(cx, &Y_SCROLL);
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        scene.frame();
        scene.press(OVERLAY_THUMB, LEFT);
        assert_eq!(scene.events(), [("mousedown".to_owned(), 2)]);
    }

    #[gpui::test]
    fn a_press_on_the_strip_beside_a_shown_overlay_thumb_is_claimed(cx: &mut TestAppContext) {
        let mut scene = overlay(cx, &Y_SCROLL);
        scene.press((88., 10.), LEFT);
        assert_eq!(bars(&scene)[0], ("bar_pressed".to_owned(), 1));
    }

    #[gpui::test]
    fn a_press_on_the_thumb_of_a_classic_auto_bar_is_claimed_at_load(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &Y_AUTO);
        scene.press(OVERLAY_THUMB, LEFT);
        assert_eq!(bars(&scene)[0], ("bar_pressed".to_owned(), 1));
        assert!(scene.events().is_empty());
    }

    #[gpui::test]
    fn a_press_on_a_classic_auto_track_beyond_the_thumb_reaches_the_content(
        cx: &mut TestAppContext,
    ) {
        let mut scene = classic(cx, &Y_AUTO);
        scene.press((95., 60.), LEFT);
        assert_eq!(scene.events(), [("mousedown".to_owned(), 2)]);
        assert!(bars(&scene).is_empty());
    }

    #[gpui::test]
    fn a_wheel_over_a_classic_track_scrolls_the_container(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.wheel(TRACK, -40.);
        assert_eq!(scene.handle(1).offset(), point(px(0.), px(-40.)));
    }

    #[gpui::test]
    fn the_frame_a_wheel_step_shows_the_overlay_thumb_in_takes_its_press(cx: &mut TestAppContext) {
        let mut scene = overlay(cx, &Y_SCROLL);
        scene.cx.executor().advance_clock(HIDE_DELAY);
        scene.cx.run_until_parked();
        scene.frame();
        scene.wheel_undrawn(CONTENT, -40.);
        scene.press((95., 20.), LEFT);
        assert_eq!(bars(&scene)[0], ("bar_pressed".to_owned(), 1));
    }

    #[gpui::test]
    fn a_release_in_the_content_after_a_lost_release_reaches_the_page(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(THUMB, LEFT);
        scene.press(CONTENT, LEFT);
        scene.release(CONTENT, LEFT);
        assert_eq!(
            scene.events(),
            [
                ("mousedown".to_owned(), 2),
                ("mouseup".to_owned(), 2),
                ("release".to_owned(), 2)
            ]
        );
    }

    #[gpui::test]
    fn a_press_in_the_content_ends_a_drag_whose_release_was_lost(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(THUMB, LEFT);
        scene.press(CONTENT, LEFT);
        assert_eq!(bars(&scene).last(), Some(&("bar_drag_end".to_owned(), 1)));
    }

    #[gpui::test]
    fn a_move_with_no_button_held_ends_the_wait_for_a_lost_release(cx: &mut TestAppContext) {
        let mut scene = classic(cx, &BOTH);
        scene.press(TRACK, LEFT);
        scene.move_to(CONTENT);
        scene.release(CONTENT, LEFT);
        assert!(names(&scene.events()).contains(&"mouseup"));
    }

    #[gpui::test]
    fn an_inert_container_takes_no_press_on_its_bar(cx: &mut TestAppContext) {
        let states = capture_entities::<ScrollbarState>(cx);
        let mut tree = nested_bar_tree(&BOTH);
        tree.set_attribute(1, "inert", true).unwrap();
        let mut scene = BarScene::with_probe(
            cx,
            Probe {
                mode: Scrollbars::Classic,
                ..probe_of(tree)
            },
        );
        scene.press(TRACK, LEFT);
        assert!(bars(&scene).is_empty());
        assert_eq!(scene.events(), [("mousedown".to_owned(), 0)]);
        let state = states.borrow()[0].clone();
        assert!(!scene.cx.update(|_, cx| state.read(cx).pressed_on_bar));
    }
}
