// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of key events.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_document::{EventPayload, EventTargetId};
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::support::*;
use inca_document::{EVENTS, MousePayload, PointerSource};
use inca_gpui::{EventSink, InputLayout};
use inca_node::NodeId;

/// `charCode`, `keyCode` and `which` are read off the event by the script.
#[gpui::test]
fn a_keypress_of_enter_carries_the_character_code(cx: &mut TestAppContext) {
    let s = js_key_input_scene(cx, "");
    listen(
        &s.dispatcher,
        s.field,
        "keypress",
        "(e) => out.push([e.key, e.charCode, e.keyCode, e.which, e.cancelable])",
    );
    s.focus(cx, s.field);
    s.press(cx, "enter");
    assert_eq!(
        s.dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap(),
        r#"[["Enter",13,13,13,true]]"#
    );
}

/// `charCode`, `keyCode` and `which` are read off the event by the script.
#[cfg(target_os = "macos")]
#[gpui_cases]
#[gpui::test]
#[case::option_s("alt-s->ß", "ß", 223)]
fn an_option_key_fires_keypress_with_the_composed_character(
    cx: &mut TestAppContext,
    pressed: &str,
    key: &str,
    code: u32,
) {
    let s = js_key_input_scene(cx, "");
    listen(
        &s.dispatcher,
        s.field,
        "keypress",
        "(e) => out.push([e.key, e.charCode, e.keyCode, e.which])",
    );
    s.focus(cx, s.field);
    s.press(cx, pressed);
    assert_eq!(
        s.dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap(),
        format!(r#"[["{key}",{code},{code},{code}]]"#),
        "{pressed}"
    );
}

/// The `destroyNode` of the listener releases the listeners of the target, so the later
/// `keypress` walk finds none and fires nothing. The test pins that the key path survives a
/// target that is gone.
#[gpui::test]
fn a_keydown_that_destroys_its_node_fires_no_keypress(cx: &mut TestAppContext) {
    let s = js_key_input_scene(cx, "");
    listen(
        &s.dispatcher,
        s.field,
        "keypress",
        "(e) => out.push('keypress')",
    );
    listen(
        &s.dispatcher,
        s.field,
        "keydown",
        &format!("(e) => __inca_native__.destroyNode({})", s.field),
    );
    s.focus(cx, s.field);
    s.press(cx, "x");
    assert_eq!(
        s.dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap(),
        "[]"
    );
}

/// The tab start follows what the native tree calls do to the pressed node.
#[gpui::test]
fn removing_the_pressed_node_moves_the_tab_start_before_it(cx: &mut TestAppContext) {
    let s = js_scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    s.dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_native__.removeChild({}, {});",
            s.parent, s.plain
        ))
        .unwrap();
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui_cases]
#[gpui::test]
#[case::replace_child("replaceChild({parent}, {first}, {plain})", |s: &Scene| s.first)]
#[case::insert_nodes("insertNodes({field}, 'after', [{first}, {plain}])", |s: &Scene| s.second)]
fn a_call_that_takes_several_nodes_out_starts_tab_before_all_of_them(
    cx: &mut TestAppContext,
    #[case] call: &str,
    #[case] focused: fn(&Scene) -> NodeId,
) {
    let s = js_scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    let call = call
        .replace("{parent}", &s.parent.to_string())
        .replace("{first}", &s.first.to_string())
        .replace("{plain}", &s.plain.to_string())
        .replace("{field}", &s.field.to_string());
    s.dispatcher
        .engine
        .eval::<()>(&format!("__inca_native__.{call};"))
        .unwrap();
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(focused(&s)));
}

#[gpui::test]
fn moving_the_pressed_node_leaves_the_tab_start_in_its_place(cx: &mut TestAppContext) {
    let s = js_scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    s.dispatcher
        .engine
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
    let s = js_scene(cx);
    s.plain_between_the_buttons();
    s.mouse_down(cx, s.plain);
    s.dispatcher
        .engine
        .eval::<()>(&format!("__inca_native__.destroyNode({});", s.plain))
        .unwrap();
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

/// Presses Tab, with `removeAttribute(link, name)` run by the `keydown` listener of the link.
#[gpui::test]
fn removing_href_in_the_keydown_of_a_tab_press_moves_focus_past_the_link(cx: &mut TestAppContext) {
    let s = js_scene(cx);
    let link = s.add_link(HTML, "a", &[("href", "u")]);
    s.dispatcher
        .engine
        .eval::<()>(&format!(
            r"
                __inca_test_listen__({link}, 'keydown', () => {{
                    __inca_native__.removeAttribute({link}, 'href');
                }});
                "
        ))
        .unwrap();
    s.focus(cx, link);
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.field));
}

#[gpui_cases]
#[gpui::test]
#[case::href(HTML, "href")]
#[case::svg_xlink_href(SVG, "xlink:href")]
fn removing_the_link_attribute_blurs_the_focused_a_on_the_next_frame(
    cx: &mut TestAppContext,
    namespace: &str,
    attribute: &str,
) {
    let s = js_scene(cx);
    let link = s.add_link(namespace, "a", &[(attribute, "u")]);
    s.focus(cx, link);
    s.dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_native__.removeAttribute({link}, '{attribute}');"
        ))
        .unwrap();
    assert_eq!(s.focused(cx), Some(link));
    s.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    assert_eq!(s.focused(cx), None);
}

/// `pointerId`, `pointerType`, `which` and `pressure` are read off the event by the script.
#[gpui::test]
fn the_contextmenu_of_the_menu_key_carries_the_mouse_pointer_fields(cx: &mut TestAppContext) {
    let s = js_scene(cx);
    s.focus(cx, s.field);
    listen(
        &s.dispatcher,
        s.field,
        "contextmenu",
        r"
            (e) => out.push([
                e.isTrusted, e.pointerId, e.pointerType, e.isPrimary, e.button,
                e.buttons, e.which, e.detail, e.pressure,
            ])
            ",
    );
    s.key(cx, "menu", false, false);
    assert_eq!(
        s.dispatcher
            .engine
            .eval::<String>("JSON.stringify(out)")
            .unwrap(),
        r#"[[true,1,"mouse",false,-1,0,0,0,0]]"#
    );
}
