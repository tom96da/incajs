// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of key events. A key the window receives reaches the listeners as `keydown`,
//! `keyup` and `keypress`, and what they cancel or stop decides the result reported to the
//! platform.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::support::rig::{self as wiring, Rig};
use gpui::{
    AnyWindowHandle, AppContext as _, DispatchEventResult, KeyBinding, KeyDownEvent, KeyUpEvent,
    Keystroke, PlatformInput, TestAppContext, VisualTestContext, prelude::*,
};
use inca_document::{
    EVENTS, EventListeners, EventPayload, EventTargetId, MousePayload, PointerSource,
};
use inca_gpui::{EventSink, InputLayout};
use inca_jsenv::Engine;
use inca_node::NodeId;
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::support::*;

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

/// A focusable parent wrapping a focusable child.
fn build_key_pair() -> (Rig, NodeId, NodeId) {
    let (host, parent, children) = wiring::focusable_tree(1, true);
    (host, parent, children[0])
}

/// Registers `js`, the body of a listener taking `e`, for `"keydown"` and
/// `"keyup"` on each of `nodes`.
fn listen_for_keys(engine: &Engine, nodes: [NodeId; 2], js: &str, passive: bool) {
    let callback = format!("(e) => {{ {js} }}");
    wiring::listen_each(engine, &nodes, &["keydown", "keyup"], &callback, passive);
}

fn a_keystroke() -> Keystroke {
    Keystroke {
        modifiers: gpui::Modifiers::default(),
        key: "a".to_string(),
        key_char: Some("a".to_string()),
    }
}

/// With nothing focused, a key goes to the host root, which is the event's
/// `target`, and bubbles from there.
#[gpui::test]
fn a_key_event_with_nothing_focused_targets_the_root(cx: &mut TestAppContext) {
    let (host, parent, child) = build_key_pair();
    let root = host.document.borrow().root();
    host.document
        .borrow_mut()
        .tree
        .append_child(root, parent)
        .unwrap();
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    listen_for_keys(
        &engine,
        [parent, child],
        "globalThis.seen.push(`${e.type}:${e.currentTarget}:${e.key}`);",
        false,
    );
    for event in ["keydown", "keyup"] {
        engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({root}, '{event}', (e) => {{
                    const {{ type, currentTarget, target, eventPhase, key }} = e;
                    globalThis.seen.push(
                        `${{type}}:${{currentTarget}}:${{target}}:${{eventPhase}}:${{key}}`);
                }});
                "
            ))
            .unwrap();
    }
    let window = wiring::mount_active(cx, &host, &engine, root);

    cx.simulate_keystrokes(window.into(), "a");
    cx.update_window(window.into(), |_root, window, cx| {
        window.dispatch_event(
            PlatformInput::KeyUp(KeyUpEvent {
                keystroke: a_keystroke(),
            }),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();

    assert_eq!(
        engine
            .eval::<String>("JSON.stringify(globalThis.seen)")
            .unwrap(),
        format!(r#"["keydown:{root}:{root}:2:a","keyup:{root}:{root}:2:a"]"#)
    );
}

gpui::actions!(key_dispatch_tests, [Bound]);

/// A window of the `build_key_pair` tree with its child focused. The parent
/// and the child run `js` for `keydown` and `keyup`.
type Pair = (AnyWindowHandle, Rc<Engine>, Rig, [NodeId; 2]);

fn focused_pair(cx: &mut TestAppContext, js: &str) -> Pair {
    let (host, parent, child) = build_key_pair();
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    listen_for_keys(&engine, [parent, child], js, false);
    let window = wiring::mount_active(cx, &host, &engine, parent);
    cx.update_window(window.into(), |_root, window, cx| {
        host.document.borrow_mut().focus.request_focus(child, true);
        window.draw(cx).clear(cx);
    })
    .unwrap();
    cx.run_until_parked();
    (window.into(), engine, host, [parent, child])
}

/// What the platform is told about one key press or release.
fn platform_result(
    cx: &mut TestAppContext,
    window: AnyWindowHandle,
    up: bool,
    key: &str,
) -> DispatchEventResult {
    let keystroke = Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os))
        .unwrap()
        .with_simulated_ime();
    let event = if up {
        PlatformInput::KeyUp(KeyUpEvent { keystroke })
    } else {
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        })
    };
    cx.update_window(window, |_, window, cx| window.dispatch_event(event, cx))
        .unwrap()
}

