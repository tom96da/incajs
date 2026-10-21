// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use inca_test_macros::gpui_cases;
use rstest::rstest;

#[rstest]
#[case::ui_event("UIEvent", "Event")]
#[case::focus_event("FocusEvent", "UIEvent")]
#[case::mouse_event("MouseEvent", "UIEvent")]
#[case::pointer_event("PointerEvent", "MouseEvent")]
#[case::wheel_event("WheelEvent", "MouseEvent")]
#[case::keyboard_event("KeyboardEvent", "UIEvent")]
fn an_event_constructor_inherits_from_its_parent(#[case] name: &str, #[case] parent: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &format!(
            concat!(
                "[Object.getPrototypeOf({name}) === {parent}, ",
                "Object.getPrototypeOf({name}.prototype) === {parent}.prototype]"
            ),
            name = name,
            parent = parent,
        ),
    );

    assert_eq!(got, serde_json::json!([true, true]));
}

#[gpui_cases]
#[gpui::test]
#[case::mouse("mousedown", mouse_at(1.0, 1.0), "MouseEvent")]
#[case::pointer("click", pointer_click(), "PointerEvent")]
#[case::wheel("wheel", wheel_at(1.0, 1.0), "WheelEvent")]
#[case::keyboard("keydown", key_payload(gpui::Modifiers::none()), "KeyboardEvent")]
#[case::focus("focus", EventPayload::Focus { related_target: None }, "FocusEvent")]
fn a_host_event_reports_its_class_as_constructor(
    cx: &mut TestAppContext,
    event: &str,
    payload: EventPayload,
    class: &str,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        event,
        &format!("(e) => out.push(e.constructor === {class})"),
    );

    let out = fire_and_read(cx, &dispatcher, node, event, &payload);

    assert_eq!(out, serde_json::json!([true]));
}

#[rstest]
#[case::wheel_constructor("WheelEvent.DOM_DELTA_PAGE", 2)]
#[case::wheel_prototype("WheelEvent.prototype.DOM_DELTA_LINE", 1)]
#[case::keyboard_constructor("KeyboardEvent.DOM_KEY_LOCATION_NUMPAD", 3)]
#[case::keyboard_prototype("KeyboardEvent.prototype.DOM_KEY_LOCATION_RIGHT", 2)]
fn an_event_constant_reads_its_value(#[case] constant: &str, #[case] want: i32) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    assert_eq!(eval_json(&dispatcher, constant), serde_json::json!(want));
}

#[rstest]
#[case::long("new UIEvent('x', { detail: 2 ** 33 + 1 }).detail", serde_json::json!(1))]
#[case::short("new MouseEvent('x', { button: 70000 }).button", serde_json::json!(4464))]
#[case::unsigned_short("new MouseEvent('x', { buttons: -1 }).buttons", serde_json::json!(65535))]
#[case::unsigned_long(
    "new KeyboardEvent('x', { keyCode: -1 }).keyCode",
    serde_json::json!(4294967295u32)
)]
#[case::client_coordinate("new MouseEvent('x', { clientX: -1.5 }).clientX", serde_json::json!(-2))]
#[case::float(
    "new PointerEvent('x', { pressure: 0.3 }).pressure",
    serde_json::json!(0.30000001192092896)
)]
#[case::double_default("new PointerEvent('x', { width: undefined }).width", serde_json::json!(1))]
#[case::pressure_default("new PointerEvent('x').pressure", serde_json::json!(0))]
#[case::height_default("new PointerEvent('x').height", serde_json::json!(1))]
#[case::string("new KeyboardEvent('x', { key: 5 }).key", serde_json::json!("5"))]
#[case::string_default("new PointerEvent('x').pointerType", serde_json::json!(""))]
#[case::null_target(
    "new FocusEvent('x', { relatedTarget: null }).relatedTarget",
    serde_json::json!(null)
)]
fn an_init_member_converts_by_its_idl_type(
    #[case] expression: &str,
    #[case] want: serde_json::Value,
) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    assert_eq!(eval_json(&dispatcher, expression), want);
}

#[rstest]
#[case::not_finite("new MouseEvent('x', { clientX: NaN })")]
#[case::float_overflow("new PointerEvent('x', { pressure: 1e39 })")]
#[case::primitive_target("new MouseEvent('x', { relatedTarget: 5 })")]
fn a_bad_init_member_throws_a_type_error(#[case] expression: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                try {{
                    {expression};
                    return 'ran';
                }} catch (err) {{
                    return err instanceof TypeError;
                }}
            }})()
            "
        ),
    );

    assert_eq!(got, serde_json::json!(true));
}

