// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The events a press, a release and a click fire, in order, with their targets and fields.

use super::common::{center, click, click_with, height, inset, load, near, settled, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};
use inca_test_macros::gpui_cases;

/// The log of one left click on `box`.
const LEFT_CLICK: &str = concat!(
    "box:pointerdown:0:0:1 box:mousedown:1:0:1 box:pointerup:0:0:0 ",
    "box:mouseup:1:0:0 box:click:1:0:0"
);

/// What a left click with Control logs on the platform: macOS opens the
/// context menu on the press and completes no `click`.
const CTRL_CLICK_LOG: &str = if cfg!(target_os = "macos") {
    concat!(
        "box:pointerdown:0:0:1 box:mousedown:1:0:1 box:contextmenu:0:0:1 box:pointerup:0:0:0 ",
        "box:mouseup:1:0:0"
    )
} else {
    LEFT_CLICK
};

/// The `#mods` tokens of a left click with Control on the platform.
const CTRL_CLICK_MODS: &str = if cfg!(target_os = "macos") {
    concat!(
        "pointerdown:true:false:false:false mousedown:true:false:false:false ",
        "pointerup:true:false:false:false mouseup:true:false:false:false"
    )
} else {
    concat!(
        "pointerdown:true:false:false:false mousedown:true:false:false:false ",
        "pointerup:true:false:false:false mouseup:true:false:false:false ",
        "click:true:false:false:false"
    )
};

#[gpui::test]
fn a_left_click_fires_the_press_and_release_events_in_order(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click(&mut h, "box");

    assert_eq!(text_of(&mut h, "log"), LEFT_CLICK);
}

#[gpui::test]
fn a_second_click_of_a_double_click_ends_with_dblclick(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click_with(&mut h, "box", MouseButton::Left, Modifiers::none(), 1);
    click_with(&mut h, "box", MouseButton::Left, Modifiers::none(), 2);

    assert_eq!(
        text_of(&mut h, "log"),
        format!(
            "{LEFT_CLICK} {}",
            concat!(
                "box:pointerdown:0:0:1 box:mousedown:2:0:1 box:pointerup:0:0:0 ",
                "box:mouseup:2:0:0 box:click:2:0:0 box:dblclick:2:0:0"
            )
        )
    );
}

#[gpui::test]
fn a_right_click_fires_contextmenu_between_mousedown_and_mouseup(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click_with(&mut h, "box", MouseButton::Right, Modifiers::none(), 1);

    assert_eq!(
        text_of(&mut h, "log"),
        concat!(
            "box:pointerdown:0:2:2 box:mousedown:1:2:2 box:contextmenu:0:2:2 box:pointerup:0:2:0 ",
            "box:mouseup:1:2:0 box:auxclick:1:2:0"
        )
    );
}

#[gpui::test]
fn a_middle_click_ends_with_auxclick(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click_with(&mut h, "box", MouseButton::Middle, Modifiers::none(), 1);

    assert_eq!(
        text_of(&mut h, "log"),
        concat!(
            "box:pointerdown:0:1:4 box:mousedown:1:1:4 box:pointerup:0:1:0 box:mouseup:1:1:0 ",
            "box:auxclick:1:1:0"
        )
    );
}

#[gpui::test]
fn a_canceled_mousedown_still_clicks(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click(&mut h, "box2");

    assert_eq!(
        text_of(&mut h, "log"),
        concat!(
            "box2:pointerdown:0:0:1 box2:mousedown:1:0:1 box2:pointerup:0:0:0 box2:mouseup:1:0:0 ",
            "box2:click:1:0:0"
        )
    );
}

#[gpui::test]
fn a_canceled_mousedown_leaves_the_node_unfocused(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click(&mut h, "box2");
    settled(&mut h, 1);
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "");
    assert_eq!(text_of(&mut h, "focus"), "");
}

