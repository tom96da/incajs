// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Space and Enter on a focused button.

use super::common::{appended, click, load, text_of};
use gpui::TestAppContext;
use inca_host::harness::Harness;
use inca_test_macros::gpui_cases;

/// Clicks `id` to focus it and returns the log the click left.
fn focus(h: &mut Harness, id: &str) -> String {
    click(h, id);
    text_of(h, "log")
}

/// Presses `keys`, releases them and returns what they added to `#log`.
fn press_release(h: &mut Harness, before: &str, keys: &str) -> String {
    h.keystrokes(keys);
    h.key_up(keys);
    appended(before, &text_of(h, "log")).to_owned()
}

#[gpui::test]
fn space_clicks_on_key_up_after_keydown_and_keypress(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");

    assert_eq!(
        press_release(&mut h, "", "space"),
        "go:keydown:Space go:keypress:Space go:keyup:Space go:click:0:0:false:false:false:false"
    );
}

#[gpui::test]
fn space_key_down_alone_clicks_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");

    h.keystrokes("space");

    assert_eq!(text_of(&mut h, "log"), "go:keydown:Space go:keypress:Space");
}

#[gpui::test]
fn space_key_up_without_key_down_clicks_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");

    h.key_up("space");

    assert_eq!(text_of(&mut h, "log"), "go:keyup:Space");
}

#[gpui::test]
fn space_key_up_on_another_button_than_the_key_down_clicks_nothing(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    h.keystrokes("space");
    click(&mut h, "other");
    let before = text_of(&mut h, "log");

    h.key_up("space");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "other:keyup:Space"
    );
}

#[gpui::test]
fn enter_clicks_between_keypress_and_key_up(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");

    assert_eq!(
        press_release(&mut h, "", "enter"),
        "go:keydown:Enter go:keypress:Enter go:click:0:0:false:false:false:false go:keyup:Enter"
    );
}

#[gpui::test]
fn a_canceled_space_key_down_leaves_no_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "down-stop");

    assert_eq!(
        press_release(&mut h, &before, "space"),
        "down-stop:keydown:Space down-stop:keyup:Space"
    );
}

#[gpui::test]
fn a_canceled_enter_key_down_leaves_no_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "down-stop");

    h.keystrokes("enter");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "down-stop:keydown:Enter"
    );
}

#[gpui::test]
fn a_canceled_space_key_up_leaves_no_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "up-stop");

    assert_eq!(
        press_release(&mut h, &before, "space"),
        "up-stop:keydown:Space up-stop:keypress:Space up-stop:keyup:Space"
    );
}

#[gpui::test]
fn a_canceled_enter_key_up_still_clicks(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "up-stop");

    assert_eq!(
        press_release(&mut h, &before, "enter"),
        "up-stop:keydown:Enter up-stop:keypress:Enter \
         up-stop:click:0:0:false:false:false:false up-stop:keyup:Enter"
    );
}

#[gpui::test]
fn a_canceled_enter_keypress_leaves_no_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "press-stop");

    h.keystrokes("enter");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "press-stop:keydown:Enter press-stop:keypress:Enter"
    );
}

#[gpui::test]
fn a_canceled_space_keypress_still_clicks_on_key_up(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "press-stop");

    assert_eq!(
        press_release(&mut h, &before, "space"),
        "press-stop:keydown:Space press-stop:keypress:Space press-stop:keyup:Space \
         press-stop:click:0:0:false:false:false:false"
    );
}

#[gpui_cases]
#[gpui::test]
#[case::enter("enter", "Enter")]
#[case::space("space", "Space")]
fn a_button_disabled_by_its_key_down_listener_clicks_nothing(
    cx: &mut TestAppContext,
    #[case] keys: &str,
    #[case] key: &str,
) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "self-off");

    assert_eq!(
        press_release(&mut h, &before, keys),
        format!("self-off:keydown:{key} self-off:keypress:{key}")
    );
}

#[gpui_cases]
#[gpui::test]
#[case::enter("enter", "Enter")]
#[case::space("space", "Space")]
fn a_focused_non_button_does_not_click(
    cx: &mut TestAppContext,
    #[case] keys: &str,
    #[case] key: &str,
) {
    let mut h = load(cx, "button-keys");
    let before = focus(&mut h, "plain");

    assert_eq!(
        press_release(&mut h, &before, keys),
        format!("plain:keydown:{key} plain:keypress:{key} plain:keyup:{key}")
    );
}

#[gpui_cases]
#[gpui::test]
#[case::ctrl("ctrl-space", true, false, false)]
#[case::alt("alt-space", false, true, false)]
#[case::cmd("cmd-space", false, false, true)]
fn space_with_a_modifier_still_clicks_on_key_up(
    cx: &mut TestAppContext,
    #[case] keys: &str,
    #[case] ctrl: bool,
    #[case] alt: bool,
    #[case] meta: bool,
) {
    let mut h = load(cx, "button-keys");

    assert_eq!(
        press_release(&mut h, "", keys),
        format!("go:keydown:Space go:keyup:Space go:click:0:0:{ctrl}:{alt}:false:{meta}")
    );
}

#[gpui_cases]
#[gpui::test]
#[case::ctrl("ctrl-enter")]
#[case::alt("alt-enter")]
#[case::cmd("cmd-enter")]
fn enter_with_a_command_modifier_clicks_nothing(cx: &mut TestAppContext, #[case] keys: &str) {
    let mut h = load(cx, "button-keys");

    assert_eq!(
        press_release(&mut h, "", keys),
        "go:keydown:Enter go:keyup:Enter"
    );
}

#[gpui::test]
fn enter_while_space_is_held_drops_the_pending_space_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "button-keys");
    h.keystrokes("space");
    h.keystrokes("enter");
    let before = text_of(&mut h, "log");

    h.key_up("space");

    assert_eq!(appended(&before, &text_of(&mut h, "log")), "go:keyup:Space");
}

#[gpui_cases]
#[gpui::test]
#[case::enter("enter", "Enter")]
#[case::space("space", "Space")]
fn a_button_outside_the_html_namespace_does_not_click(
    cx: &mut TestAppContext,
    #[case] keys: &str,
    #[case] key: &str,
) {
    let mut h = load(cx, "ns-keys");
    h.keystrokes("tab tab tab");
    let before = text_of(&mut h, "log");

    assert_eq!(
        press_release(&mut h, &before, keys),
        format!("svg-btn:keydown:{key} svg-btn:keypress:{key} svg-btn:keyup:{key}")
    );
}
