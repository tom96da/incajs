// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

/// Records every `contextmenu` and `keydown` into `globalThis.log`.
fn record(s: &Scene, node: NodeId) {
    s.engine
        .eval::<()>(&format!(
            r"
            globalThis.log = [];
            __inca_test_listen__({node}, 'keydown', (e) => log.push(e.type));
            __inca_test_listen__({node}, 'contextmenu', (e) => log.push([
                e.type, e.target, e.isTrusted, e.pointerId, e.pointerType, e.isPrimary, e.button,
                e.buttons, e.which, e.detail, e.pressure, e.clientX, e.clientY
            ]));
            "
        ))
        .unwrap();
}

fn log(s: &Scene) -> serde_json::Value {
    let json = s.engine.eval::<String>("JSON.stringify(log)").unwrap();
    serde_json::from_str(&json).unwrap()
}

fn center_of(s: &Scene, node: NodeId) -> (f64, f64) {
    let layout = s.host.borrow().layout.borrow().get(&node).copied().unwrap();
    let center = layout.bounds.center();
    (
        f64::from(f32::from(center.x)),
        f64::from(f32::from(center.y)),
    )
}

#[gpui::test]
fn contextmenu_key_dispatches_to_focused_node(cx: &mut TestAppContext) {
    let s = scene(cx);
    record(&s, s.field);
    s.focus(cx, s.field);
    let (x, y) = center_of(&s, s.field);
    s.key(cx, "menu", false, false);
    s.key(cx, "menu", true, false);
    let log = log(&s);
    assert_eq!(log[0], "keydown");
    assert_eq!(
        log[1].as_array().unwrap()[..11],
        serde_json::json!([
            "contextmenu",
            s.field,
            true,
            1,
            "mouse",
            false,
            -1,
            0,
            0,
            0,
            0
        ])
        .as_array()
        .unwrap()[..11]
    );
    assert_eq!(
        (log[1][11].as_f64(), log[1][12].as_f64()),
        (Some(x), Some(y))
    );
}

#[gpui_cases]
#[gpui::test]
#[case::shift_f10("shift-f10", false, usize::from(!cfg!(target_os = "macos")))]
#[case::f10("f10", false, 0)]
#[case::shift_menu("shift-menu", false, 0)]
#[case::keyup("shift-f10", true, 0)]
fn only_shift_f10_keydown_dispatches_contextmenu(
    cx: &mut TestAppContext,
    key: &str,
    up: bool,
    want: usize,
) {
    let s = scene(cx);
    record(&s, s.field);
    s.focus(cx, s.field);
    s.key(cx, key, up, false);
    let menus = log(&s)
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry.is_array())
        .count();
    assert_eq!(menus, want);
}

#[gpui_cases]
#[gpui::test]
#[case::menu("menu")]
#[case::shift_f10("shift-f10")]
fn a_canceled_contextmenu_keydown_suppresses_contextmenu(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    record(&s, s.field);
    s.focus(cx, s.field);
    s.key(cx, key, false, true);
    assert_eq!(log(&s), serde_json::json!(["keydown"]));
}

#[gpui::test]
fn contextmenu_key_without_focus_targets_the_container_at_1_1(cx: &mut TestAppContext) {
    let s = scene(cx);
    for style in ["width", "height"] {
        s.host
            .borrow_mut()
            .tree
            .set_style(s.first, style, 100.0)
            .unwrap();
    }
    s.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    let root = s.host.borrow().root();
    record(&s, root);
    s.key(cx, "menu", false, false);
    assert_eq!(
        log(&s)[1],
        serde_json::json!([
            "contextmenu",
            s.first,
            true,
            1,
            "mouse",
            false,
            -1,
            0,
            0,
            0,
            0,
            1,
            1
        ])
    );
}

#[gpui::test]
fn contextmenu_key_on_a_focused_node_without_a_box_targets_the_container_at_1_1(
    cx: &mut TestAppContext,
) {
    let s = scene(cx);
    s.host
        .borrow_mut()
        .tree
        .set_style(s.first, "width", 100.0)
        .unwrap();
    s.host
        .borrow_mut()
        .tree
        .set_style(s.first, "height", 100.0)
        .unwrap();
    s.focus(cx, s.field);
    s.host.borrow().layout.borrow_mut().remove(&s.field);
    let root = s.host.borrow().root();
    record(&s, root);
    s.key(cx, "menu", false, false);
    let log = log(&s);
    assert_eq!(log[1][1], serde_json::json!(s.first));
    assert_eq!(
        (log[1][11].as_f64(), log[1][12].as_f64()),
        (Some(1.0), Some(1.0))
    );
}

#[gpui::test]
fn contextmenu_key_outside_every_box_targets_the_root(cx: &mut TestAppContext) {
    let s = scene(cx);
    let root = s.host.borrow().root();
    record(&s, root);
    s.key(cx, "menu", false, false);
    assert_eq!(log(&s)[1][1], serde_json::json!(root));
}

#[gpui_cases]
#[gpui::test]
#[case::linux_and_windows(false, true)]
#[case::macos(true, false)]
fn shift_f10_opens_the_context_menu_except_on_macos(
    _cx: &mut TestAppContext,
    macos: bool,
    want: bool,
) {
    assert_eq!(
        crate::dispatch::sink::shift_f10_opens_context_menu(macos),
        want
    );
}
