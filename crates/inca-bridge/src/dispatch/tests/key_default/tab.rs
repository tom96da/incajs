// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui::test]
fn tab_moves_focus_to_the_next_stop(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui::test]
fn shift_tab_moves_focus_to_the_previous_stop(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.second);
    s.key(cx, "shift-tab", false, false);
    assert_eq!(s.focused(cx), Some(s.first));
}

#[gpui::test]
fn tab_up_leaves_focus_where_it_is(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "tab", true, false);
    assert_eq!(s.focused(cx), Some(s.first));
}

#[gpui_cases]
#[gpui::test]
#[case::control("ctrl-tab")]
#[case::alt("alt-tab")]
#[case::platform("cmd-tab")]
fn tab_with_a_command_modifier_leaves_focus_where_it_is(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, key, false, false);
    assert_eq!(s.focused(cx), Some(s.first));
}

#[gpui::test]
fn a_canceled_tab_down_leaves_focus_where_it_is(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "tab", false, true);
    assert_eq!(s.focused(cx), Some(s.first));
}

#[gpui::test]
fn a_tab_move_reports_its_focus_events_on_the_next_frame(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.engine.eval::<()>("globalThis.moves = [];").unwrap();
    for node in [s.first, s.second] {
        for event in ["blur", "focus"] {
            s.engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', (e) => {{
                        moves.push(`${{e.type}}:${{e.target}}:${{e.relatedTarget}}`);
                    }});
                    "
                ))
                .unwrap();
        }
    }
    s.key(cx, "tab", false, false);
    s.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    let got = s.engine.eval::<String>("moves.join()").unwrap();
    let (first, second) = (s.first, s.second);
    assert_eq!(got, format!("blur:{first}:{second},focus:{second}:{first}"));
}

impl Scene {
    /// Moves `plain` between `first` and `second`.
    fn plain_between_the_buttons(&self) {
        self.host
            .borrow_mut()
            .tree
            .insert_before(self.parent, self.plain, Some(self.second))
            .unwrap();
    }
}

#[gpui::test]
fn a_press_on_a_node_that_takes_no_focus_sets_where_tab_starts(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui::test]
fn a_canceled_press_sets_where_tab_starts(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.plain_between_the_buttons();
    s.engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({}, 'mousedown', (e) => {{
                e.preventDefault();
            }});
            ",
            s.plain
        ))
        .unwrap();
    s.mouse_down(cx, s.plain);
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui::test]
fn a_press_on_the_root_clears_where_tab_starts(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    let root = s.host.borrow().root();
    s.mouse_down(cx, root);
    s.key(cx, "shift-tab", false, false);
    assert_eq!(s.focused(cx), Some(s.field));
}

#[gpui::test]
fn shift_tab_after_a_pressed_button_loses_focus_focuses_it_again(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.mouse_down(cx, s.first);
    s.blur(cx, s.first);
    s.key(cx, "shift-tab", false, false);
    assert_eq!(s.focused(cx), Some(s.first));
}

#[gpui::test]
fn removing_the_pressed_node_moves_the_tab_start_before_it(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    s.engine
        .eval::<()>(&format!(
            "__inca_native__.removeChild({}, {});",
            s.parent, s.plain
        ))
        .unwrap();
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui::test]
fn moving_the_pressed_node_leaves_the_tab_start_in_its_place(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    s.engine
        .eval::<()>(&format!(
            "__inca_native__.appendChild({}, {});",
            s.parent, s.plain
        ))
        .unwrap();
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui::test]
fn destroying_the_pressed_node_moves_the_tab_start_before_it(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    s.engine
        .eval::<()>(&format!("__inca_native__.destroyNode({});", s.plain))
        .unwrap();
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui::test]
fn a_press_on_the_focused_node_leaves_a_later_focus_as_the_start(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.second);
    s.mouse_down(cx, s.second);
    s.blur(cx, s.second);
    s.focus(cx, s.second);
    s.blur(cx, s.second);
    s.key(cx, "shift-tab", false, false);
    assert_eq!(s.focused(cx), Some(s.first));
}
