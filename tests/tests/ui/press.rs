// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A press and a release on two nodes, read through a real `.vue` app.

use super::common::{center, click, height, inset, load, near, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};

#[gpui::test]
fn a_release_on_a_sibling_clicks_their_common_parent(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let none = Modifiers::none();
    h.keystrokes("f9");
    let a = center(&mut h, "a");
    let b = center(&mut h, "b");
    h.mouse_down(a, MouseButton::Left, none);
    h.move_to(b, Some(MouseButton::Left), none);

    h.mouse_up(b, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerdown@a mousedown@a pointerup@b mouseup@b click@row"
    );
}

#[gpui::test]
fn a_release_on_the_parent_of_the_pressed_node_clicks_the_parent(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let none = Modifiers::none();
    h.keystrokes("f9");
    let a1 = center(&mut h, "a1");
    let up = inset(&mut h, "a", 10.0, 30.0);
    h.mouse_down(a1, MouseButton::Left, none);
    h.move_to(up, Some(MouseButton::Left), none);

    h.mouse_up(up, MouseButton::Left, none);

    assert_eq!(
        text_of(&mut h, "log"),
        "pointerdown@a1 mousedown@a1 pointerup@a mouseup@a click@a"
    );
}

#[gpui::test]
fn a_pressed_node_keeps_active_while_the_pointer_moves_off(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let none = Modifiers::none();
    let a = center(&mut h, "a");
    let b = center(&mut h, "b");
    h.mouse_down(a, MouseButton::Left, none);

    h.move_to(b, Some(MouseButton::Left), none);

    near(height(&mut h, "a"), 60.0);
}

#[gpui::test]
fn the_last_release_clears_active_off_the_pressed_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let none = Modifiers::none();
    let a = center(&mut h, "a");
    let b = center(&mut h, "b");
    h.mouse_down(a, MouseButton::Left, none);
    h.move_to(b, Some(MouseButton::Left), none);

    h.mouse_up(b, MouseButton::Left, none);

    near(height(&mut h, "a"), 40.0);
}

#[gpui::test]
fn a_move_with_the_left_button_held_reports_buttons_1(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let none = Modifiers::none();
    let a = center(&mut h, "a");
    let b = center(&mut h, "b");
    h.mouse_down(a, MouseButton::Left, none);

    h.move_to(b, Some(MouseButton::Left), none);

    assert_eq!(text_of(&mut h, "buttons"), "1");
}

#[gpui::test]
fn a_space_release_on_a_button_the_mouse_holds_clicks(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let btn = center(&mut h, "btn");
    click(&mut h, "btn");
    h.keystrokes("f9");
    h.mouse_down(btn, MouseButton::Left, Modifiers::none());
    h.keystrokes("space");

    h.key_up("space");

    assert_eq!(
        text_of(&mut h, "seq"),
        "pointerdown@btn mousedown@btn keydown@btn keyup@btn click@btn"
    );
}

#[gpui::test]
fn the_mouse_release_after_a_space_click_clicks_again(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let btn = center(&mut h, "btn");
    click(&mut h, "btn");
    h.mouse_down(btn, MouseButton::Left, Modifiers::none());
    h.keystrokes("space");
    h.key_up("space");
    h.keystrokes("f9");
    h.move_to(btn, Some(MouseButton::Left), Modifiers::none());

    h.mouse_up(btn, MouseButton::Left, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "seq"),
        "pointerup@btn mouseup@btn click@btn"
    );
}

#[gpui::test]
fn a_pressed_button_takes_the_focus_right_after_mousedown(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let btn = center(&mut h, "btn");
    h.keystrokes("f9");

    h.mouse_down(btn, MouseButton::Left, Modifiers::none());
    h.mouse_up(btn, MouseButton::Left, Modifiers::none());

    assert_eq!(
        text_of(&mut h, "seq"),
        "pointerdown@btn mousedown@btn focus@btn focusin@btn pointerup@btn mouseup@btn click@btn"
    );
}

#[gpui::test]
fn a_release_with_no_press_fires_pointerup_and_mouseup_alone(cx: &mut TestAppContext) {
    let mut h = load(cx, "press-two");
    let a = center(&mut h, "a");
    h.keystrokes("f9");
    h.move_to(a, None, Modifiers::none());

    h.mouse_up(a, MouseButton::Left, Modifiers::none());

    assert_eq!(text_of(&mut h, "log"), "pointerup@a mouseup@a");
}
