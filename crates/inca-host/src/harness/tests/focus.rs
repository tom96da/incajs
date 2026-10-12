// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

/// A 200x200 frame holding a 100x50 `box` at its top-left corner. `box`
/// records its `focus` and `blur` events in `globalThis.log`. The frame's
/// centre lies outside `box`.
const FOCUSABLE: &str = r"
        const n = __inca_native__;
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        const box = n.createNode('div');
        n.setStyle(box, 'width', 100);
        n.setStyle(box, 'height', 50);
        n.appendChild(frame, box);
        globalThis.frame = frame;
        globalThis.box = box;
        globalThis.log = [];
        globalThis.cb = {
            0: (e) => { globalThis.log.push(e.type); },
            1: (e) => { e.preventDefault(); },
        };
        listen(box, 'focus', 0);
        listen(box, 'blur', 0);
    ";

/// Makes `box` focusable and focuses it.
const FOCUS_BOX: &str =
    "__inca_native__.setAttribute(box, 'tabindex', '0'); __inca_native__.focusNode(box);";

#[gpui::test]
fn a_click_focuses_a_tabindex_node(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'tabindex', '0');",
    );
    let boxed = node(&h, "box");
    h.click(boxed);
    assert_eq!(log(&h), "focus");
}

#[gpui::test]
fn default_prevented_carries_over_to_the_ancestors_listener(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        "cb[2] = (e) => { const was = e.defaultPrevented; e.preventDefault(); \
            globalThis.log.push([e.type, was, e.defaultPrevented].join('/')); }; \
         listen(box, 'click', 2); \
         listen(frame, 'click', 2);",
    );
    let boxed = node(&h, "box");
    h.click(boxed);
    assert_eq!(log(&h), "click/false/true,click/true/true");
}

#[gpui::test]
fn focusin_and_focusout_bubble(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        "cb[2] = (e) => { globalThis.log.push([e.type, \
            e.target === box ? 'box' : 'other', e.eventPhase].join('/')); }; \
         for (const t of [box, frame]) { \
            listen(t, 'focusin', 2); \
            listen(t, 'focusout', 2); } \
         __inca_native__.setAttribute(box, 'tabindex', '0'); __inca_native__.focusNode(box);",
    );
    let frame = node(&h, "frame");
    h.click(frame);
    assert_eq!(
        log(&h),
        "focus,focusin/box/2,focusin/box/3,\
         blur,focusout/box/2,focusout/box/3"
    );
}

#[gpui::test]
fn a_negative_tabindex_takes_click_and_focus(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'tabindex', '-1');",
    );
    let boxed = node(&h, "box");
    h.click(boxed);
    assert_eq!(log(&h), "focus");
    let frame = node(&h, "frame");
    h.click(frame);
    run_js(&mut h, "__inca_native__.focusNode(box);");
    assert_eq!(log(&h), "focus,blur,focus");
}

#[gpui::test]
fn a_click_on_an_empty_area_blurs_the_focused_node(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(&mut h, FOCUS_BOX);
    let frame = node(&h, "frame");
    h.click(frame);
    assert_eq!(log(&h), "focus,blur");
}

#[gpui::test]
fn a_prevented_mousedown_keeps_the_focused_node(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        &format!("{FOCUS_BOX} listen(frame, 'mousedown', 1);"),
    );
    let frame = node(&h, "frame");
    h.click(frame);
    assert_eq!(log(&h), "focus");
}

#[gpui::test]
fn a_stopped_mousedown_still_focuses_the_node(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'tabindex', '0'); \
         cb[2] = (e) => { e.stopPropagation(); }; \
         listen(box, 'mousedown', 2);",
    );
    let boxed = node(&h, "box");
    h.click(boxed);
    assert_eq!(log(&h), "focus");
}

#[gpui::test]
fn a_prevented_mousedown_on_the_root_keeps_the_focused_node(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        &format!("{FOCUS_BOX} listen(__inca_native__.rootNodeId(), 'mousedown', 1);"),
    );
    let frame = node(&h, "frame");
    h.click(frame);
    assert_eq!(log(&h), "focus");
}

#[gpui::test]
fn a_passive_mousedown_listener_still_lets_the_node_take_focus(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'tabindex', '0'); \
         listen(box, 'mousedown', 1, false, true);",
    );
    let boxed = node(&h, "box");
    h.click(boxed);
    assert_eq!(log(&h), "focus");
}

#[gpui::test]
fn hiding_a_focused_node_blurs_it(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(&mut h, FOCUS_BOX);
    run_js(&mut h, "__inca_native__.setStyle(box, 'display', 'none');");
    assert_eq!(log(&h), "focus,blur");
}

#[gpui::test]
fn a_prevented_mousedown_does_not_focus_the_node(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, FOCUSABLE);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(box, 'tabindex', '0'); \
         listen(box, 'mousedown', 1);",
    );
    let boxed = node(&h, "box");
    h.click(boxed);
    assert_eq!(log(&h), "");
}

