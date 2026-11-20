// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The listeners a template binds: handler arrays, `.once`, the `onclick` property and the event and key modifiers.

#![allow(clippy::unwrap_used)]

use super::common::{click, click_with, load, text_of};
use gpui::{Modifiers, MouseButton, TestAppContext};

#[gpui::test]
fn click_once_runs_a_single_time_across_several_clicks(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-once");
    assert_eq!(text_of(&mut h, "count"), "0");

    click(&mut h, "box");
    click(&mut h, "box");
    click(&mut h, "box");

    assert_eq!(text_of(&mut h, "count"), "1");
}

#[gpui::test]
fn an_array_of_handlers_runs_every_handler_in_order(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners");

    click(&mut h, "all");

    assert_eq!(text_of(&mut h, "log"), "a b parent");
}

#[gpui::test]
fn stop_immediate_propagation_skips_the_later_handlers_and_the_parent(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners");

    click(&mut h, "halted");

    assert_eq!(text_of(&mut h, "log"), "a halt");
}

#[gpui::test]
fn a_ref_click_runs_the_click_listener_of_that_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-ref-click");

    click(&mut h, "go");

    assert_eq!(text_of(&mut h, "log"), "clicked:true");
}

#[gpui::test]
fn a_click_runs_the_function_a_template_binds_to_onclick(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-property");
    assert_eq!(text_of(&mut h, "count"), "0");

    click(&mut h, "bump");
    click(&mut h, "bump");

    assert_eq!(text_of(&mut h, "count"), "2");
}

#[gpui::test]
fn an_onclick_property_that_returns_false_cancels_the_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-property");

    click(&mut h, "cancel");

    assert_eq!(text_of(&mut h, "log"), "prevented:true");
}

#[gpui::test]
fn click_stop_keeps_the_ancestor_handler_from_running(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-modifiers");

    click(&mut h, "inner");

    assert_eq!(text_of(&mut h, "log"), "inner");
}

#[gpui::test]
fn keydown_enter_runs_for_enter(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-modifiers");

    h.keystrokes("enter");

    assert_eq!(text_of(&mut h, "log"), "enter");
}

#[gpui::test]
fn keydown_enter_skips_other_keys(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-modifiers");

    h.keystrokes("x");

    assert_eq!(text_of(&mut h, "log"), "");
}

#[gpui::test]
fn self_runs_only_for_the_node_itself_and_a_listener_reads_the_deep_target(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "listeners-self");

    click(&mut h, "inner");
    assert_eq!(text_of(&mut h, "log"), "");
    click(&mut h, "outer");
    click(&mut h, "kid");
    click(&mut h, "wrap");

    assert_eq!(text_of(&mut h, "log"), "self delegated wrap");
}

#[gpui::test]
fn contextmenu_prevent_runs_on_a_right_click(cx: &mut TestAppContext) {
    let mut h = load(cx, "listeners-mouse-button");

    click_with(&mut h, "box", MouseButton::Right, Modifiers::none(), 1);

    assert_eq!(text_of(&mut h, "log"), "right menu:true");
}
