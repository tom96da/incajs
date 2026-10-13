// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[gpui::test]
fn a_listener_on_an_ancestor_reads_the_deepest_node_as_target(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    listen(
        &mut h,
        "frame",
        "mousemove mousedown mouseup click wheel",
        0,
    );
    let inner = node(&h, "inner");

    h.hover(inner);
    h.click(inner);
    h.scroll(inner, 0.0, -5.0);

    assert_eq!(
        targets_log(&mut h),
        concat!(
            "mousemove@frame/inner/3,mousedown@frame/inner/3,mouseup@frame/inner/3,",
            "click@frame/inner/3,wheel@frame/inner/3"
        )
    );
}

#[gpui::test]
fn stopping_a_click_leaves_the_ancestors_mouseup(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(&mut h, "cb[1] = (e) => { cb[0](e); e.stopPropagation(); };");
    listen(&mut h, "frame", "mouseup click", 0);
    listen(&mut h, "inner", "click", 1);
    let inner = node(&h, "inner");

    h.click(inner);

    assert_eq!(
        targets_log(&mut h),
        "mouseup@frame/inner/3,click@inner/inner/2"
    );
}

#[gpui::test]
fn a_text_child_reports_its_container_as_target(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    listen(&mut h, "frame", "click", 0);
    let label = node(&h, "label");

    h.click(label);

    assert_eq!(targets_log(&mut h), "click@frame/mid/3");
}

#[gpui::test]
fn the_phase_is_2_where_current_target_is_target(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    listen(&mut h, "inner", "click", 0);
    listen(&mut h, "mid", "click", 0);
    listen(&mut h, "frame", "click", 0);
    let inner = node(&h, "inner");
    let mid = node(&h, "mid");

    h.click(inner);
    assert_eq!(
        targets_log(&mut h),
        "click@inner/inner/2,click@mid/inner/3,click@frame/inner/3"
    );
    h.click(mid);
    assert_eq!(targets_log(&mut h), "click@mid/mid/2,click@frame/mid/3");
}

#[gpui::test]
fn a_key_event_targets_the_focused_node(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);",
    );
    listen(&mut h, "frame", "keydown keyup focus blur", 0);
    listen(&mut h, "inner", "focus blur", 0);

    h.keystrokes("a");
    h.key_up("a");
    run_js(&mut h, "__inca_native__.blurNode(inner);");

    assert_eq!(
        targets_log(&mut h),
        "keydown@frame/inner/3,keyup@frame/inner/3,blur@inner/inner/2"
    );
}

#[gpui::test]
fn an_unfocused_key_event_targets_the_root_and_reads_location_composing_and_modifiers(
    cx: &mut TestAppContext,
) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        r"
            const root = __inca_native__.rootNodeId();
            cb[1] = (e) => {
                globalThis.log.push([e.type, e.currentTarget === root,
                    e.target === root, e.eventPhase, e.key, e.location,
                    e.isComposing, e.repeat, e.getModifierState('Shift'),
                    e.getModifierState('Control'), e.getModifierState('Alt'),
                    e.getModifierState('Meta'), e.getModifierState('Accel'),
                    e.getModifierState('CapsLock')].join(':'));
            };
            listen(root, 'keydown', 1);
            listen(root, 'keyup', 1);
            listen(frame, 'keydown', 0);
            ",
    );

    h.keystrokes("shift-a");
    h.key_up("ctrl-b");

    // Accel is Meta on macOS and Control elsewhere.
    let accel = !cfg!(target_os = "macos");
    assert_eq!(
        targets_log(&mut h),
        format!(
            concat!(
                "keydown:true:true:2:A:0:false:false:true:false:false:false:false:false,",
                "keyup:true:true:2:b:0:false:false:false:true:false:false:{accel}:false"
            ),
            accel = accel
        )
    );
}

/// Callback 1 logs `type:key:` and the four modifier flags as 0 or 1.
const MODIFIER_LOG: &str = r"
    cb[1] = (e) => {
    globalThis.log.push(`${e.type}:${e.key}:${+e.shiftKey}${+e.ctrlKey}${+e.altKey}${+e.metaKey}`); };
    ";

#[gpui::test]
fn a_modifier_alone_fires_keydown_and_keyup_in_flag_order(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        "__inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);",
    );
    run_js(&mut h, MODIFIER_LOG);
    listen(&mut h, "frame", "keydown keyup", 1);
    let shift_control = Modifiers {
        shift: true,
        control: true,
        ..Modifiers::none()
    };

    for modifiers in [
        Modifiers::none(),
        Modifiers::shift(),
        Modifiers::shift(),
        shift_control,
        Modifiers::alt(),
        Modifiers::command(),
        Modifiers::none(),
    ] {
        h.cx.simulate_modifiers_change(modifiers);
    }
    h.settle();

    assert_eq!(
        log(&h),
        concat!(
            "keydown:Shift:1000,keydown:Control:1100,keyup:Shift:0010,keyup:Control:0010,",
            "keydown:Alt:0010,keyup:Alt:0001,keydown:Meta:0001,keyup:Meta:0000"
        )
    );
}

#[gpui::test]
fn a_modifier_alone_with_nothing_focused_reaches_the_root(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        r"
            const root = __inca_native__.rootNodeId();
            cb[1] = (e) => {
                globalThis.log.push(`${e.type}:${e.currentTarget === root}:${e.target === root}`);
            };
            listen(root, 'keydown', 1);
            ",
    );

    h.cx.simulate_modifiers_change(Modifiers::shift());
    h.settle();

    assert_eq!(log(&h), "keydown:true:true");
}

#[gpui::test]
fn a_stopped_modifier_keydown_skips_the_ancestors(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        r"
        __inca_native__.setAttribute(inner, 'tabindex', '0'); __inca_native__.focusNode(inner);
        cb[2] = (e) => { e.stopPropagation(); };
        ",
    );
    listen(&mut h, "inner", "keydown", 2);
    listen(&mut h, "frame", "keydown", 0);

    h.cx.simulate_modifiers_change(Modifiers::shift());
    h.settle();

    assert_eq!(log(&h), "");
}

#[gpui::test]
fn screen_coordinates_are_the_window_origin_plus_the_client_position(cx: &mut TestAppContext) {
    let mut h = load_listening(cx, TARGETS);
    run_js(
        &mut h,
        r"
        cb[1] = (e) => {
        globalThis.log.push(`${e.screenX - e.clientX}:${e.screenY - e.clientY}`); };
        ",
    );
    listen(&mut h, "frame", "click mousemove", 1);
    let origin = h.cx.update(|window, _| window.bounds().origin);
    let inner = node(&h, "inner");

    h.click(inner);

    assert!(origin.x > px(0.0) && origin.y > px(0.0), "{origin:?}");
    let expected = format!("{}:{}", f32::from(origin.x), f32::from(origin.y));
    assert!(log(&h).ends_with(&expected), "{} vs {expected}", log(&h));
}
