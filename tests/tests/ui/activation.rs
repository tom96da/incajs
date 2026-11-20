// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Window activation, read through a real `.vue` app.

use super::common::{center, inset, load, near, nudge, text_of, width};
use gpui::{Modifiers, MouseButton, TestAppContext};
use inca_host::harness::Harness;

/// Loads the fixture, activates the window and clicks `#field`.
fn focused_field(cx: &mut TestAppContext) -> Harness {
    let mut h = load(cx, "activation");
    h.activate_window();
    let field = center(&mut h, "field");
    h.mouse_down(field, MouseButton::Left, Modifiers::none());
    h.mouse_up(field, MouseButton::Left, Modifiers::none());
    h
}

/// Loads the fixture, activates the window and presses Left on `#btn`.
fn pressed_button(cx: &mut TestAppContext) -> Harness {
    let mut h = load(cx, "activation");
    h.activate_window();
    let btn = center(&mut h, "btn");
    h.mouse_down(btn, MouseButton::Left, Modifiers::none());
    h
}

#[gpui::test]
fn activating_fires_focus_at_window_before_the_focused_node(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.deactivate_window();

    h.activate_window();

    assert_eq!(
        text_of(&mut h, "since"),
        "focus@window focus@field focusin@field"
    );
}

#[gpui::test]
fn deactivating_fires_blur_at_the_focused_node_before_window(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.keystrokes("f9");

    h.deactivate_window();

    assert_eq!(
        text_of(&mut h, "log"),
        "blur@field focusout@field blur@window"
    );
}

#[gpui::test]
fn the_focus_events_of_an_activation_carry_no_related_target(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.keystrokes("f9");
    h.deactivate_window();

    h.activate_window();

    assert_eq!(text_of(&mut h, "related"), "focusout:null focusin:null");
}

#[gpui::test]
fn the_focused_node_stays_the_active_element_while_the_window_is_inactive(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);

    h.deactivate_window();

    assert_eq!(text_of(&mut h, "active"), "field");
}

#[gpui::test]
fn deactivating_takes_the_focus_rule_off_the_focused_node(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);

    h.deactivate_window();

    near(width(&mut h, "field"), 100.0);
}

#[gpui::test]
fn activating_puts_the_focus_rule_back(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.deactivate_window();

    h.activate_window();

    near(width(&mut h, "field"), 120.0);
}

#[gpui::test]
fn deactivating_after_an_edit_fires_change_before_blur(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.keystrokes("x");
    h.keystrokes("f9");

    h.deactivate_window();

    assert_eq!(
        text_of(&mut h, "log"),
        "change@field blur@field focusout@field blur@window"
    );
}

#[gpui::test]
fn a_second_deactivation_without_an_edit_fires_no_change(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.keystrokes("x");
    h.deactivate_window();
    h.activate_window();
    h.keystrokes("f9");

    h.deactivate_window();

    assert_eq!(
        text_of(&mut h, "log"),
        "blur@field focusout@field blur@window"
    );
}

#[gpui::test]
fn a_focus_requested_while_inactive_fires_its_events_at_activation(cx: &mut TestAppContext) {
    let mut h = load(cx, "activation");
    h.activate_window();
    h.keystrokes("f2");
    h.deactivate_window();

    h.activate_window();

    assert_eq!(
        text_of(&mut h, "since"),
        "focus@window focus@field focusin@field"
    );
}

#[gpui::test]
fn deactivating_during_a_press_drops_its_click(cx: &mut TestAppContext) {
    let mut h = pressed_button(cx);
    h.deactivate_window();
    h.activate_window();
    let btn = center(&mut h, "btn");

    h.mouse_up(btn, MouseButton::Left, Modifiers::none());

    assert_eq!(text_of(&mut h, "clicks"), "0");
}

#[gpui::test]
fn a_move_after_a_press_lost_to_a_deactivation_reports_no_button(cx: &mut TestAppContext) {
    let mut h = pressed_button(cx);
    h.deactivate_window();
    h.activate_window();
    let btn = center(&mut h, "btn");

    h.move_to(nudge(btn), None, Modifiers::none());

    assert_eq!(text_of(&mut h, "buttons"), "0");
}

#[gpui::test]
fn a_move_after_a_press_lost_to_a_deactivation_selects_no_text(cx: &mut TestAppContext) {
    let mut h = load(cx, "activation");
    h.activate_window();
    let press = inset(&mut h, "field", 98.0, 10.0);
    h.mouse_down(press, MouseButton::Left, Modifiers::none());
    h.deactivate_window();
    h.activate_window();
    let to = inset(&mut h, "field", 2.0, 10.0);
    h.move_to(to, None, Modifiers::none());

    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "value"), "xabcd");
}

#[gpui::test]
fn a_modifier_released_while_inactive_fires_keydown_when_pressed_again(cx: &mut TestAppContext) {
    let mut h = load(cx, "activation");
    h.activate_window();
    h.set_modifiers(Modifiers::shift());
    h.deactivate_window();
    h.activate_window();

    h.set_modifiers(Modifiers::shift());

    assert_eq!(text_of(&mut h, "keys"), "Shift Shift");
}

#[gpui::test]
fn deactivating_during_a_press_takes_active_off_the_pressed_node(cx: &mut TestAppContext) {
    let mut h = pressed_button(cx);

    h.deactivate_window();

    near(width(&mut h, "btn"), 80.0);
}

#[gpui::test]
fn a_space_held_across_a_deactivation_clicks_nothing_at_its_release(cx: &mut TestAppContext) {
    let mut h = load(cx, "activation");
    h.activate_window();
    let btn = center(&mut h, "btn");
    h.mouse_down(btn, MouseButton::Left, Modifiers::none());
    h.mouse_up(btn, MouseButton::Left, Modifiers::none());
    h.keystrokes("f9");
    h.keystrokes("space");
    h.deactivate_window();
    h.activate_window();

    h.key_up("space");

    assert_eq!(text_of(&mut h, "clicks"), "0");
}

#[gpui::test]
fn focusing_then_blurring_while_inactive_fires_nothing_at_activation(cx: &mut TestAppContext) {
    let mut h = load(cx, "activation");
    h.activate_window();
    h.keystrokes("f4");
    h.deactivate_window();

    h.activate_window();

    assert_eq!(text_of(&mut h, "since"), "focus@window");
}

#[gpui::test]
fn a_focused_node_disabled_while_inactive_fires_no_second_blur(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.keystrokes("f6");
    h.deactivate_window();

    h.activate_window();

    assert_eq!(text_of(&mut h, "since"), "focus@window");
}

#[gpui::test]
fn a_focused_node_removed_while_inactive_fires_nothing_at_activation(cx: &mut TestAppContext) {
    let mut h = focused_field(cx);
    h.keystrokes("f7");
    h.deactivate_window();

    h.activate_window();

    assert_eq!(text_of(&mut h, "since"), "focus@window");
}

#[gpui::test]
fn a_canceled_press_lost_to_a_deactivation_suppresses_no_mousemove(cx: &mut TestAppContext) {
    let mut h = load(cx, "activation");
    h.activate_window();
    let pad = center(&mut h, "pad");
    h.mouse_down(pad, MouseButton::Left, Modifiers::none());
    h.deactivate_window();
    h.activate_window();
    h.keystrokes("f9");

    h.move_to(nudge(pad), None, Modifiers::none());

    assert_eq!(text_of(&mut h, "mm"), "1");
}