fn seen(engine: &Engine) -> String {
    engine
        .eval::<String>("JSON.stringify(globalThis.seen)")
        .unwrap()
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false, r#"["keydown","keydown"]"#)]
#[case::key_up(true, r#"["keyup","keyup"]"#)]
fn a_canceled_key_event_is_reported_to_the_platform_as_handled(
    cx: &mut TestAppContext,
    up: bool,
    expected_seen: &str,
) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type); e.preventDefault();");

    let result = platform_result(cx, window, up, "a");

    assert!(!result.propagate);
    assert!(result.default_prevented);
    assert_eq!(
        seen(&engine),
        expected_seen,
        "the listeners of both nodes ran before the key was reported handled"
    );
}

#[gpui::test]
fn a_canceled_keypress_is_reported_to_the_platform_as_handled(cx: &mut TestAppContext) {
    let (window, engine, _host, [_, child]) = focused_pair(cx, "");
    wiring::listen_each(
        &engine,
        &[child],
        &["keypress"],
        "(e) => e.preventDefault()",
        false,
    );

    let result = platform_result(cx, window, false, "a");

    assert!(!result.propagate);
    assert!(result.default_prevented);
}

#[gpui::test]
fn a_keypress_of_a_held_key_reports_repeat(cx: &mut TestAppContext) {
    let (window, engine, _host, [_, child]) = focused_pair(cx, "");
    wiring::listen_each(
        &engine,
        &[child],
        &["keypress"],
        "(e) => seen.push(e.repeat)",
        false,
    );

    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: Keystroke::parse("a").unwrap().with_simulated_ime(),
                is_held: true,
                prefer_character_input: false,
            }),
            cx,
        )
    })
    .unwrap();

    assert_eq!(seen(&engine), "[true]");
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false, r#"["keydown"]"#)]
#[case::key_up(true, r#"["keyup"]"#)]
fn a_stopped_key_event_is_reported_to_the_platform_as_unhandled(
    cx: &mut TestAppContext,
    up: bool,
    expected_seen: &str,
) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type); e.stopPropagation();");

    let result = platform_result(cx, window, up, "a");

    assert!(result.propagate && !result.default_prevented);
    assert_eq!(seen(&engine), expected_seen);
}

#[gpui::test]
fn a_key_event_nobody_listens_for_is_reported_to_the_platform_as_unhandled(
    cx: &mut TestAppContext,
) {
    let (window, engine, host, _) = focused_pair(cx, "seen.push(e.type); e.preventDefault();");
    host.document.borrow_mut().listeners = EventListeners::default();

    let result = platform_result(cx, window, false, "a");

    assert!(result.propagate && !result.default_prevented);
    assert_eq!(seen(&engine), "[]");
}

#[gpui_cases]
#[gpui::test]
#[case::key_down(false, r#"["keydown","keydown"]"#)]
#[case::key_up(true, r#"["keyup","keyup"]"#)]
fn a_passive_key_listener_leaves_the_platform_result_untouched(
    cx: &mut TestAppContext,
    up: bool,
    expected_seen: &str,
) {
    let js = "seen.push(e.type); e.preventDefault();";
    let (window, engine, host, nodes) = focused_pair(cx, js);
    host.document.borrow_mut().listeners = EventListeners::default();
    listen_for_keys(&engine, nodes, js, true);

    let result = platform_result(cx, window, up, "a");

    assert!(result.propagate && !result.default_prevented);
    assert_eq!(seen(&engine), expected_seen);
}

#[gpui::test]
fn a_bound_action_runs_and_no_listener_sees_its_key(cx: &mut TestAppContext) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.type);");
    let acted = Rc::new(RefCell::new(0));
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("b", Bound, None)]);
        let acted = Rc::clone(&acted);
        cx.on_action(move |_: &Bound, _| *acted.borrow_mut() += 1);
    });

    let result = platform_result(cx, window, false, "b");

    assert_eq!(*acted.borrow(), 1);
    assert_eq!(seen(&engine), "[]");
    assert!(!result.propagate);
}

#[gpui::test]
fn a_key_event_bubbles_from_the_focused_node_to_its_ancestor(cx: &mut TestAppContext) {
    let js = "seen.push(`${e.type}:${e.currentTarget}:${e.key}`);";
    let (window, engine, _host, [parent, child]) = focused_pair(cx, js);

    cx.simulate_keystrokes(window, "a");
    platform_result(cx, window, true, "a");

    assert_eq!(
        seen(&engine),
        format!(
            r#"["keydown:{child}:a","keydown:{parent}:a","keyup:{child}:a","keyup:{parent}:a"]"#
        )
    );
}

