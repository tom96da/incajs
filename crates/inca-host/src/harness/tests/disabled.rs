// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn a_press_on_a_disabled_button_moves_focus_to_its_tabindex_ancestor(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    run_js(
        &mut h,
        r"
        globalThis.other = __inca_native__.createNode('div');
        __inca_native__.setAttribute(other, 'tabindex', '0');
        __inca_native__.setStyle(other, 'width', 10);
        __inca_native__.setStyle(other, 'height', 10);
        __inca_native__.appendChild(__inca_native__.rootNodeId(), other);
        __inca_native__.setAttribute(frame, 'tabindex', '0');
        __inca_native__.setAttribute(box, 'disabled', true);
        __inca_native__.focusNode(other);
        ",
    );
    listen(&mut h, "other", "focus blur", 0);
    listen(&mut h, "frame", "focus blur", 0);
    let button = node(&h, "box");

    h.click(button);

    assert_eq!(log(&h), "blur@kid/kid,focus@frame/frame");
}

#[gpui::test]
fn a_disabled_button_takes_no_focus_and_no_key_clicks(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "focus blur keydown click", 0);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'disabled', true);",
    );

    run_js(&mut h, "__inca_native__.focusNode(box);");
    h.keystrokes("enter space");
    h.key_up("space");

    assert_eq!(log(&h), "blur@box/box");
}

#[gpui::test]
fn disabling_a_focused_button_blurs_it_and_enabling_restores_focus(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "focus blur click", 0);

    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'disabled', true);",
    );
    run_js(&mut h, "__inca_native__.removeAttribute(box, 'disabled');");
    run_js(&mut h, "__inca_native__.focusNode(box);");
    h.keystrokes("enter");

    assert_eq!(log(&h), "blur@box/box,focus@box/box,click@box/box");
}

#[gpui::test]
fn a_disabled_button_and_its_ancestors_get_no_press_or_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    for target in ["box", "frame", "__inca_native__.rootNodeId()"] {
        listen(&mut h, target, "mousedown mouseup click", 0);
    }
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'disabled', true);",
    );
    let button = node(&h, "box");

    h.click(button);
    assert_eq!(log(&h), "");
    run_js(&mut h, "__inca_native__.removeAttribute(box, 'disabled');");
    h.click(button);

    assert!(log(&h).contains("click@box/box"), "{}", log(&h));
}

#[gpui::test]
fn a_disabled_button_and_its_ancestors_get_no_dblclick(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    for target in ["box", "frame", "__inca_native__.rootNodeId()"] {
        listen(&mut h, target, "dblclick", 0);
    }
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'disabled', true);",
    );
    let button = node(&h, "box");

    h.click_with(button, gpui::MouseButton::Left, Modifiers::none(), 1);
    h.click_with(button, gpui::MouseButton::Left, Modifiers::none(), 2);
    assert_eq!(log(&h), "");
    run_js(&mut h, "__inca_native__.removeAttribute(box, 'disabled');");
    h.click_with(button, gpui::MouseButton::Left, Modifiers::none(), 2);

    assert!(log(&h).contains("dblclick@box/box"), "{}", log(&h));
}

#[gpui::test]
fn a_press_on_a_disabled_button_released_on_an_ancestor_is_dropped(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    for target in ["box", "frame", "__inca_native__.rootNodeId()"] {
        listen(&mut h, target, "mouseup click", 0);
    }
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'disabled', true);",
    );
    h.cx.simulate_event(gpui::MouseDownEvent {
        position: point(px(10.0), px(10.0)),
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    h.cx.simulate_event(gpui::MouseUpEvent {
        position: point(px(150.0), px(150.0)),
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    h.settle();
    assert_eq!(log(&h), "");

    run_js(&mut h, "__inca_native__.removeAttribute(box, 'disabled');");
    let button = node(&h, "box");
    h.click(button);
    assert!(log(&h).contains("click@box/box"), "{}", log(&h));
}

#[gpui::test]
fn a_disabled_button_keeps_hover_and_move_events(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "mouseenter mousemove", 0);
    listen(&mut h, "frame", "mousemove", 0);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'disabled', true);",
    );
    let button = node(&h, "box");

    // The button is focusable, so the pointer rests on it from load.
    h.unhover();
    h.hover(button);

    let log = log(&h);
    for want in [
        "mouseenter@box/box",
        "mousemove@box/box",
        "mousemove@frame/box",
    ] {
        assert!(log.contains(want), "{log}");
    }
}

#[gpui::test]
fn a_disabled_button_keeps_a_descendant_listener(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    run_js(
        &mut h,
        r"
        globalThis.k = __inca_native__.createNode('div');
        __inca_native__.setStyle(k, 'width', 100);
        __inca_native__.setStyle(k, 'height', 50);
        __inca_native__.appendChild(box, k);
        listen(k, 'click', 0);
        listen(box, 'click', 0);
        listen(frame, 'click', 0);
        __inca_native__.setAttribute(box, 'disabled', true);
        ",
    );
    let kid = node(&h, "k");

    h.click(kid);

    assert_eq!(log(&h), "click@kid/kid");
}
