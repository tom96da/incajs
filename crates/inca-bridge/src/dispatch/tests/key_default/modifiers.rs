// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui::test]
fn space_down_with_modifiers_still_starts_a_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "ctrl-space", false, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
}

#[gpui_cases]
#[gpui::test]
#[case::shift_enter("shift-enter", false, "true,false,false,false")]
#[case::control_space_up("ctrl-space", true, "false,true,false,false")]
#[case::alt_space_up("alt-space", true, "false,false,true,false")]
#[case::meta_space_up("cmd-space", true, "false,false,false,true")]
fn a_key_click_carries_the_modifiers_of_its_key(
    cx: &mut TestAppContext,
    key: &str,
    up: bool,
    want: &str,
) {
    let s = scene(cx);
    s.engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({}, 'click', (e) => {{
                globalThis.modifiers = [e.shiftKey, e.ctrlKey, e.altKey, e.metaKey].join();
            }});
            ",
            s.first
        ))
        .unwrap();
    s.focus(cx, s.first);
    if up {
        s.key(cx, "space", false, false);
    }
    s.key(cx, key, up, false);
    let got = s.engine.eval::<String>("globalThis.modifiers").unwrap();
    assert_eq!(got, want);
}