#[gpui::test]
fn each_key_event_takes_its_own_event_id_shared_along_its_bubble(cx: &mut TestAppContext) {
    let (window, engine, _host, _) = focused_pair(cx, "seen.push(e.eventId);");

    cx.simulate_keystrokes(window, "a a b");

    let ids: Vec<u64> = serde_json::from_str(&seen(&engine)).unwrap();
    assert_eq!(ids.len(), 6);
    for pair in ids.chunks(2) {
        assert_eq!(pair[0], pair[1]);
    }
    assert!(ids[0] >= 1 && ids[2] > ids[0] && ids[4] > ids[2], "{ids:?}");
}

/// Where `target` is `currentTarget`, records `type:key:` and the shift, control, alt and meta
/// flags as 0 or 1.
const MODIFIER_JS: &str = r"
    if (e.target === e.currentTarget) {
        seen.push(`${e.type}:${e.key}:${+e.shiftKey}${+e.ctrlKey}${+e.altKey}${+e.metaKey}`);
    }
    ";

#[gpui::test]
fn a_modifier_alone_fires_its_key_events_in_flag_order(cx: &mut TestAppContext) {
    let (window, engine, _host, _) = focused_pair(cx, MODIFIER_JS);
    let mut visual = VisualTestContext::from_window(window, cx);
    let shift_control = gpui::Modifiers {
        shift: true,
        control: true,
        ..gpui::Modifiers::none()
    };

    for modifiers in [
        gpui::Modifiers::none(),
        gpui::Modifiers::shift(),
        gpui::Modifiers::shift(),
        shift_control,
        gpui::Modifiers::alt(),
        gpui::Modifiers::command(),
        gpui::Modifiers::none(),
    ] {
        visual.simulate_modifiers_change(modifiers);
    }

    assert_eq!(
        seen(&engine),
        concat!(
            r#"["keydown:Shift:1000","keydown:Control:1100","keyup:Shift:0010","#,
            r#""keyup:Control:0010","keydown:Alt:0010","keyup:Alt:0001","#,
            r#""keydown:Meta:0001","keyup:Meta:0000"]"#
        )
    );
}

#[gpui::test]
fn a_modifier_alone_with_nothing_focused_targets_the_root(cx: &mut TestAppContext) {
    let (host, parent, child) = build_key_pair();
    let root = host.document.borrow().root();
    host.document
        .borrow_mut()
        .tree
        .append_child(root, parent)
        .unwrap();
    let engine = wiring::engine_for(&host);
    engine.eval::<()>("globalThis.seen = [];").unwrap();
    listen_for_keys(&engine, [parent, child], "seen.push('below');", false);
    engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({root}, 'keydown', (e) => {{
                seen.push(`${{e.currentTarget === {root}}}:${{e.target === {root}}}`);
            }});
            "
        ))
        .unwrap();
    let window = wiring::mount_active(cx, &host, &engine, root);
    let mut visual = VisualTestContext::from_window(window.into(), cx);

    visual.simulate_modifiers_change(gpui::Modifiers::shift());

    assert_eq!(seen(&engine), r#"["true:true"]"#);
}

#[gpui_cases]
#[gpui::test]
#[case::pressed(false)]
#[case::held(true)]
fn a_key_event_reports_its_key_fields(cx: &mut TestAppContext, held: bool) {
    let (window, engine, _host, _) = focused_pair(
        cx,
        r"
        if (e.target === e.currentTarget) {
            seen.push([e.type, e.key, e.location, e.isComposing, e.repeat, +e.shiftKey, +e.ctrlKey,
                +e.altKey, +e.metaKey].join(':'));
        }
        ",
    );

    cx.update_window(window, |_, window, cx| {
        window.dispatch_event(
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke: Keystroke::parse("shift-a").unwrap(),
                is_held: held,
                prefer_character_input: false,
            }),
            cx,
        )
    })
    .unwrap();
    platform_result(cx, window, true, "ctrl-b");

    assert_eq!(
        seen(&engine),
        format!(r#"["keydown:A:0:false:{held}:1:0:0:0","keyup:b:0:false:false:0:1:0:0"]"#)
    );
}
