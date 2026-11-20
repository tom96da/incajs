// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The events one click fires, in order, with their fields.

use super::common::{click, click_with, load, settled, text_of};
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
    let mut h = load(cx, "click-order");

    click(&mut h, "box");

    assert_eq!(text_of(&mut h, "log"), LEFT_CLICK);
}

#[gpui::test]
fn a_second_click_of_a_double_click_ends_with_dblclick(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-order");

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
    let mut h = load(cx, "click-order");

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
    let mut h = load(cx, "click-order");

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
    let mut h = load(cx, "click-order");

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
    let mut h = load(cx, "click-order");

    click(&mut h, "box2");
    settled(&mut h, 1);
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "");
    assert_eq!(text_of(&mut h, "focus"), "");
}

#[gpui::test]
fn a_canceled_pointerdown_skips_the_mouse_press_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-order");

    click(&mut h, "box3");

    assert_eq!(
        text_of(&mut h, "log"),
        "box3:pointerdown:0:0:1 box3:pointerup:0:0:0 box3:click:1:0:0"
    );
}

#[gpui::test]
fn a_canceled_pointerdown_leaves_the_node_unfocused(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-order");

    click(&mut h, "box3");
    settled(&mut h, 1);
    click(&mut h, "read");

    assert_eq!(text_of(&mut h, "active"), "");
}

#[gpui::test]
fn a_canceled_pointerdown_still_fires_contextmenu_on_a_right_press(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-order");

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
    let mut h = load(cx, "click-order");

    click_with(&mut h, "box", MouseButton::Left, modifiers, 1);

    assert_eq!(text_of(&mut h, "mods"), expected);
}

#[gpui::test]
fn a_control_click_fires_the_press_events_the_platform_gives(cx: &mut TestAppContext) {
    let mut h = load(cx, "click-order");

    click_with(&mut h, "box", MouseButton::Left, Modifiers::control(), 1);

    assert_eq!(text_of(&mut h, "log"), CTRL_CLICK_LOG);
}
