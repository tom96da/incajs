// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui_cases]
#[gpui::test]
#[case::enter("enter")]
#[case::space("space")]
fn a_canceled_key_down_clicks_nothing(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, key, false, true);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn a_canceled_other_key_down_leaves_the_pending_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "a", false, true);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn space_up_after_a_canceled_space_down_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "space", false, true);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn a_canceled_space_up_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "space", true, true);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn a_second_space_up_after_a_canceled_one_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "space", true, true);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn a_canceled_enter_down_leaves_the_pending_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "enter", false, true);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui_cases]
#[gpui::test]
#[case::enter_click("enter", 0, "disabled", "true", None)]
#[case::tab_move("tab", 1, "tabindex", "'-1'", Some(2))]
fn a_job_queued_by_a_key_down_listener_lands_before_the_default_action(
    cx: &mut TestAppContext,
    key: &str,
    changed: usize,
    attribute: &str,
    value: &str,
    focused: Option<usize>,
) {
    let s = scene(cx);
    let nodes = [s.first, s.second, s.field];
    s.engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({}, 'keydown', () => {{
                Promise.resolve().then(() => {{
                    __inca_native__.setAttribute({}, '{attribute}', {value});
                }});
            }});
            ",
            s.first, nodes[changed]
        ))
        .unwrap();
    s.focus(cx, s.first);
    s.key(cx, key, false, false);
    assert_eq!(s.clicks(), "[]");
    assert_eq!(s.focused(cx), focused.map(|index| nodes[index]));
}

#[gpui_cases]
#[gpui::test]
#[case::enter("enter")]
#[case::space("space")]
fn an_ancestors_canceled_key_down_clicks_nothing(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.cancel_at_parent(true);
    s.key(cx, key, false, false);
    s.cancel_at_parent(false);
    s.key(cx, key, true, false);
    assert_eq!(s.clicks(), "[]");
}
