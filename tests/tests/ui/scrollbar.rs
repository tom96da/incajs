// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The press and the drag on a scrollbar, read through a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{center, click, inset, load, near, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};
use inca_host::harness::{Harness, Scrollbars};
use inca_test_macros::gpui_cases;
use std::time::Duration;

fn thumb(h: &mut Harness) -> gpui::Point<gpui::Pixels> {
    inset(h, "box", 7.5, 10.0)
}

fn track(h: &mut Harness) -> gpui::Point<gpui::Pixels> {
    inset(h, "box", 7.5, 90.0)
}

/// Presses the thumb of `#box`, drags it over `#field` and releases it there.
fn drag_thumb_onto_field(h: &mut Harness) {
    let none = Modifiers::none();
    let grab = thumb(h);
    let middle = inset(h, "box", 7.5, 50.0);
    let field = center(h, "field");
    h.mouse_down(grab, MouseButton::Left, none);
    h.move_to(middle, Some(MouseButton::Left), none);
    h.move_to(field, Some(MouseButton::Left), none);
    h.mouse_up(field, MouseButton::Left, none);
}

/// Moves to `at` with no button, then presses F9 to mark where the observed part starts.
fn mark_at(h: &mut Harness, at: gpui::Point<gpui::Pixels>) {
    h.move_to(at, None, Modifiers::none());
    h.keystrokes("f9");
}

#[gpui::test]
fn a_press_on_the_track_targets_the_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let at = track(&mut h);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerdown@box mousedown@box pointerup@box mouseup@box"
    );
}

#[gpui::test]
fn a_press_on_the_track_keeps_the_focus_of_an_input(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    click(&mut h, "field");
    let at = track(&mut h);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(text_of(&mut h, "active"), "field");
}

#[gpui::test]
fn a_press_beside_the_strip_reaches_the_content(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let at = inset(&mut h, "box", 16.0, 50.0);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerdown@box mousedown@box pointerup@box mouseup@box click@box"
    );
}

#[gpui::test]
fn a_thumb_dragged_past_the_end_scrolls_to_the_end(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    drag_thumb_onto_field(&mut h);

    h.keystrokes("e");

    assert_eq!(text_of(&mut h, "at-end"), "true");
}

#[gpui::test]
fn a_thumb_drag_fires_scroll_at_the_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");

    drag_thumb_onto_field(&mut h);

    assert_eq!(text_of(&mut h, "scrolled"), "yes");
}

#[gpui::test]
fn a_thumb_drag_fires_no_pointer_or_mouse_move(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let grab = thumb(&mut h);
    let middle = inset(&mut h, "box", 7.5, 50.0);
    let field = center(&mut h, "field");
    h.mouse_down(grab, MouseButton::Left, none);
    h.keystrokes("f9");

    h.move_to(middle, Some(MouseButton::Left), none);
    h.move_to(field, Some(MouseButton::Left), none);

    assert_eq!(text_of(&mut h, "moves"), "");
}

#[gpui::test]
fn a_right_press_on_the_thumb_fires_contextmenu_at_the_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let at = thumb(&mut h);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Right, none);
    h.mouse_up(at, MouseButton::Right, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerdown@box mousedown@box contextmenu@box pointerup@box mouseup@box"
    );
}

#[gpui::test]
fn a_thumb_dragged_by_half_its_travel_scrolls_half_the_range(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let grab = thumb(&mut h);
    let half = inset(&mut h, "box", 7.5, 48.5);
    h.mouse_down(grab, MouseButton::Left, none);

    h.move_to(half, Some(MouseButton::Left), none);
    h.keystrokes("e");

    near(text_of(&mut h, "scroll-top").parse().unwrap(), 450.0);
}

#[gpui::test]
fn a_thumb_dragged_above_the_track_scrolls_to_the_start(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let grab = thumb(&mut h);
    let half = inset(&mut h, "box", 7.5, 48.5);
    let above = inset(&mut h, "box", 7.5, -10.0);
    h.mouse_down(grab, MouseButton::Left, none);
    h.move_to(half, Some(MouseButton::Left), none);
    h.keystrokes("e");

    h.move_to(above, Some(MouseButton::Left), none);
    h.keystrokes("e");

    assert_eq!(text_of(&mut h, "scroll-top"), "0");
}

#[gpui::test]
fn a_horizontal_thumb_dragged_past_the_end_scrolls_to_the_end(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let grab = inset(&mut h, "hbox", 90.0, 92.5);
    let past = inset(&mut h, "hbox", -50.0, 92.5);
    h.mouse_down(grab, MouseButton::Left, none);

    h.move_to(past, Some(MouseButton::Left), none);
    h.keystrokes("e");

    assert_eq!(text_of(&mut h, "h-at-end"), "true");
}

#[gpui::test]
fn a_press_on_the_corner_of_two_bars_reaches_the_content(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let at = inset(&mut h, "both", 7.5, 92.5);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerdown@huge mousedown@huge pointerup@huge mouseup@huge click@huge"
    );
}

#[gpui::test]
fn a_release_over_a_strip_after_a_press_in_the_content_targets_the_container(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let content = inset(&mut h, "both", 50.0, 50.0);
    let strip = inset(&mut h, "both", 7.5, 50.0);
    mark_at(&mut h, content);
    h.mouse_down(content, MouseButton::Left, none);
    h.move_to(strip, Some(MouseButton::Left), none);

    h.mouse_up(strip, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "ups"),
        "pointerup@both mouseup@both click@both"
    );
}

