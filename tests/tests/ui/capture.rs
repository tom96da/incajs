// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pointer capture, read through a real `.vue` app.

use super::common::{by_id, center, height, load, near, nudge, text_of};
use gpui::{Modifiers, MouseButton, Pixels, Point, TestAppContext};
use inca_host::harness::Harness;

const LEFT: Option<MouseButton> = Some(MouseButton::Left);

/// Presses Left on `#a` and moves 2 px held, so `#a` holds the capture.
/// Returns the point of the pointer.
fn capture_a(h: &mut Harness) -> Point<Pixels> {
    let a = center(h, "a");
    h.mouse_down(a, MouseButton::Left, Modifiers::none());
    h.move_to(nudge(a), LEFT, Modifiers::none());
    nudge(a)
}

/// CP-2: the capture of `#a` holds while the pointer moves over `#b`.
/// Returns the point of the pointer.
fn move_over_b(h: &mut Harness) -> Point<Pixels> {
    capture_a(h);
    h.keystrokes("f9");
    let b = center(h, "b");
    h.move_to(b, LEFT, Modifiers::none());
    h.move_to(nudge(b), LEFT, Modifiers::none());
    nudge(b)
}

#[gpui::test]
fn a_capture_set_in_pointerdown_starts_at_the_next_move(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let a = center(&mut h, "a");
    h.mouse_down(a, MouseButton::Left, Modifiers::none());
    h.keystrokes("f9");

    h.move_to(nudge(a), LEFT, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "log"),
        "gotpointercapture@a pointermove@a mousemove@a"
    );
}

#[gpui::test]
fn a_captured_move_over_another_node_stays_at_the_capturing_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");

    move_over_b(&mut h);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointermove@a mousemove@a pointermove@a mousemove@a"
    );
}

#[gpui::test]
fn the_last_release_of_a_capture_fires_lostpointercapture_before_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let p = move_over_b(&mut h);
    h.keystrokes("f9");

    h.mouse_up(p, MouseButton::Left, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerup@a mouseup@a lostpointercapture@a click@a pointerout@a pointerleave@a \
         pointerover@b pointerenter@b mouseout@a mouseleave@a mouseover@b mouseenter@b"
    );
}

#[gpui::test]
fn a_capture_of_another_node_sends_the_click_there(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let c = center(&mut h, "c");
    h.mouse_down(c, MouseButton::Left, Modifiers::none());
    h.move_to(nudge(c), LEFT, Modifiers::none());
    h.keystrokes("f9");

    h.mouse_up(nudge(c), MouseButton::Left, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerup@b mouseup@b lostpointercapture@b click@b pointerout@b pointerleave@b \
         pointerover@c pointerenter@c mouseout@b mouseleave@b mouseover@c mouseenter@c"
    );
}

#[gpui::test]
fn a_released_capture_stays_until_the_next_pointer_event(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    move_over_b(&mut h);
    h.keystrokes("f9");

    h.keystrokes("r");

    assert_eq!(text_of(&mut h, "caps"), "has:false");
}

#[gpui::test]
fn a_released_capture_ends_at_the_next_move_before_its_boundary_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let p = move_over_b(&mut h);
    h.keystrokes("f9");
    h.keystrokes("r");

    h.move_to(nudge(p), LEFT, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "log"),
        "lostpointercapture@a pointerout@a pointerleave@a pointerover@b pointerenter@b \
         mouseout@a mouseleave@a mouseover@b mouseenter@b pointermove@b mousemove@b"
    );
}

#[gpui::test]
fn removing_the_capturing_node_reports_the_loss_at_document(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    capture_a(&mut h);
    h.keystrokes("x");

    let b = center(&mut h, "b");

    h.move_to(b, LEFT, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "caps"),
        "gotpointercapture@a lostpointercapture@document"
    );
}

#[gpui::test]
fn set_pointer_capture_with_no_button_held_does_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let b = center(&mut h, "b");
    h.move_to(b, None, Modifiers::none());
    h.keystrokes("c");

    h.move_to(nudge(b), None, Modifiers::none());

    assert_eq!(text_of(&mut h, "caps"), "has:false");
}

#[gpui::test]
fn releasing_one_of_two_buttons_keeps_the_capture(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let none = Modifiers::none();
    let a = center(&mut h, "a");
    h.mouse_down(a, MouseButton::Left, none);
    h.mouse_down(a, MouseButton::Right, none);
    h.move_to(nudge(a), LEFT, none);
    h.mouse_up(nudge(a), MouseButton::Right, none);

    h.move_to(nudge(nudge(a)), LEFT, none);

    assert_eq!(text_of(&mut h, "caps"), "gotpointercapture@a");
}