#[test]
fn a_ui_event_reads_its_init() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new UIEvent('x', { detail: 3, which: 7 });
            return [e.detail, e.which, e.view];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([3, 0, null]));
}

#[rstest]
#[case::focus("FocusEvent", "focus")]
#[case::mouse("MouseEvent", "click")]
fn a_built_event_keeps_its_related_target(#[case] class: &str, #[case] event_type: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                const other = {{}};
                return new {class}('{event_type}', {{ relatedTarget: other }}).relatedTarget === other;
            }})()
            "
        ),
    );

    assert_eq!(got, serde_json::json!(true));
}

#[test]
fn the_which_of_a_built_mouse_event_is_its_button_plus_one() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(&dispatcher, "new MouseEvent('x', { button: -2 }).which");

    assert_eq!(got, serde_json::json!(4294967295u32));
}

#[test]
fn a_built_mouse_event_reads_button_minus_one_as_zero() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(&dispatcher, "new MouseEvent('x', { button: -1 }).button");

    assert_eq!(got, serde_json::json!(0));
}

#[test]
fn a_built_mouse_event_copies_its_client_position() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new MouseEvent('x', { clientX: 25.5, clientY: 37 });
            return [e.pageX, e.pageY, e.x, e.y, e.offsetX, e.offsetY];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([25, 37, 25, 37, 25, 37]));
}

#[test]
fn a_built_mouse_event_floors_its_screen_position() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new MouseEvent('x', { screenX: 3.7, screenY: -4.5, clientX: 1, clientY: 2 });
            return [e.screenX, e.screenY];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([3, -5]));
}

#[test]
fn a_built_mouse_event_truncates_its_movement() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new MouseEvent('x', { movementX: -2.5, movementY: NaN });
            return [e.movementX, e.movementY];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([-2, 0]));
}

#[rstest]
#[case::alt("altKey", serde_json::json!([true, false, false, false]))]
#[case::ctrl("ctrlKey", serde_json::json!([false, true, false, false]))]
#[case::meta("metaKey", serde_json::json!([false, false, true, false]))]
#[case::shift("shiftKey", serde_json::json!([false, false, false, true]))]
fn a_built_mouse_event_reads_its_modifier_keys(#[case] key: &str, #[case] want: serde_json::Value) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                const e = new MouseEvent('x', {{ '{key}': 1 }});
                return [e.altKey, e.ctrlKey, e.metaKey, e.shiftKey];
            }})()
            "
        ),
    );

    assert_eq!(got, want);
}

#[test]
fn a_pointer_event_reads_its_init() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new PointerEvent('pointerdown', {
                pointerId: 7,
                width: 2.5,
                height: 3,
                pressure: 0.25,
                pointerType: 'pen',
                isPrimary: true,
            });
            return [e.pointerId, e.width, e.height, e.pressure, e.pointerType, e.isPrimary];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([7, 2.5, 3, 0.25, "pen", true]));
}

#[test]
fn a_wheel_event_reads_its_init() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new WheelEvent('wheel', { deltaX: 0.1, deltaY: 2, deltaZ: 3, deltaMode: 1.9 });
            return [e.deltaX, e.deltaY, e.deltaZ, e.deltaMode];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([0.1, 2, 3, 1]));
}

#[test]
fn a_keyboard_event_reads_its_init() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new KeyboardEvent('keydown', {
                key: 'Enter',
                code: 'NumpadEnter',
                location: 3,
                repeat: true,
                isComposing: false,
                keyCode: 13,
                charCode: 65,
                which: 9,
                shiftKey: true,
            });
            return [
                e.key,
                e.code,
                e.location,
                e.repeat,
                e.isComposing,
                e.keyCode,
                e.charCode,
                e.which,
                e.getModifierState('Shift'),
            ];
        })()
        ",
    );

    assert_eq!(
        got,
        serde_json::json!(["Enter", "NumpadEnter", 3, true, false, 13, 65, 13, true])
    );
}

#[test]
fn a_keyboard_event_reads_its_composing_state() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new KeyboardEvent('keydown', { repeat: false, isComposing: true });
            return [e.repeat, e.isComposing];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([false, true]));
}

#[test]
fn a_built_mouse_event_shows_its_fields_to_a_listener() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        "(e) => out.push([e.clientX, e.button])",
    );

    let got = eval_json(
        &dispatcher,
        &format!(
            concat!(
                "(__inca_native__.dispatchEvent({node}, ",
                "new MouseEvent('click', {{ clientX: 5, button: 2 }})), out)"
            ),
            node = node,
        ),
    );

    assert_eq!(got, serde_json::json!([[5, 2]]));
}