#[gpui::test]
fn a_canceled_pointerdown_on_a_bar_suppresses_the_mouse_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    h.keystrokes("p");
    let at = track(&mut h);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(text_of(&mut h, "log"), "pointerdown@box pointerup@box");
}

#[gpui::test]
fn a_second_button_on_a_bar_fires_pointermove(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let at = track(&mut h);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_down(at, MouseButton::Right, none);
    h.mouse_up(at, MouseButton::Right, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(text_of(&mut h, "moves"), "pointermove@box pointermove@box");
}

#[gpui::test]
fn a_thumb_drag_release_fires_boundary_events_at_the_node_under_the_pointer(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let grab = thumb(&mut h);
    let middle = inset(&mut h, "box", 7.5, 50.0);
    let field = center(&mut h, "field");
    h.mouse_down(grab, MouseButton::Left, none);
    h.move_to(middle, Some(MouseButton::Left), none);
    h.move_to(field, Some(MouseButton::Left), none);
    h.keystrokes("f9");

    h.mouse_up(field, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "log"),
        concat!(
            "pointerup@box mouseup@box pointerout@box pointerleave@box pointerover@field ",
            "pointerenter@field mouseout@box mouseleave@box mouseover@field mouseenter@field"
        )
    );
}

#[gpui::test]
fn a_capture_set_in_a_bar_press_ends_at_the_bar_release(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    h.keystrokes("k");
    let at = track(&mut h);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "caps"),
        "gotpointercapture@box lostpointercapture@box"
    );
}

#[gpui::test]
fn a_bar_press_during_a_capture_goes_to_the_capturing_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    let none = Modifiers::none();
    let above = center(&mut h, "above");
    let nudged = above + gpui::point(gpui::px(2.0), gpui::px(2.0));
    let at = track(&mut h);
    h.mouse_down(above, MouseButton::Left, none);
    h.move_to(nudged, Some(MouseButton::Left), none);
    h.move_to(at, Some(MouseButton::Left), none);
    h.keystrokes("f9");

    h.mouse_down(at, MouseButton::Right, none);
    h.mouse_up(at, MouseButton::Right, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "mousedown@above contextmenu@above mouseup@above"
    );
}

#[gpui_cases]
#[gpui::test]
#[case::classic(Scrollbars::Classic, "85:100")]
#[case::overlay(Scrollbars::Overlay, "100:100")]
fn a_scroll_box_reports_the_client_width_its_scrollbars_leave(
    cx: &mut TestAppContext,
    #[case] mode: Scrollbars,
    #[case] expected: &str,
) {
    let mut h = load(cx, "scrollbar");
    h.scrollbars(mode);

    click(&mut h, "measure");

    assert_eq!(text_of(&mut h, "sizes"), expected);
}

#[gpui::test]
fn an_overlay_thumb_dragged_past_the_end_scrolls_to_the_end(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    h.scrollbars(Scrollbars::Overlay);
    drag_thumb_onto_field(&mut h);

    h.keystrokes("e");

    assert_eq!(text_of(&mut h, "at-end"), "true");
}

#[gpui::test]
fn a_horizontal_overlay_thumb_dragged_past_the_end_scrolls_to_the_end(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    h.scrollbars(Scrollbars::Overlay);
    let none = Modifiers::none();
    let grab = inset(&mut h, "hbox", 90.0, 92.5);
    let past = inset(&mut h, "hbox", -50.0, 92.5);
    h.mouse_down(grab, MouseButton::Left, none);

    h.move_to(past, Some(MouseButton::Left), none);
    h.keystrokes("e");

    assert_eq!(text_of(&mut h, "h-at-end"), "true");
}

const OBOX_MENU: &str =
    "pointerdown@obox mousedown@obox contextmenu@obox pointerup@obox mouseup@obox";

/// Right-presses and releases the thumb spot of `#obox` after `wait` on the fake clock.
fn right_press_on_obox_after(cx: &mut TestAppContext, wait: Duration) -> String {
    let mut h = load(cx, "scrollbar");
    h.scrollbars(Scrollbars::Overlay);
    h.advance_clock(wait);
    let at = inset(&mut h, "obox", 7.5, 10.0);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Right, Modifiers::none());
    h.mouse_up(at, MouseButton::Right, Modifiers::none());

    text_of(&mut h, "log")
}

#[gpui::test]
fn a_right_press_on_a_shown_overlay_thumb_fires_contextmenu_at_the_container(
    cx: &mut TestAppContext,
) {
    assert_eq!(right_press_on_obox_after(cx, Duration::ZERO), OBOX_MENU);
}

#[gpui::test]
fn a_press_where_a_hidden_overlay_thumb_was_reaches_the_content(cx: &mut TestAppContext) {
    let mut h = load(cx, "scrollbar");
    h.scrollbars(Scrollbars::Overlay);
    h.advance_clock(Duration::from_secs(1));
    let none = Modifiers::none();
    let at = inset(&mut h, "obox", 7.5, 10.0);
    mark_at(&mut h, at);

    h.mouse_down(at, MouseButton::Left, none);
    h.mouse_up(at, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerdown@otall mousedown@otall pointerup@otall mouseup@otall click@otall"
    );
}

#[gpui::test]
fn an_overlay_thumb_still_takes_a_press_just_before_the_hide_delay(cx: &mut TestAppContext) {
    assert_eq!(
        right_press_on_obox_after(cx, Duration::from_millis(999)),
        OBOX_MENU
    );
}
