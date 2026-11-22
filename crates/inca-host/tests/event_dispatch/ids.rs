// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

fn mouse_down(cx: &mut VisualTestContext) {
    cx.simulate_mouse_down(
        point(px(10.0), px(10.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
}

#[gpui::test]
fn each_input_event_gets_the_next_event_id(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, _child) = mount_recording_pair(cx, &["mousedown"]);

    mouse_down(&mut cx);
    mouse_down(&mut cx);
    mouse_down(&mut cx);

    let ids: Vec<u64> = take_ids(&engine).iter().map(|seen| seen.2).collect();
    assert_eq!(ids, [1, 1, 2, 2, 3, 3]);
}

#[gpui::test]
fn a_click_has_its_own_event_id_after_its_mousedown(cx: &mut TestAppContext) {
    let (mut cx, engine, _parent, child) = mount_recording_pair(cx, &["mousedown", "click"]);

    cx.simulate_click(point(px(10.0), px(10.0)), Modifiers::none());

    let seen = take_ids(&engine);
    let of = |name: &str, node: NodeId| {
        seen.iter()
            .find(|entry| entry.0 == name && entry.1 == node)
            .unwrap()
            .2
    };
    assert!(of("click", child) > of("mousedown", child));
}
