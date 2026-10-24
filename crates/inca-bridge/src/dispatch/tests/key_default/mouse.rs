// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui::test]
fn a_mouse_press_on_the_button_leaves_the_pending_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.mouse_down(cx, s.first);
    s.mouse_up(cx, s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn space_up_clicks_the_focused_button_after_a_prevented_mouse_press_on_it(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({}, 'mousedown', (e) => {{
                e.preventDefault();
            }});
            ",
            s.first
        ))
        .unwrap();
    s.focus(cx, s.first);
    s.mouse_down(cx, s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn a_held_mouse_press_takes_one_space_up_click(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.mouse_down(cx, s.first);
    s.key(cx, "space", true, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn an_enter_click_ends_a_held_mouse_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.mouse_down(cx, s.first);
    s.key(cx, "enter", false, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn losing_focus_ends_a_held_mouse_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.mouse_down(cx, s.first);
    s.focus(cx, s.second);
    s.focus(cx, s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn a_space_press_after_a_held_mouse_click_clicks_again(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.mouse_down(cx, s.first);
    s.key(cx, "space", true, false);
    s.key(cx, "space", false, false);
    s.key(cx, "space", true, false);
    let click = format!("click:{}:true", s.first);
    assert_eq!(s.clicks(), format!(r#"["{click}","{click}"]"#));
}

#[gpui::test]
fn a_mouse_press_on_a_non_focusable_node_ends_the_pending_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.mouse_down(cx, s.plain);
    s.focus(cx, s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn space_up_clicks_the_button_under_a_held_mouse_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.mouse_down(cx, s.second);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.second));
}

#[gpui::test]
fn space_up_after_the_mouse_release_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.mouse_down(cx, s.first);
    s.mouse_up(cx, s.second);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn space_up_after_a_prevented_mouse_press_elsewhere_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({}, 'mousedown', (e) => {{
                e.preventDefault();
            }});
            ",
            s.second
        ))
        .unwrap();
    s.focus(cx, s.first);
    s.mouse_down(cx, s.second);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn enter_down_on_a_disabled_button_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.disable(s.first);
    s.key(cx, "enter", false, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn space_up_after_the_button_lost_focus_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.blur(cx, s.first);
    s.focus(cx, s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn enter_on_a_disabled_button_leaves_the_pending_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.disable(s.first);
    s.key(cx, "enter", false, false);
    s.enable(s.first);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui::test]
fn a_key_click_reaches_the_ancestors_while_a_disabled_button_is_hovered(cx: &mut TestAppContext) {
    let s = scene(cx);
    {
        let mut host = s.host.borrow_mut();
        host.tree.set_style(s.first, "width", 100.0).unwrap();
        host.tree.set_style(s.first, "height", 50.0).unwrap();
    }
    s.disable(s.first);
    s.engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({}, 'click', () => {{
                out.push('parent');
            }});
            ",
            s.parent
        ))
        .unwrap();
    s.focus(cx, s.second);
    s.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    let mut visual = gpui::VisualTestContext::from_window(s.window.into(), cx);
    visual.simulate_mouse_move(at(10.0, 10.0), None, gpui::Modifiers::none());
    s.key(cx, "enter", false, false);
    assert_eq!(
        s.clicks(),
        format!(r#"["click:{}:true","parent"]"#, s.second)
    );
}

#[gpui_cases]
#[gpui::test]
#[case::enter("enter")]
#[case::space("space")]
fn a_focused_descendant_of_a_button_does_not_click_it(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    let inner = {
        let mut host = s.host.borrow_mut();
        let inner = host.tree.create_node("div").unwrap();
        host.tree.set_attribute(inner, "tabindex", "-1").unwrap();
        host.tree.append_child(s.first, inner).unwrap();
        host.focus.mark_tab_dirty(inner);
        inner
    };
    s.focus(cx, inner);
    assert_eq!(s.focused(cx), Some(inner));
    s.key(cx, key, false, false);
    s.key(cx, key, true, false);
    assert_eq!(s.clicks(), "[]");
}

#[gpui::test]
fn a_press_on_a_disabled_button_focuses_its_focusable_ancestor(cx: &mut TestAppContext) {
    let s = scene(cx);
    {
        let mut host = s.host.borrow_mut();
        host.tree.set_attribute(s.parent, "tabindex", "0").unwrap();
        host.focus.mark_tab_dirty(s.parent);
    }
    s.disable(s.first);
    s.host.borrow_mut().focus.mark_tab_dirty(s.first);
    s.focus(cx, s.second);
    s.mouse_down(cx, s.first);
    assert_eq!(s.focused(cx), Some(s.parent));
}

#[gpui_cases]
#[gpui::test]
#[case::pointerdown("pointerdown")]
#[case::mousedown("mousedown")]
fn a_canceled_press_does_not_focus(cx: &mut TestAppContext, event: &str) {
    let s = scene(cx);
    s.engine
        .eval::<()>(&format!(
            "__inca_test_listen__({}, '{event}', (e) => e.preventDefault());",
            s.field
        ))
        .unwrap();
    s.mouse_down(cx, s.field);
    assert_eq!(s.focused(cx), None);
}
