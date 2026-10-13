// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// A > B > (C, D), each listening for the four hover events. The pointer
/// starts outside A.
const HOVER_ORDER: &str = r"
        const n = __inca_native__;
        const make = (parent, w, h) => {
            const id = n.createNode('div');
            n.setStyle(id, 'width', w);
            n.setStyle(id, 'height', h);
            n.appendChild(parent, id);
            return id;
        };
        const A = make(n.rootNodeId(), 200, 200);
        n.setStyle(A, 'margin_left', 50);
        const B = make(A, 100, 100);
        const C = make(B, 30, 30);
        const D = make(B, 30, 30);
        Object.assign(globalThis, { A, B, C, D });
        globalThis.log = [];
        const names = { [n.rootNodeId()]: 'root', [A]: 'A', [B]: 'B', [C]: 'C', [D]: 'D' };
        globalThis.cb = {
            0: (e) => { globalThis.log.push(`${e.type}@${names[e.currentTarget]}/${names[e.target]}/${
                e.relatedTarget === null ? 'none' : names[e.relatedTarget]}`); },
        };
        for (const id of [A, B, C, D]) {
            for (const t of ['mouseover', 'mouseout', 'mouseenter', 'mouseleave']) {
                listen(id, t, 0);
            }
        }
    ";

#[gpui::test]
fn entering_a_nested_node_fires_over_then_enter_on_each_ancestor(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, HOVER_ORDER);
    let c = node(&h, "C");
    targets_log(&mut h);

    h.hover(c);

    assert_eq!(
        targets_log(&mut h),
        concat!(
            "mouseover@C/C/root,mouseover@B/C/root,mouseover@A/C/root,",
            "mouseenter@A/A/root,mouseenter@B/B/root,mouseenter@C/C/root"
        )
    );
}

#[gpui::test]
fn a_replaced_node_under_a_resting_pointer_gets_enter_events(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, HOVER_ORDER);
    let d = node(&h, "D");
    h.hover(d);
    targets_log(&mut h);

    run_js(
        &mut h,
        r"
            const n = __inca_native__;
            n.removeChild(B, D);
            const N = n.createNode('div');
            n.setStyle(N, 'width', 30);
            n.setStyle(N, 'height', 30);
            n.appendChild(B, N);
            globalThis.N = N;
            for (const t of ['mouseover', 'mouseenter']) {
                listen(N, t, 0);
            }
            ",
    );

    // The new node is outside the name table, so the log spells it undefined.
    assert_eq!(
        targets_log(&mut h),
        concat!(
            "mouseover@undefined/undefined/B,mouseover@B/undefined/B,",
            "mouseover@A/undefined/B,mouseenter@undefined/undefined/B"
        )
    );
}

#[gpui::test]
fn swapping_a_node_under_a_resting_pointer_keeps_the_parent_hovered(cx: &mut TestAppContext) {
    let mut h = load_listening(
        cx,
        r"
            const n = __inca_native__;
            const make = (parent, tag, w, h) => {
                const id = n.createNode(tag);
                n.setStyle(id, 'width', w);
                n.setStyle(id, 'height', h);
                n.appendChild(parent, id);
                return id;
            };
            const F = make(n.rootNodeId(), 'div', 200, 200);
            const K = make(F, 'div', 200, 30);
            n.setAttribute(K, 'tabindex', '0');
            const P = make(F, 'div', 200, 50);
            n.setStyle(P, 'padding', 4);
            n.setStyle(P, 'border_width', 1);
            const D = make(P, 'div', 180, 30);
            make(D, 'div', 100, 10);
            Object.assign(globalThis, { K, P, D });
            globalThis.log = [];
            const names = { [P]: 'P', [D]: 'D' };
            globalThis.cb = {
                0: (e) => { globalThis.log.push(`${e.type}@${names[e.currentTarget] ?? 'new'}/${
                    names[e.target] ?? 'new'}/${
                    (e.relatedTarget === null ? 'none' : (names[e.relatedTarget] ?? 'new'))}`); },
                1: () => {
                    n.removeChild(P, D);
                    const N = make(P, 'button', 180, 30);
                    make(N, 'div', 100, 10);
                    for (const t of ['mouseover', 'mouseout', 'mouseenter', 'mouseleave']) {
                        listen(N, t, 0);
                    }
                },
            };
            for (const t of ['mouseover', 'mouseout', 'mouseenter', 'mouseleave']) {
                listen(P, t, 0);
                listen(D, t, 0);
            }
            listen(K, 'keydown', 1);
            ",
    );
    run_js(&mut h, "__inca_native__.focusNode(K)");
    let d = node(&h, "D");
    h.hover(d);
    targets_log(&mut h);

    h.keystrokes("s");

    assert_eq!(
        targets_log(&mut h),
        "mouseover@new/new/P,mouseover@P/new/P,mouseenter@new/new/P"
    );
}

