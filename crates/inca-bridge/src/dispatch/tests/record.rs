// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use inca_test_macros::gpui_cases;
use rstest::rstest;

fn pointer_click() -> EventPayload {
    EventPayload::Mouse(MousePayload {
        pointer: Some(PointerSource::Mouse),
        ..MousePayload::at(at(1.0, 1.0), gpui::Modifiers::none())
    })
}

#[gpui::test]
fn a_host_event_owns_only_is_trusted(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        "(e) => out.push(Reflect.ownKeys(e))",
    );

    let out = fire_and_read(cx, &dispatcher, node, "click", &pointer_click());

    assert_eq!(out, serde_json::json!([["isTrusted"]]));
}

#[test]
fn a_custom_event_owns_only_is_trusted() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        "Reflect.ownKeys(new CustomEvent('x', { detail: 1 }))",
    );

    assert_eq!(got, serde_json::json!(["isTrusted"]));
}

#[test]
fn the_is_trusted_descriptor_reads_enumerable_only() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const d = Object.getOwnPropertyDescriptor(new Event('x'), 'isTrusted');
            return [typeof d.get, typeof d.set, d.enumerable, d.configurable];
        })()
        ",
    );

    assert_eq!(
        got,
        serde_json::json!(["function", "undefined", true, false])
    );
}

#[gpui::test]
fn every_event_shares_one_is_trusted_getter(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(&dispatcher, node, "click", "(e) => (globalThis.kept = e)");
    fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    let shared = dispatcher
        .engine
        .eval::<bool>(
            r"
            const getterOf = (e) => Object.getOwnPropertyDescriptor(e, 'isTrusted').get;
            getterOf(kept) === getterOf(new Event('x'));
            ",
        )
        .unwrap();

    assert!(shared);
}

#[gpui_cases]
#[gpui::test]
#[case::mouse("mousedown", mouse_at(1.0, 1.0), "clientX")]
#[case::mouse_related_target("mouseover", mouse_at(1.0, 1.0), "relatedTarget")]
#[case::pointer("click", pointer_click(), "pointerId")]
#[case::wheel("wheel", wheel_at(1.0, 1.0), "deltaX")]
#[case::keyboard("keydown", key_payload(gpui::Modifiers::none()), "key")]
#[case::focus("focus", EventPayload::Focus { related_target: None }, "relatedTarget")]
fn a_field_is_a_getter_on_the_prototype_of_its_kind(
    cx: &mut TestAppContext,
    event: &str,
    payload: EventPayload,
    key: &str,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        event,
        &r"
        (e) => {
            const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e), 'KEY');
            out.push([typeof d.get, typeof d.set, d.enumerable, d.configurable]);
        }
        "
        .replace("KEY", key),
    );

    let out = fire_and_read(cx, &dispatcher, node, event, &payload);

    assert_eq!(
        out,
        serde_json::json!([["function", "undefined", true, true]])
    );
}

#[gpui_cases]
#[gpui::test]
#[case::mouse("mousedown", mouse_at(1.0, 1.0), "parent === Event.prototype")]
#[case::pointer("click", pointer_click(), "Object.hasOwn(parent, 'clientX')")]
#[case::wheel("wheel", wheel_at(1.0, 1.0), "Object.hasOwn(parent, 'clientX')")]
#[case::keyboard(
    "keydown",
    key_payload(gpui::Modifiers::none()),
    "parent === Event.prototype"
)]
#[case::focus(
    "focus",
    EventPayload::Focus { related_target: None },
    "parent === Event.prototype"
)]
fn the_prototype_of_a_kind_inherits_from_its_parent(
    cx: &mut TestAppContext,
    event: &str,
    payload: EventPayload,
    check: &str,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        event,
        &r"
        (e) => {
            const parent = Object.getPrototypeOf(Object.getPrototypeOf(e));
            out.push(CHECK);
        }
        "
        .replace("CHECK", check),
    );

    let out = fire_and_read(cx, &dispatcher, node, event, &payload);

    assert_eq!(out, serde_json::json!([true]));
}

