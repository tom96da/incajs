// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn enter_clicks_a_focused_button_on_key_down_each_time(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "keydown keyup click", 0);

    h.keystrokes("enter");
    assert_eq!(log(&h), "keydown@box/box,click@box/box");
    h.keystrokes("enter");
    h.key_up("enter");

    assert_eq!(
        log(&h),
        "keydown@box/box,click@box/box,keydown@box/box,click@box/box,keyup@box/box"
    );
}

#[gpui::test]
fn a_job_queued_by_a_key_down_listener_lands_before_the_enter_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    run_js(
        &mut h,
        "cb[5] = () => { Promise.resolve().then(() => \
            __inca_native__.setAttribute(box, 'disabled', true)); }; \
         listen(box, 'keydown', 5);",
    );
    listen(&mut h, "box", "click", 0);

    h.keystrokes("enter");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn space_clicks_a_focused_button_once_after_key_up(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "keydown keyup click", 0);

    h.keystrokes("space");
    h.keystrokes("space");
    assert_eq!(log(&h), "keydown@box/box,keydown@box/box");
    h.key_up("space");
    h.key_up("space");

    assert_eq!(
        log(&h),
        "keydown@box/box,keydown@box/box,keyup@box/box,click@box/box,keyup@box/box"
    );
}

#[gpui::test]
fn a_space_key_up_clicks_only_after_its_key_down(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 0);

    h.key_up("space");
    assert_eq!(log(&h), "");

    h.keystrokes("space");
    h.key_up("space");
    assert_eq!(log(&h), "click@box/box");
}

#[gpui::test]
fn enter_clicks_with_shift_and_not_with_control_alt_or_meta(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 3);

    h.keystrokes("ctrl-enter alt-enter cmd-enter");
    assert_eq!(log(&h), "");
    h.keystrokes("enter shift-enter");

    assert_eq!(
        log(&h),
        "0:0:0:0:0:false:-1::false:1:1:0,0:0:0:0:0:true:-1::false:1:1:0"
    );
}

#[gpui::test]
fn a_pending_space_press_ends_at_the_next_key_press_or_mouse_press(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 0);

    h.keystrokes("space a");
    h.key_up("space");
    assert_eq!(log(&h), "");

    h.keystrokes("space");
    let frame = node(&h, "frame");
    h.click(frame);
    run_js(&mut h, "__inca_native__.focusNode(box);");
    h.key_up("space");
    assert_eq!(log(&h), "");

    h.keystrokes("space");
    h.key_up("space");
    assert_eq!(log(&h), "click@box/box");
}

#[gpui::test]
fn a_pending_space_press_does_not_click_another_button(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    run_js(
        &mut h,
        "globalThis.b2 = __inca_native__.createNode('button'); \
         __inca_native__.appendChild(frame, b2);",
    );
    listen(&mut h, "box", "click", 0);
    listen(&mut h, "b2", "click", 0);

    h.keystrokes("space");
    run_js(&mut h, "__inca_native__.focusNode(b2);");
    h.key_up("space");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn space_clicks_on_key_up_with_a_modifier_held(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 0);

    h.keystrokes("space");
    h.key_up("ctrl-space");

    assert_eq!(log(&h), "click@box/box");
}

#[gpui::test]
fn a_prevented_key_down_cancels_the_enter_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "keydown", 1);
    listen(&mut h, "box", "click", 0);

    h.keystrokes("enter");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn a_prevented_key_down_cancels_the_space_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "keydown", 1);
    listen(&mut h, "box", "click", 0);

    h.keystrokes("space");
    h.key_up("space");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn an_ancestors_prevented_key_down_cancels_the_key_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "frame", "keydown", 1);
    listen(&mut h, "box", "click", 0);

    h.keystrokes("enter space");
    h.key_up("space");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn a_prevented_key_up_cancels_the_space_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "keyup", 1);
    listen(&mut h, "box", "click", 0);

    h.keystrokes("space");
    h.key_up("space");
    assert_eq!(log(&h), "");
    // The cancelled press leaves nothing pending.
    run_js(
        &mut h,
        "__inca_native__.removeEventListener(box, 'keyup', 1);",
    );
    h.key_up("space");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn a_key_click_bubbles_with_the_button_as_target(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 0);
    listen(&mut h, "frame", "click", 0);
    listen(&mut h, "__inca_native__.rootNodeId()", "click", 0);

    h.keystrokes("enter");

    assert_eq!(log(&h), "click@box/box,click@frame/box,click@root/box");
}

#[gpui::test]
fn stopping_a_key_click_keeps_it_from_the_ancestors(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 2);
    listen(&mut h, "frame", "click", 0);

    h.keystrokes("enter");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn a_key_press_in_a_focused_descendant_does_not_click_the_button(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 0);
    run_js(
        &mut h,
        "globalThis.k = __inca_native__.createNode('div'); \
         __inca_native__.setAttribute(k, 'tabindex', '0'); \
         __inca_native__.appendChild(box, k); \
         __inca_native__.focusNode(k);",
    );

    h.keystrokes("enter space");
    h.key_up("space");

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn enter_and_space_click_only_a_button(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    run_js(
        &mut h,
        "const d = __inca_native__.createNode('div'); \
         __inca_native__.setAttribute(d, 'tabindex', '0'); \
         __inca_native__.appendChild(frame, d); \
         listen(d, 'click', 0); \
         listen(d, 'keydown', 0); \
         __inca_native__.focusNode(d);",
    );

    h.keystrokes("enter space");
    h.key_up("enter space");

    assert_eq!(log(&h), "keydown@kid/kid,keydown@kid/kid");
}

#[gpui::test]
fn a_key_click_on_an_enabled_button_reaches_the_ancestor_after_a_disabled_hover(
    cx: &mut TestAppContext,
) {
    let mut h = load_listening(cx, BUTTON);
    run_js(
        &mut h,
        "globalThis.b2 = __inca_native__.createNode('button'); \
         __inca_native__.setStyle(b2, 'width', 100); \
         __inca_native__.setStyle(b2, 'height', 50); \
         __inca_native__.appendChild(frame, b2); \
         __inca_native__.setAttribute(box, 'disabled', true);",
    );
    listen(&mut h, "frame", "click", 0);
    let button = node(&h, "box");
    h.hover(button);
    run_js(&mut h, "__inca_native__.focusNode(b2);");

    h.keystrokes("enter");

    assert_eq!(log(&h), "click@frame/kid");
}
