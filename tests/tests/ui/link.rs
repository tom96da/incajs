// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Enter and Space on a focused link.

use super::common::{appended, click, load, text_of};
use gpui::TestAppContext;
use inca_host::harness::Harness;

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
fn enter_clicks_a_focused_link_on_key_down(cx: &mut TestAppContext) {
    let mut h = load(cx, "link");

    assert_eq!(
        press_release(&mut h, "", "enter"),
        "lnk:keydown:Enter lnk:click:0:0:false:true lnk:keyup:Enter"
    );
}

#[gpui::test]
fn space_does_not_click_a_focused_link(cx: &mut TestAppContext) {
    let mut h = load(cx, "link");

    assert_eq!(
        press_release(&mut h, "", "space"),
        "lnk:keydown:Space lnk:keypress:Space lnk:keyup:Space"
    );
}

#[gpui::test]
fn shift_enter_clicks_a_link_with_shift_set(cx: &mut TestAppContext) {
    let mut h = load(cx, "link");

    h.keystrokes("shift-enter");

    assert_eq!(
        text_of(&mut h, "log"),
        "lnk:keydown:Enter lnk:click:0:0:true:true"
    );
}

#[gpui::test]
fn a_canceled_key_down_leaves_a_link_unclicked(cx: &mut TestAppContext) {
    let mut h = load(cx, "link");
    let before = focus(&mut h, "stop");

    h.keystrokes("enter");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "stop:keydown:Enter"
    );
}

#[gpui::test]
fn a_link_without_href_does_not_click_on_enter(cx: &mut TestAppContext) {
    let mut h = load(cx, "link");
    let before = focus(&mut h, "bare");

    h.keystrokes("enter");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "bare:keydown:Enter bare:keypress:Enter"
    );
}

#[gpui::test]
fn a_link_with_the_disabled_attribute_still_clicks_on_enter(cx: &mut TestAppContext) {
    let mut h = load(cx, "link");
    let before = focus(&mut h, "dis");

    h.keystrokes("enter");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "dis:keydown:Enter dis:click:0:0:false:true"
    );
}

#[gpui::test]
fn a_canceled_enter_keypress_still_clicks_a_link(cx: &mut TestAppContext) {
    let mut h = load(cx, "link");
    let before = focus(&mut h, "stop-press");

    h.keystrokes("enter");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "stop-press:keydown:Enter stop-press:click:0:0:false:true"
    );
}

/// Tabs to an SVG link of `ns-keys`, presses Enter and returns what that added to `#log`.
fn enter_on_svg_link(cx: &mut TestAppContext, tabs: &str) -> String {
    let mut h = load(cx, "ns-keys");
    h.keystrokes(tabs);
    let before = text_of(&mut h, "log");
    press_release(&mut h, &before, "enter")
}

#[gpui::test]
fn enter_clicks_a_focused_svg_link_with_href(cx: &mut TestAppContext) {
    assert_eq!(
        enter_on_svg_link(cx, "tab"),
        "svg-href:keydown:Enter svg-href:click:0:0 svg-href:keyup:Enter"
    );
}

#[gpui::test]
fn enter_clicks_a_focused_svg_link_with_xlink_href(cx: &mut TestAppContext) {
    assert_eq!(
        enter_on_svg_link(cx, "tab tab"),
        "svg-xlink:keydown:Enter svg-xlink:click:0:0 svg-xlink:keyup:Enter"
    );
}
