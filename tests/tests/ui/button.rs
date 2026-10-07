// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A `button` in a real `.vue` app.

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn enter_clicks_on_key_down_and_bubbles_with_the_button_as_target(cx: &mut TestAppContext) {
    let mut h = load(cx, "button");

    h.keystrokes("enter");

    assert_eq!(
        text_of(&mut h, "log"),
        "go:focus go:keydown:Enter go:click:0:0:0:false wrap:click:go"
    );
}

#[gpui::test]
fn disabled_false_enables_the_button_and_true_disables_it_again(cx: &mut TestAppContext) {
    let mut h = load(cx, "button");
    let before = text_of(&mut h, "log");
    click(&mut h, "off");
    assert_eq!(text_of(&mut h, "log"), before);

    click(&mut h, "toggle");
    click(&mut h, "off");
    let enabled = text_of(&mut h, "log");
    for event in [
        "off:mousedown",
        "off-wrap:mouseup",
        "off:click",
        "off-wrap:click",
        "off:focus",
    ] {
        assert!(enabled.contains(event), "{event} missing from {enabled}");
    }

    click(&mut h, "toggle");
    click(&mut h, "off");

    assert_eq!(text_of(&mut h, "log"), enabled);
}