#[gpui::test]
fn a_node_made_inert_while_the_window_is_inactive_fires_no_second_blur(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);",
    );
    listen(&mut h, "inner", "focus blur", 0);
    h.cx.update(|window, _| window.activate_window());
    h.settle();
    run_js(&mut h, "globalThis.log.length = 0;");

    h.cx.deactivate_window();
    h.settle();
    run_js(
        &mut h,
        "__inca_native__.setAttribute(inner, 'inert', true);",
    );
    h.cx.update(|window, _| window.activate_window());
    h.settle();

    assert_eq!(log(&h), "blur@inner/inner/2");
}

#[gpui::test]
fn focusing_in_an_inactive_window_fires_focus_when_it_activates(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(inner, 'tabindex', '0');",
    );
    listen(&mut h, "inner", "focus blur", 0);
    h.cx.update(|window, _| window.activate_window());
    h.settle();
    h.cx.deactivate_window();
    h.settle();

    run_js(&mut h, "__inca_native__.focusNode(inner);");
    let held = log(&h);
    h.cx.update(|window, _| window.activate_window());
    h.settle();

    assert_eq!(
        (held.as_str(), log(&h).as_str()),
        ("", "focus@inner/inner/2")
    );
}

#[gpui::test]
fn focusing_then_blurring_in_an_inactive_window_fires_nothing(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(inner, 'tabindex', '0');",
    );
    listen(&mut h, "inner", "focus blur", 0);
    h.cx.update(|window, _| window.activate_window());
    h.settle();
    h.cx.deactivate_window();
    h.settle();

    run_js(&mut h, "__inca_native__.focusNode(inner);");
    run_js(&mut h, "__inca_native__.blurNode(inner);");
    h.cx.update(|window, _| window.activate_window());
    h.settle();

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn deactivating_the_window_with_nothing_focused_fires_nothing(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    listen(&mut h, "frame", "blur focusout", 0);
    h.cx.update(|window, _| window.activate_window());
    h.settle();

    h.cx.deactivate_window();
    h.settle();

    assert_eq!(log(&h), "");
}

fn tabs(cx: &mut TestAppContext) -> Harness {
    let mut h = load_listening(cx, TABS);
    run_js(&mut h, "watch(4);");
    h
}

#[gpui::test]
fn tab_follows_the_order_and_wraps_through_nothing(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    for _ in 0..7 {
        h.keystrokes("tab");
    }
    assert_eq!(
        log(&h),
        "focus@n1,blur@n1,focus@n2,blur@n2,focus@n3,blur@n3,focus@z1,blur@z1,focus@z2,\
         blur@z2,focus@n1"
    );
}

#[gpui::test]
fn keys_after_the_wrap_to_nothing_reach_the_root(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(
        &mut h,
        "listen(__inca_native__.rootNodeId(), 'keydown', 4);\
         for (const id of [n1, n2, n3, z1, m1]) __inca_native__.removeAttribute(id, 'tabindex');\
         __inca_native__.focusNode(z2);",
    );
    h.keystrokes("tab x tab");
    assert_eq!(
        log(&h),
        "focus@z2,keydown@z2,blur@z2,keydown@null,keydown@null,focus@z2"
    );
}

#[gpui::test]
fn shift_tab_walks_backward_and_wraps_through_nothing(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    for _ in 0..7 {
        h.keystrokes("shift-tab");
    }
    assert_eq!(
        log(&h),
        "focus@z2,blur@z2,focus@z1,blur@z1,focus@n3,blur@n3,focus@n2,blur@n2,focus@n1,\
         blur@n1,focus@z2"
    );
}

#[gpui::test]
fn tab_from_a_negative_tabindex_continues_in_tree_order(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(&mut h, "__inca_native__.focusNode(m1);");
    h.keystrokes("tab");
    run_js(&mut h, "__inca_native__.focusNode(m1);");
    h.keystrokes("shift-tab");
    assert_eq!(
        log(&h),
        "focus@m1,blur@m1,focus@n2,blur@n2,focus@m1,blur@m1,focus@z2"
    );
}

#[gpui::test]
fn focus_events_of_a_tab_move_carry_related_targets(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TABS);
    run_js(
        &mut h,
        "watch(0, ['focus', 'blur', 'focusin', 'focusout']);",
    );
    h.keystrokes("tab tab");
    assert_eq!(
        log(&h),
        "focus@n1/null,focusin@n1/null,blur@n1/n2,focusout@n1/n2,focus@n2/n1,focusin@n2/n1"
    );
}

#[gpui::test]
fn a_prevented_key_down_cancels_the_tab_move(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(&mut h, "__inca_native__.focusNode(n1);");
    run_js(
        &mut h,
        "listen(__inca_native__.rootNodeId(), 'keydown', 1);",
    );
    h.keystrokes("tab shift-tab");
    assert_eq!(log(&h), "focus@n1");
}

#[gpui::test]
fn stopping_a_key_down_keeps_the_tab_move(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(&mut h, "__inca_native__.focusNode(n1);");
    run_js(
        &mut h,
        "listen(n1, 'keydown', 4); \
         listen(n1, 'keydown', 2);",
    );
    h.keystrokes("tab");
    assert_eq!(log(&h), "focus@n1,keydown@n1,blur@n1,focus@n2");
}

