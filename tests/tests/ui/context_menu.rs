// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Menu key and Shift+F10 open the context menu.

use super::common::{appended, by_id, click, load, text_of};
use gpui::TestAppContext;

/// What Shift+F10 logs on `box`: the menu opens everywhere but on macOS.
const SHIFT_F10_LOG: &str = if cfg!(target_os = "macos") {
    "keydown:F10 keyup:F10"
} else {
    "keydown:F10 contextmenu:-1:0:0:50:60:true keyup:F10"
};

#[gpui::test]
fn the_menu_key_fires_contextmenu_at_the_focused_node_center(cx: &mut TestAppContext) {
    let mut h = load(cx, "context-menu");

    h.keystrokes("menu");
    h.key_up("menu");

    assert_eq!(
        text_of(&mut h, "log"),
        "keydown:ContextMenu contextmenu:-1:0:0:50:60:true keyup:ContextMenu"
    );
}

#[gpui::test]
fn shift_f10_fires_contextmenu_where_the_platform_opens_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "context-menu");

    h.keystrokes("shift-f10");
    h.key_up("shift-f10");

    assert_eq!(text_of(&mut h, "log"), SHIFT_F10_LOG);
}

#[gpui::test]
fn a_canceled_menu_key_down_fires_no_contextmenu(cx: &mut TestAppContext) {
    let mut h = load(cx, "context-menu");
    let stop = by_id(&h.snapshot(), "box-stop").id;
    h.click(stop);

    h.keystrokes("menu");

    assert_eq!(text_of(&mut h, "log"), "keydown:ContextMenu");
    assert_eq!(text_of(&mut h, "win"), "");
}

#[gpui::test]
fn the_menu_key_with_nothing_focused_fires_contextmenu_at_the_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "context-menu");
    click(&mut h, "unfocus");
    let before = text_of(&mut h, "win");

    h.keystrokes("menu");

    assert_eq!(
        appended(&before, &text_of(&mut h, "win")),
        "contextmenu@spacer:1:1"
    );
}

#[gpui::test]
fn shift_with_the_menu_key_fires_no_contextmenu(cx: &mut TestAppContext) {
    let mut h = load(cx, "context-menu");

    h.keystrokes("shift-menu");
    h.key_up("shift-menu");

    assert_eq!(
        text_of(&mut h, "log"),
        "keydown:ContextMenu keyup:ContextMenu"
    );
}