#[gpui::test]
fn the_hover_rule_stays_on_the_node_under_the_pointer_during_a_capture(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    capture_a(&mut h);
    let b = center(&mut h, "b");

    h.move_to(b, LEFT, Modifiers::none());

    near(height(&mut h, "b"), 50.0);
}

#[gpui::test]
fn a_capture_moved_to_another_node_fires_lost_before_got_at_the_next_move(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let p = capture_a(&mut h);
    h.keystrokes("f9");
    h.keystrokes("c");

    h.move_to(nudge(p), LEFT, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "caps"),
        "has:true lostpointercapture@a gotpointercapture@b"
    );
}

#[gpui::test]
fn a_capture_with_an_unknown_pointer_id_throws_not_found_error(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");

    h.keystrokes("i");

    assert_eq!(text_of(&mut h, "caps"), "NotFoundError");
}

#[gpui::test]
fn a_capture_on_a_removed_node_throws_invalid_state_error(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    h.keystrokes("x");

    h.keystrokes("d");

    assert_eq!(text_of(&mut h, "caps"), "InvalidStateError");
}

#[gpui::test]
fn capturing_the_node_that_holds_the_capture_fires_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let p = capture_a(&mut h);
    h.keystrokes("f9");
    h.keystrokes("s");

    h.move_to(nudge(p), LEFT, Modifiers::none());

    assert_eq!(text_of(&mut h, "caps"), "has:true");
}

#[gpui::test]
fn the_contextmenu_of_a_press_during_a_capture_goes_to_the_capture_target(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    capture_a(&mut h);
    let b = center(&mut h, "b");
    h.move_to(b, LEFT, Modifiers::none());

    h.mouse_down(b, MouseButton::Right, Modifiers::none());

    assert_eq!(text_of(&mut h, "menu"), "contextmenu@a");
}

#[gpui::test]
fn a_captured_right_release_sends_auxclick_to_the_capture_target(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let none = Modifiers::none();
    let c = center(&mut h, "c");
    h.mouse_down(c, MouseButton::Right, none);
    h.move_to(nudge(c), Some(MouseButton::Right), none);

    h.mouse_up(nudge(c), MouseButton::Right, none);

    assert_eq!(text_of(&mut h, "aux"), "auxclick@b");
}

#[gpui::test]
fn a_capture_follows_its_node_when_vue_moves_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let p = capture_a(&mut h);
    h.keystrokes("m");

    h.move_to(nudge(p), LEFT, Modifiers::none());

    assert_eq!(text_of(&mut h, "caps"), "gotpointercapture@a");
}

#[gpui::test]
fn gotpointercapture_copies_the_move_that_caused_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let a = center(&mut h, "a");
    h.mouse_down(a, MouseButton::Left, Modifiers::none());

    h.move_to(nudge(a), LEFT, Modifiers::none());

    assert_eq!(text_of(&mut h, "got"), "-1,1,0.5");
}

#[gpui::test]
fn gotpointercapture_copies_the_release_that_caused_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let a = center(&mut h, "a");
    h.mouse_down(a, MouseButton::Left, Modifiers::none());

    h.mouse_up(a, MouseButton::Left, Modifiers::none());

    assert_eq!(text_of(&mut h, "got"), "0,0,0");
}

#[gpui::test]
fn a_removed_node_with_a_pending_capture_has_no_capture(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let a = center(&mut h, "a");
    h.mouse_down(a, MouseButton::Left, Modifiers::none());
    h.keystrokes("x");

    h.keystrokes("h");

    assert_eq!(text_of(&mut h, "caps"), "has:false");
}

#[gpui::test]
fn a_right_press_sends_contextmenu_to_the_pressed_node_before_its_capture_starts(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "capture");
    let c = center(&mut h, "c");

    h.mouse_down(c, MouseButton::Right, Modifiers::none());

    assert_eq!(text_of(&mut h, "menu"), "contextmenu@c");
}

#[gpui::test]
fn a_captured_double_click_fires_dblclick_at_the_capture_target(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let c = by_id(&h.snapshot(), "c").id;

    h.click_with(c, MouseButton::Left, Modifiers::none(), 2);

    assert_eq!(text_of(&mut h, "dbl"), "dblclick@b");
}

#[gpui::test]
fn capture_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    let p = capture_a(&mut h);

    h.mouse_up(p, MouseButton::Left, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "cflags"),
        "gotpointercapture:true:false:true:false lostpointercapture:true:false:true:false"
    );
}

#[gpui::test]
fn deactivating_the_window_ends_the_capture(cx: &mut TestAppContext) {
    let mut h = load(cx, "capture");
    h.activate_window();
    capture_a(&mut h);

    h.deactivate_window();

    assert_eq!(
        text_of(&mut h, "caps"),
        "gotpointercapture@a lostpointercapture@a"
    );
}