/// One body: the events of five kinds are fired once and read across.
#[gpui::test]
fn a_field_getter_throws_type_error_for_an_event_of_another_kind(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    for event in ["mousedown", "click", "wheel", "keydown", "focus"] {
        listen(
            &dispatcher,
            node,
            event,
            "(e) => ((globalThis.kept ??= {})[e.type] = e)",
        );
    }
    fire_and_read(cx, &dispatcher, node, "mousedown", &mouse_at(1.0, 1.0));
    fire_and_read(cx, &dispatcher, node, "click", &pointer_click());
    fire_and_read(cx, &dispatcher, node, "wheel", &wheel_at(1.0, 1.0));
    fire_and_read(
        cx,
        &dispatcher,
        node,
        "keydown",
        &key_payload(gpui::Modifiers::none()),
    );
    let focus = EventPayload::Focus {
        related_target: None,
    };
    fire_and_read(cx, &dispatcher, node, "focus", &focus);

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const getterOf = (event, key) => {
                let owner = event;
                while (!Object.hasOwn(owner, key)) {
                    owner = Object.getPrototypeOf(owner);
                }
                return Object.getOwnPropertyDescriptor(owner, key).get;
            };
            const read = (event, key, receiver) => {
                try {
                    return 'ran ' + getterOf(event, key).call(receiver);
                } catch (err) {
                    return err instanceof TypeError;
                }
            };
            return [
                read(kept.mousedown, 'clientX', kept.keydown),
                read(kept.click, 'pointerId', kept.mousedown),
                read(kept.wheel, 'deltaX', kept.mousedown),
                read(kept.keydown, 'key', kept.mousedown),
                read(kept.focus, 'relatedTarget', kept.mousedown),
                read(kept.mousedown, 'relatedTarget', kept.focus),
                read(new CustomEvent('x'), 'detail', new Event('x'))
            ];
        })()
        ",
    );

    assert_eq!(
        got,
        serde_json::json!([true, true, true, true, true, true, true])
    );
}

#[gpui::test]
fn a_mouse_field_getter_reads_a_wheel_event(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    for event in ["mousedown", "wheel"] {
        listen(
            &dispatcher,
            node,
            event,
            "(e) => ((globalThis.kept ??= {})[e.type] = e)",
        );
    }
    fire_and_read(cx, &dispatcher, node, "mousedown", &mouse_at(1.0, 1.0));
    fire_and_read(cx, &dispatcher, node, "wheel", &wheel_at(2.0, 1.0));

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const mouse = Object.getPrototypeOf(kept.mousedown);
            return Object.getOwnPropertyDescriptor(mouse, 'clientX').get.call(kept.wheel);
        })()
        ",
    );

    assert_eq!(got, serde_json::json!(2));
}

#[rstest]
#[case::event("Event")]
#[case::custom_event("CustomEvent")]
fn a_constructor_without_an_object_prototype_gives_the_kind_s_prototype(#[case] constructor: &str) {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        &format!(
            r"
            (() => {{
                function target() {{}}
                target.prototype = null;
                const e = Reflect.construct({constructor}, ['x'], target);
                return Object.getPrototypeOf(e) === {constructor}.prototype;
            }})()
            "
        ),
    );

    assert_eq!(got, serde_json::json!(true));
}

#[gpui::test]
fn the_targets_hook_receives_its_ids_past_an_inherited_index_setter(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    listen(&dispatcher, child, "click", "() => {}");
    dispatcher
        .engine
        .eval::<()>(
            r"
            Object.defineProperty(Array.prototype, 0, {
                set(value) {
                    globalThis.caught = value;
                },
                configurable: true,
            });
            globalThis.__inca_targets__ = (ids) => {
                globalThis.ids = ids;
                return ids.map((id) => id);
            };
            ",
        )
        .unwrap();
    // Not `fire_and_read`: its `JSON.stringify` would meet the setter.
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(child, "click", &EventPayload::None, window, cx);
    });

    let got = eval_json(&dispatcher, "[ids, globalThis.caught === undefined]");

    assert_eq!(got, serde_json::json!([[child, parent, outer], true]));
}

#[gpui::test]
fn composed_path_entries_ignore_an_inherited_index_setter(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
    listen(
        &dispatcher,
        parent,
        "click",
        "(e) => (globalThis.path = e.composedPath())",
    );
    dispatcher
        .engine
        .eval::<()>(
            r"
            Object.defineProperty(Array.prototype, 0, {
                set(value) {
                    globalThis.caught = value;
                },
                configurable: true,
            });
            ",
        )
        .unwrap();
    // Not `fire_and_read`: its `JSON.stringify` would meet the setter.
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(child, "click", &EventPayload::None, window, cx);
    });

    let got = eval_json(&dispatcher, "[path, globalThis.caught === undefined]");

    assert_eq!(got, serde_json::json!([[child, parent, outer], true]));
}

#[test]
fn the_event_id_descriptor_reads_configurable_only() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const d = Object.getOwnPropertyDescriptor(Event.prototype, 'eventId');
            return [typeof d.get, typeof d.set, d.enumerable, d.configurable];
        })()
        ",
    );

    assert_eq!(
        got,
        serde_json::json!(["function", "undefined", false, true])
    );
}
