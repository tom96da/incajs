// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer events of a disabled `button` and `input` in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{appended, click, click_with, hover, load, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};

const OFF_PTR: &str = concat!(
    "pointerdown@off(off) pointerdown@wrap(off) ",
    "pointerup@off(off) pointerup@wrap(off)"
);

#[gpui::test]
fn a_left_click_on_a_disabled_button_fires_only_pointer_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "disabled");

    click(&mut h, "off");

    assert_eq!(text_of(&mut h, "mouse"), "");
    assert_eq!(text_of(&mut h, "ptr"), OFF_PTR);
}

#[gpui::test]
fn a_listener_below_a_disabled_button_hears_the_mouse_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "disabled");

    click(&mut h, "kid");

    assert_eq!(
        text_of(&mut h, "mouse"),
        "mousedown@kid(kid) mouseup@kid(kid) click@kid(kid)"
    );
}

#[gpui::test]
fn a_double_click_on_a_disabled_button_fires_no_dblclick(cx: &mut TestAppContext) {
    let mut h = load(cx, "disabled");
    click_with(&mut h, "off", MouseButton::Left, Modifiers::none(), 1);
    click_with(&mut h, "off", MouseButton::Left, Modifiers::none(), 2);

    assert_eq!(text_of(&mut h, "mouse"), "");
    assert_eq!(text_of(&mut h, "ptr"), format!("{OFF_PTR} {OFF_PTR}"));
}

#[gpui::test]
fn a_right_click_on_a_disabled_button_fires_the_menu_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "disabled");
    click_with(&mut h, "off", MouseButton::Right, Modifiers::none(), 1);

    assert_eq!(
        text_of(&mut h, "menu"),
        "contextmenu@off(off) contextmenu@wrap(off) auxclick@off(off) auxclick@wrap(off)"
    );
}

#[gpui::test]
fn a_middle_click_on_a_disabled_button_fires_auxclick(cx: &mut TestAppContext) {
    let mut h = load(cx, "disabled");
    click_with(&mut h, "off", MouseButton::Middle, Modifiers::none(), 1);

    assert_eq!(
        text_of(&mut h, "menu"),
        "auxclick@off(off) auxclick@wrap(off)"
    );
}

#[gpui::test]
fn a_disabled_button_keeps_hover_events_under_the_pointer(cx: &mut TestAppContext) {
    let mut h = load(cx, "disabled");
    h.unhover();
    let before = text_of(&mut h, "hover");

    hover(&mut h, "off");

    assert_eq!(
        appended(&before, &text_of(&mut h, "hover")),
        concat!(
            "mouseover@off(off) mouseover@wrap(off) mouseenter@wrap(wrap) ",
            "mouseenter@off(off) mousemove@off(off) mousemove@wrap(off)"
        )
    );
}

#[gpui::test]
fn a_click_on_a_disabled_input_fires_no_mouse_event(cx: &mut TestAppContext) {
    let mut h = load(cx, "disabled");

    click(&mut h, "off-input");

    assert_eq!(text_of(&mut h, "mouse"), "");
    assert_eq!(
        text_of(&mut h, "ptr"),
        concat!(
            "pointerdown@off-input(off-input) pointerdown@wrap(off-input) ",
            "pointerup@off-input(off-input) pointerup@wrap(off-input)"
        )
    );
}

#[gpui::test]
fn a_press_on_a_disabled_button_moves_focus_to_the_nearest_focusable_ancestor(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "disabled");
    click(&mut h, "other");

    click(&mut h, "off");
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "wrap");
}

#[gpui::test]
fn a_press_on_a_disabled_button_with_no_focusable_ancestor_blurs_the_focus(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "disabled");
    click(&mut h, "other");

    click(&mut h, "off-alone");
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "");
}
