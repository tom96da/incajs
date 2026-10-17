// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui::test]
fn space_up_after_space_down_clicks_the_button(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn space_up_without_space_down_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn space_up_with_a_modifier_clicks_the_button(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "ctrl-space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui_cases]
#[gpui::test]
#[case::down(false)]
#[case::up(true)]
fn another_key_leaves_the_pending_press(cx: &mut TestAppContext, up: bool) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "a", up, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn space_up_clicks_the_held_button_after_a_space_down_on_another(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.mouse_down(cx, s.second);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.second));
}

#[gpui::test]
fn space_up_with_a_pending_press_of_another_window_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    let other = scene(cx);
    let other_id = other.window_id(cx);
    s.focus(cx, s.first);
    s.set_pending(Some((other_id, s.first)));
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn space_up_after_space_down_while_disabled_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.disable(s.first);
    s.key(cx, "space", false, false);
    s.enable(s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn space_up_after_the_button_became_disabled_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.disable(s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui_cases]
#[gpui::test]
#[case::enter("enter")]
#[case::space("space")]
fn a_focused_non_button_does_not_click(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    s.focus(cx, s.field);
    s.key(cx, key, false, false);
    s.key(cx, key, true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn enter_down_on_a_focused_button_clicks_it(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "enter", false, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn enter_up_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "enter", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui_cases]
#[gpui::test]
#[case::control("ctrl-enter")]
#[case::alt("alt-enter")]
#[case::platform("cmd-enter")]
fn enter_with_a_command_modifier_clicks_nothing(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, key, false, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn enter_with_shift_clicks_the_button(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "shift-enter", false, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn space_up_after_an_enter_click_clicks_nothing_more(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "enter", false, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn space_down_after_an_enter_click_still_clicks_on_release(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "enter", false, false);
    s.key(cx, "space", false, false);
    s.key(cx, "space", true, false);
    let click = format!("click:{}:true", s.first);
    assert_eq!(s.clicks(), format!(r#"["{click}","{click}"]"#));
}

#[gpui_cases]
#[gpui::test]
#[case::blur_of_another_node("blur", false)]
#[case::focusout_of_the_node("focusout", true)]
fn space_up_after_a_focus_event_that_is_no_blur_of_the_node_clicks_the_button(
    cx: &mut TestAppContext,
    event: &str,
    same_node: bool,
) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    let node = if same_node { s.first } else { s.second };
    s.fire(cx, node, event);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn space_up_after_a_blur_of_the_node_in_another_window_clicks_the_button(cx: &mut TestAppContext) {
    let s = scene(cx);
    let other = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    let payload = EventPayload::Focus {
        related_target: None,
    };
    other.in_window(cx, |_, window, cx| {
        EventSink::fire(&s.dispatcher, s.first, "blur", &payload, window, cx);
    });
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui_cases]
#[gpui::test]
#[case::control("ctrl-enter", false)]
#[case::alt("alt-enter", false)]
#[case::platform("cmd-enter", false)]
#[case::enter_up("enter", true)]
fn enter_without_a_click_leaves_the_pending_press(cx: &mut TestAppContext, key: &str, up: bool) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, key, up, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui_cases]
#[gpui::test]
#[case::enter("enter")]
#[case::space("space")]
fn a_key_with_nothing_focused_clicks_nothing(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.blur(cx, s.first);
    s.key(cx, key, false, false);
    s.key(cx, key, true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn space_up_with_a_pending_press_of_another_button_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    let window_id = s.window_id(cx);
    s.focus(cx, s.first);
    s.set_pending(Some((window_id, s.second)));
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}