#[gpui::test]
fn a_canceled_pointerdown_skips_the_mouse_press_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click(&mut h, "box3");

    assert_eq!(
        text_of(&mut h, "log"),
        "box3:pointerdown:0:0:1 box3:pointerup:0:0:0 box3:click:1:0:0"
    );
}

#[gpui::test]
fn a_canceled_pointerdown_leaves_the_node_unfocused(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click(&mut h, "box3");
    settled(&mut h, 1);
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "");
}

#[gpui::test]
fn a_canceled_pointerdown_still_fires_contextmenu_on_a_right_press(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click_with(&mut h, "box3", MouseButton::Right, Modifiers::none(), 1);

    assert_eq!(
        text_of(&mut h, "log"),
        "box3:pointerdown:0:2:2 box3:contextmenu:0:2:2 box3:pointerup:0:2:0 box3:auxclick:1:2:0"
    );
}

#[gpui_cases]
#[gpui::test]
#[case::ctrl(Modifiers::control(), CTRL_CLICK_MODS)]
#[case::alt(
    Modifiers::alt(),
    concat!("pointerdown:false:true:false:false mousedown:false:true:false:false ", "pointerup:false:true:false:false mouseup:false:true:false:false ", "click:false:true:false:false")
)]
#[case::shift(
    Modifiers::shift(),
    concat!("pointerdown:false:false:true:false mousedown:false:false:true:false ", "pointerup:false:false:true:false mouseup:false:false:true:false ", "click:false:false:true:false")
)]
#[case::cmd(
    Modifiers::command(),
    concat!("pointerdown:false:false:false:true mousedown:false:false:false:true ", "pointerup:false:false:false:true mouseup:false:false:false:true ", "click:false:false:false:true")
)]
fn a_modified_click_carries_the_held_modifier_on_every_press_event(
    cx: &mut TestAppContext,
    #[case] modifiers: Modifiers,
    #[case] expected: &str,
) {
    let mut h = load(cx, "click");

    click_with(&mut h, "box", MouseButton::Left, modifiers, 1);

    assert_eq!(text_of(&mut h, "mods"), expected);
}

#[gpui::test]
fn a_control_click_fires_the_press_events_the_platform_gives(cx: &mut TestAppContext) {
    let mut h = load(cx, "click");

    click_with(&mut h, "box", MouseButton::Left, Modifiers::control(), 1);

    assert_eq!(text_of(&mut h, "log"), CTRL_CLICK_LOG);
}

#[gpui::test]
fn a_release_on_a_sibling_clicks_their_common_parent(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-two-nodes");
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
    let mut h = load(cx, "click-two-nodes");
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
    let mut h = load(cx, "click-two-nodes");
    let none = Modifiers::none();
    let a = center(&mut h, "a");
    let b = center(&mut h, "b");
    h.mouse_down(a, MouseButton::Left, none);

    h.move_to(b, Some(MouseButton::Left), none);

    near(height(&mut h, "a"), 60.0);
}

#[gpui::test]
fn the_last_release_clears_active_off_the_pressed_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-two-nodes");
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
    let mut h = load(cx, "click-two-nodes");
    let none = Modifiers::none();
    let a = center(&mut h, "a");
    let b = center(&mut h, "b");
    h.mouse_down(a, MouseButton::Left, none);

    h.move_to(b, Some(MouseButton::Left), none);

    assert_eq!(text_of(&mut h, "buttons"), "1");
}

#[gpui::test]
fn a_space_release_on_a_button_the_mouse_holds_clicks(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-two-nodes");
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
    let mut h = load(cx, "click-two-nodes");
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
    let mut h = load(cx, "click-two-nodes");
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
    let mut h = load(cx, "click-two-nodes");
    let a = center(&mut h, "a");
    h.keystrokes("f9");
    h.move_to(a, None, Modifiers::none());

    h.mouse_up(a, MouseButton::Left, Modifiers::none());

    assert_eq!(text_of(&mut h, "log"), "pointerup@a mouseup@a");
}