#[test]
fn init_event_sets_the_type_and_the_flags() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new Event('a');
            e.initEvent(5, 1, 0);
            return [e.type, e.bubbles, e.cancelable];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(["5", true, false]));
}

#[test]
fn init_event_defaults_the_flags_to_false() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new Event('a', { bubbles: true, cancelable: true });
            e.initEvent('b');
            return [e.type, e.bubbles, e.cancelable];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(["b", false, false]));
}

#[test]
fn init_event_without_a_type_throws_a_type_error() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            try {
                new Event('a').initEvent();
                return 'ran';
            } catch (err) {
                return err instanceof TypeError;
            }
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(true));
}

#[test]
fn init_event_during_its_dispatch_changes_nothing() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "x",
        r"
        (e) => {
            e.initEvent('y', true, true);
            out.push([e.type, e.bubbles, e.cancelable]);
        }
        ",
    );

    let got = eval_json(
        &dispatcher,
        &format!("(__inca_native__.dispatchEvent({node}, new Event('x')), out)"),
    );

    assert_eq!(got, serde_json::json!([["x", false, false]]));
}

#[test]
fn init_event_sets_the_type_and_bubbling_for_the_next_dispatch() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, _, child] = tree_of_three(&host);
    listen(&dispatcher, child, "b", "() => out.push('child')");
    listen(&dispatcher, outer, "b", "() => out.push('outer')");

    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                const e = new Event('a');
                e.initEvent('b', true);
                __inca_native__.dispatchEvent({child}, e);
                return out;
            }})()
            "
        ),
    );

    assert_eq!(got, serde_json::json!(["child", "outer"]));
}

#[test]
fn init_event_clears_the_canceled_flag() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new Event('a', { cancelable: true });
            e.preventDefault();
            e.initEvent('a', false, true);
            return e.defaultPrevented;
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(false));
}

#[test]
fn init_event_clears_the_stop_flag() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new Event('a');
            e.stopPropagation();
            e.initEvent('a');
            return e.cancelBubble;
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(false));
}

#[test]
fn init_event_clears_the_immediate_stop_flag() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(&dispatcher, node, "a", "() => out.push('ran')");

    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                const e = new Event('a');
                e.stopImmediatePropagation();
                e.initEvent('a');
                __inca_native__.dispatchEvent({node}, e);
                return out;
            }})()
            "
        ),
    );

    assert_eq!(got, serde_json::json!(["ran"]));
}

#[gpui::test]
fn init_event_makes_a_kept_host_event_untrusted(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(&dispatcher, node, "click", "(e) => (globalThis.kept = e)");
    fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const before = kept.isTrusted;
            kept.initEvent('zz');
            return [before, kept.isTrusted];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!([true, false]));
}

#[test]
fn init_event_keeps_composed() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new Event('a', { composed: true });
            e.initEvent('b');
            return e.composed;
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(true));
}

#[test]
fn init_event_keeps_the_target() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();

    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                const e = new Event('a');
                __inca_native__.dispatchEvent({node}, e);
                e.initEvent('b');
                return e.target;
            }})()
            "
        ),
    );

    assert_eq!(got, serde_json::json!(node));
}

#[test]
fn init_custom_event_sets_the_detail() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const d = { n: 1 };
            const e = new CustomEvent('a', { detail: 2 });
            e.initCustomEvent('b', true, false, d);
            return [e.type, e.bubbles, e.cancelable, e.detail === d];
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(["b", true, false, true]));
}

#[test]
fn init_custom_event_reads_an_undefined_detail_as_null() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const e = new CustomEvent('a', { detail: 2 });
            e.initCustomEvent('b', false, false, undefined);
            return e.detail;
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(null));
}

#[test]
fn init_custom_event_throws_on_an_event_without_detail() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            try {
                CustomEvent.prototype.initCustomEvent.call(new Event('a'), 'b');
                return 'ran';
            } catch (err) {
                return err instanceof TypeError;
            }
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(true));
}

#[test]
fn init_custom_event_during_its_dispatch_keeps_the_detail() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "x",
        r"
        (e) => {
            e.initCustomEvent('y', true, true, 9);
            out.push(e.detail);
        }
        ",
    );

    let got = eval_json(
        &dispatcher,
        &format!(
            "(__inca_native__.dispatchEvent({node}, new CustomEvent('x', {{ detail: 1 }})), out)"
        ),
    );

    assert_eq!(got, serde_json::json!([1]));
}

#[rstest]
#[case::init_event("Event.prototype.initEvent")]
#[case::init_custom_event("CustomEvent.prototype.initCustomEvent")]
fn an_init_method_length_reads_one(#[case] method: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    assert_eq!(
        eval_json(&dispatcher, &format!("{method}.length")),
        serde_json::json!(1)
    );
}
