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