#[gpui::test]
fn a_job_queued_by_a_key_down_listener_lands_before_the_tab_move(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(
        &mut h,
        "__inca_native__.focusNode(n1); \
         cb[5] = () => { Promise.resolve().then(() => \
            __inca_native__.setAttribute(n2, 'tabindex', '-1')); }; \
         listen(n1, 'keydown', 5);",
    );

    h.keystrokes("tab");

    assert_eq!(log(&h), "focus@n1,blur@n1,focus@n3");
}

#[gpui::test]
fn tab_moves_after_a_prevented_mouse_down(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(&mut h, "listen(frame, 'mousedown', 1);");
    let frame = node(&h, "frame");
    h.click(frame);
    h.keystrokes("tab");
    assert_eq!(log(&h), "focus@n1");
}

#[gpui::test]
fn tab_after_a_click_on_an_empty_area_focuses_the_first_node(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(&mut h, "__inca_native__.focusNode(n2);");
    let frame = node(&h, "frame");
    h.click(frame);
    h.keystrokes("tab");
    assert_eq!(log(&h), "focus@n2,blur@n2,focus@n1");
}

#[gpui::test]
fn control_alt_and_cmd_tab_leave_focus_alone(cx: &mut TestAppContext) {
    let mut h = tabs(cx);
    run_js(&mut h, "__inca_native__.focusNode(n1);");
    h.keystrokes("ctrl-tab alt-tab cmd-tab ctrl-shift-tab");
    assert_eq!(log(&h), "focus@n1");
}

#[gpui::test]
fn tab_skips_a_disabled_button_and_a_hidden_node_and_leaves_a_button(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TABS);
    run_js(
        &mut h,
        "const n = __inca_native__; \
         n.setAttribute(mk('bd', 1, 'button'), 'disabled', true); \
         n.setAttribute(mk('hid', 1), 'hidden', true); \
         mk('bt', null, 'button'); watch(4);",
    );
    for _ in 0..7 {
        h.keystrokes("tab");
    }
    assert_eq!(
        log(&h),
        "focus@n1,blur@n1,focus@n2,blur@n2,focus@n3,blur@n3,focus@z1,blur@z1,focus@z2,\
         blur@z2,focus@bt,blur@bt"
    );
}

#[gpui::test]
fn the_window_losing_and_regaining_focus_blurs_and_focuses_the_focused_node(
    cx: &mut TestAppContext,
) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);\
         cb[1] = (e) => { \
         globalThis.log.push(`${e.type}:${e.relatedTarget}`); };",
    );
    listen(&mut h, "inner", "focus blur focusin focusout", 1);
    h.cx.update(|window, _| window.activate_window());
    h.settle();
    run_js(&mut h, "globalThis.log.length = 0;");

    h.cx.deactivate_window();
    h.settle();
    let lost = targets_log(&mut h);
    h.cx.update(|window, _| window.activate_window());
    h.settle();

    assert_eq!(lost, "blur:null,focusout:null");
    assert_eq!(log(&h), "focus:null,focusin:null");
}

/// A `frame` of 10x10 nodes `n3`, `n1`, `z1`, `z2`, `m1` and `n2` with
/// `tabindex` 3, 1, 0, 0, -1 and 2, and `mk(name, tabindex, tag)` for more.
/// Callback 0 logs `type@target/relatedTarget`, 1 calls `preventDefault`,
/// 2 calls `stopPropagation`, 4 logs `type@target`. `watch(4)` listens to
/// `focus` and `blur` on every node made so far.
const TABS: &str = r"
        const n = __inca_native__;
        const names = {};
        const nm = (id) => names[id] ?? 'null';
        const frame = n.createNode('div');
        n.setStyle(frame, 'width', 200);
        n.setStyle(frame, 'height', 200);
        n.appendChild(n.rootNodeId(), frame);
        globalThis.frame = frame;
        globalThis.mk = (name, tabindex, tag = 'div') => {
            const id = n.createNode(tag);
            n.setStyle(id, 'width', 10);
            n.setStyle(id, 'height', 10);
            if (tabindex !== null) n.setAttribute(id, 'tabindex', String(tabindex));
            n.appendChild(frame, id);
            names[id] = name;
            globalThis[name] = id;
            return id;
        };
        for (const [name, index] of [['n3', 3], ['n1', 1], ['z1', 0], ['z2', 0], ['m1', -1], ['n2', 2]]) {
            mk(name, index);
        }
        globalThis.log = [];
        globalThis.cb = {
            0: (e) => { globalThis.log.push(`${e.type}@${nm(e.target)}/${nm(e.relatedTarget)}`); },
            1: (e) => { e.preventDefault(); },
            2: (e) => { e.stopPropagation(); },
            4: (e) => { globalThis.log.push(`${e.type}@${nm(e.target)}`); },
        };
        globalThis.watch = (callback, types = ['focus', 'blur']) => {
            for (const id of Object.keys(names)) {
                for (const type of types) listen(Number(id), type, callback);
            }
        };
    ";
