// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `bubbles`, `cancelable` and `composed` flags of each event, and the class of an event
//! object, in a real `.vue` app.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, node_id, sorted_unique, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};
use inca_host::harness::Harness;

/// The mouse triggers: hover, click, double click, right click, hover another node.
fn mouse_run(h: &mut Harness) {
    let tree = h.snapshot();
    let (plain, plain2) = (by_id(&tree, "plain").id, by_id(&tree, "plain2").id);
    h.hover(plain);
    h.click(plain);
    h.click_with(plain, MouseButton::Left, Modifiers::none(), 2);
    h.click_with(plain, MouseButton::Right, Modifiers::none(), 1);
    h.hover(plain2);
}

fn scroll_run(h: &mut Harness) {
    let scroller = node_id(h, "scroller");
    h.scroll(scroller, 0.0, -30.0);
}

fn log(h: &mut Harness, id: &str) -> Vec<String> {
    sorted_unique(&text_of(h, id))
}

fn input_run(h: &mut Harness) {
    click(h, "field");
    h.keystrokes("a");
    click(h, "plain");
}

#[gpui::test]
fn mouse_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    mouse_run(&mut h);

    assert_eq!(
        log(&mut h, "mouse"),
        [
            "auxclick:true:true:true",
            "click:true:true:true",
            "contextmenu:true:true:true",
            "dblclick:true:true:true",
            "mousedown:true:true:true",
            "mouseenter:false:false:false",
            "mouseleave:false:false:false",
            "mousemove:true:true:true",
            "mouseout:true:true:true",
            "mouseover:true:true:true",
            "mouseup:true:true:true",
        ]
    );
}

#[gpui::test]
fn pointer_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    mouse_run(&mut h);

    assert_eq!(
        log(&mut h, "pointer"),
        [
            "pointerdown:true:true:true",
            "pointerenter:false:false:false",
            "pointerleave:false:false:false",
            "pointermove:true:true:true",
            "pointerout:true:true:true",
            "pointerover:true:true:true",
            "pointerup:true:true:true",
        ]
    );
}

#[gpui::test]
fn keyboard_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");
    click(&mut h, "box");

    h.keystrokes("a");
    h.key_up("a");

    assert_eq!(
        log(&mut h, "key"),
        [
            "keydown:true:true:true",
            "keypress:true:true:true",
            "keyup:true:true:true"
        ]
    );
}

#[gpui::test]
fn focus_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    click(&mut h, "box");
    click(&mut h, "plain");

    assert_eq!(
        log(&mut h, "focus"),
        [
            "blur:false:false:true",
            "focus:false:false:true",
            "focusin:true:false:true",
            "focusout:true:false:true",
        ]
    );
}

#[gpui::test]
fn wheel_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    scroll_run(&mut h);

    assert_eq!(log(&mut h, "wheel"), ["wheel:true:true:true"]);
}

#[gpui::test]
fn scroll_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    scroll_run(&mut h);

    assert_eq!(log(&mut h, "scroll"), ["scroll:false:false:false"]);
}

#[gpui::test]
fn input_events_carry_their_bubbles_cancelable_and_composed_flags(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    input_run(&mut h);

    assert_eq!(
        log(&mut h, "input"),
        [
            "beforeinput:true:true:true",
            "change:true:false:false",
            "input:true:false:true",
        ]
    );
}

#[gpui::test]
fn preventing_a_non_cancelable_event_leaves_default_prevented_false(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    mouse_run(&mut h);
    click(&mut h, "box");
    click(&mut h, "plain");
    scroll_run(&mut h);
    input_run(&mut h);

    assert_eq!(
        log(&mut h, "prevented"),
        [
            "change:false",
            "focus:false",
            "focusin:false",
            "input:false",
            "mouseenter:false",
            "mouseleave:false",
            "scroll:false",
        ]
    );
}

#[gpui::test]
fn click_contextmenu_and_auxclick_are_pointer_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-flags");

    mouse_run(&mut h);
    scroll_run(&mut h);

    assert_eq!(
        log(&mut h, "class"),
        [
            "auxclick:PointerEvent",
            "click:PointerEvent",
            "contextmenu:PointerEvent",
            "dblclick:MouseEvent",
            "mousedown:MouseEvent",
            "scroll:Event",
        ]
    );
}