#[gpui::test]
fn the_hover_events_of_a_move_precede_its_mousemove(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, HOVER_ORDER);
    let c = node(&h, "C");
    listen(&mut h, "C", "mousemove", 0);
    targets_log(&mut h);

    h.hover(c);

    assert!(targets_log(&mut h).ends_with("mouseenter@C/C/root,mousemove@C/C/none"));
}

#[gpui::test]
fn moving_to_a_sibling_fires_out_leave_over_enter_with_related_targets(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, HOVER_ORDER);
    let (c, d) = (node(&h, "C"), node(&h, "D"));
    h.hover(c);
    targets_log(&mut h);

    h.hover(d);

    assert_eq!(
        targets_log(&mut h),
        concat!(
            "mouseout@C/C/D,mouseout@B/C/D,mouseout@A/C/D,mouseleave@C/C/D,",
            "mouseover@D/D/C,mouseover@B/D/C,mouseover@A/D/C,mouseenter@D/D/C"
        )
    );
}

#[gpui::test]
fn leaving_everything_fires_out_then_leave_from_the_inside_out(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, HOVER_ORDER);
    let d = node(&h, "D");
    h.hover(d);
    targets_log(&mut h);

    h.unhover();

    assert_eq!(
        targets_log(&mut h),
        concat!(
            "mouseout@D/D/none,mouseout@B/D/none,mouseout@A/D/none,",
            "mouseleave@D/D/none,mouseleave@B/B/none,mouseleave@A/A/none"
        )
    );
}

#[gpui::test]
fn a_click_between_two_nodes_targets_their_common_ancestor(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    listen(&mut h, "frame", "click", 0);
    let inner = node(&h, "inner");
    h.hover(inner);
    let (down, up) = (point(px(15.0), px(15.0)), point(px(60.0), px(40.0)));

    h.cx.simulate_mouse_down(down, gpui::MouseButton::Left, gpui::Modifiers::none());
    h.cx.simulate_mouse_up(up, gpui::MouseButton::Left, gpui::Modifiers::none());
    h.settle();

    assert_eq!(targets_log(&mut h), "click@frame/mid/3");
}

/// Callback 1 logs `type:detail:button:buttons:clientX:clientY`.
const POINTER_LOG: &str = r"
    cb[1] = (e) => {
        globalThis.log.push([e.type, e.detail, e.button, e.buttons, e.clientX, e.clientY].join(':'));
    };
    ";

#[gpui::test]
fn a_mouse_click_carries_detail_button_buttons_and_coordinates(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(&mut h, POINTER_LOG);
    listen(&mut h, "frame", "click", 1);
    let inner = node(&h, "inner");
    h.hover(inner);

    press(
        &mut h,
        point(px(15.0), px(15.0)),
        gpui::MouseButton::Left,
        1,
    );

    assert_eq!(targets_log(&mut h), "click:1:0:0:15:15");
}

#[gpui::test]
fn a_right_press_fires_contextmenu_after_mousedown_and_no_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(&mut h, POINTER_LOG);
    listen(&mut h, "inner", "mousedown", 0);
    listen(&mut h, "frame", "mousedown", 0);
    listen(&mut h, "frame", "contextmenu", 1);
    listen(&mut h, "inner", "contextmenu mouseup click", 0);
    listen(&mut h, "frame", "auxclick", 0);
    let inner = node(&h, "inner");
    h.hover(inner);

    press(
        &mut h,
        point(px(15.0), px(15.0)),
        gpui::MouseButton::Right,
        1,
    );

    assert_eq!(
        targets_log(&mut h),
        concat!(
            "mousedown@inner/inner/2,mousedown@frame/inner/3,contextmenu@inner/inner/2,",
            "contextmenu:0:2:2:15:15,mouseup@inner/inner/2,",
            "auxclick@frame/inner/3"
        )
    );
}

#[gpui::test]
fn a_stopped_mousedown_still_fires_contextmenu_and_a_middle_press_does_not(
    cx: &mut TestAppContext,
) {
    let mut h = load_listening(cx, TARGETS);
    run_js(&mut h, "cb[2] = (e) => { e.stopPropagation(); };");
    listen(&mut h, "inner", "mousedown", 0);
    listen(&mut h, "inner", "mousedown", 2);
    listen(&mut h, "frame", "mousedown contextmenu click", 0);
    let inner = node(&h, "inner");
    h.hover(inner);

    press(
        &mut h,
        point(px(15.0), px(15.0)),
        gpui::MouseButton::Right,
        1,
    );
    let right = targets_log(&mut h);
    press(
        &mut h,
        point(px(15.0), px(15.0)),
        gpui::MouseButton::Middle,
        1,
    );

    assert_eq!(right, "mousedown@inner/inner/2,contextmenu@frame/inner/3");
    assert_eq!(targets_log(&mut h), "mousedown@inner/inner/2");
}

#[gpui::test]
fn contextmenu_and_auxclick_reach_a_disabled_buttons_ancestors(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'disabled', true);",
    );
    listen(&mut h, "frame", "mousedown contextmenu auxclick", 0);
    let button = node(&h, "box");
    h.hover(button);

    press(
        &mut h,
        point(px(10.0), px(10.0)),
        gpui::MouseButton::Right,
        1,
    );

    assert_eq!(log(&h), "contextmenu@frame/box,auxclick@frame/box");
}

#[gpui::test]
fn auxclick_carries_the_released_button_and_coordinates(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(&mut h, POINTER_LOG);
    listen(&mut h, "frame", "auxclick", 1);
    let inner = node(&h, "inner");
    h.hover(inner);
    let mut got = Vec::new();

    for button in [
        gpui::MouseButton::Left,
        gpui::MouseButton::Right,
        gpui::MouseButton::Middle,
    ] {
        press(&mut h, point(px(15.0), px(15.0)), button, 1);
        got.push(targets_log(&mut h));
    }

    assert_eq!(got, ["", "auxclick:1:2:0:15:15", "auxclick:1:1:0:15:15"]);
}

#[gpui::test]
fn a_double_click_fires_dblclick_after_the_second_click(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(&mut h, POINTER_LOG);
    listen(&mut h, "frame", "click dblclick", 1);
    listen(&mut h, "mid", "dblclick", 0);
    let inner = node(&h, "inner");
    h.hover(inner);

    for count in 1..=3 {
        press(
            &mut h,
            point(px(15.0), px(15.0)),
            gpui::MouseButton::Left,
            count,
        );
    }

    assert_eq!(
        targets_log(&mut h),
        concat!(
            "click:1:0:0:15:15,click:2:0:0:15:15,dblclick@mid/inner/3,dblclick:2:0:0:15:15,",
            "click:3:0:0:15:15"
        )
    );
}

#[gpui::test]
fn a_mouse_click_reaches_a_button_and_its_ancestor(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 0);
    listen(&mut h, "frame", "click", 0);
    let button = node(&h, "box");

    h.click(button);

    assert_eq!(log(&h), "click@box/box,click@frame/box");
}

#[gpui::test]
fn a_press_ends_when_the_window_deactivates(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, BUTTON);
    listen(&mut h, "box", "click", 0);
    let button = node(&h, "box");
    let at = h.center_of(button);
    let none = Modifiers::none();
    h.cx.update(|window, _| window.activate_window());
    h.settle();

    h.cx.simulate_mouse_down(at, gpui::MouseButton::Left, none);
    h.cx.deactivate_window();
    h.settle();
    h.cx.simulate_mouse_up(at, gpui::MouseButton::Left, none);
    h.settle();
    assert_eq!(log(&h), "");

    h.click(button);
    assert!(log(&h).contains("click@box/box"), "{}", log(&h));
}

fn press(h: &mut Harness, at: Point<Pixels>, button: gpui::MouseButton, count: usize) {
    h.cx.simulate_event(gpui::MouseDownEvent {
        position: at,
        button,
        click_count: count,
        ..Default::default()
    });
    h.cx.simulate_event(gpui::MouseUpEvent {
        position: at,
        button,
        click_count: count,
        ..Default::default()
    });
    h.settle();
}
