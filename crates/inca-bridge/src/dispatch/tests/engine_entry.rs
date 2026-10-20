// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use inca_test_macros::gpui_cases;

#[gpui::test]
fn only_a_listener_the_event_reaches_enters_the_engine(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    let cx = cx.add_empty_window();
    let mut drained = |dispatcher: &EventDispatcher| {
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
            .unwrap();
        cx.update(|window, cx| {
            dispatcher.fire(child, "focus", &EventPayload::None, window, cx);
        });
        dispatcher.engine.eval::<bool>("globalThis.ran").unwrap()
    };

    // A bubble listener on an ancestor of a `focus` event stays out of reach.
    listen(&dispatcher, parent, "focus", "() => {}");
    assert!(!drained(&dispatcher), "bubble listener on an ancestor");

    host.borrow_mut().listeners.remove(parent, "focus", 0);
    listen_with(&dispatcher, parent, "focus", (true, false), "() => {}");
    assert!(drained(&dispatcher), "capture listener on an ancestor");
}

#[gpui::test]
fn an_event_method_throws_a_type_error_for_any_receiver_that_is_not_an_event(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        r"
        (e) => {
        const proto = Object.getPrototypeOf(e);
        const getter = (name) => Object.getOwnPropertyDescriptor(proto, name).get;
        const setter = (name) => Object.getOwnPropertyDescriptor(proto, name).set;
        const calls = [
            e.stopPropagation, e.stopImmediatePropagation, e.preventDefault,
            e.composedPath, getter('defaultPrevented'), getter('returnValue'),
            getter('cancelBubble'), (r) => setter('returnValue').call(r, false),
            (r) => setter('cancelBubble').call(r, true),
        ];
        for (const receiver of [undefined, null, 5, {}, Object.create(e), proto]) {
            for (const call of calls) {
                try { call.call(receiver, receiver); out.push('ran'); }
                catch (err) { out.push(err instanceof TypeError); }
            }
        }
        e.preventDefault();
        out.push(e.defaultPrevented); }
        ",
    );

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    let mut want = vec![serde_json::json!(true); 6 * 9];
    want.push(serde_json::json!(true));
    assert_eq!(out, serde_json::Value::Array(want));
}

#[gpui::test]
fn tracks_pointer_only_with_a_listener_or_a_focusable_node(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    let root = host.borrow().root();
    host.borrow_mut().tree.append_child(root, node).unwrap();
    assert!(!dispatcher.tracks_pointer());

    listen(&dispatcher, node, "keydown", "() => {}");
    assert!(dispatcher.tracks_pointer());
    host.borrow_mut().listeners.remove(node, "keydown", 0);
    assert!(!dispatcher.tracks_pointer());

    host.borrow_mut()
        .tree
        .set_attribute(node, "tabindex", "0")
        .unwrap();
    host.borrow_mut().focus.mark_tab_dirty(node);
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        let host = &mut *host.borrow_mut();
        let _ = host.focus.apply_pending(&host.tree, window, cx);
    });
    assert!(dispatcher.tracks_pointer());
}

#[gpui::test]
fn an_event_nobody_keeps_is_collected_with_its_state(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        "(e) => { globalThis.weak = new WeakRef(e); e.stopPropagation(); }",
    );
    fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    dispatcher.engine.with(|ctx| ctx.run_gc());

    let gone = dispatcher
        .engine
        .eval::<bool>("weak.deref() === undefined")
        .unwrap();
    assert!(gone);
}

#[gpui::test]
fn an_unlistened_pointer_or_key_input_leaves_the_engine_untouched(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(&dispatcher, node, "click", "() => {}");
    dispatcher
        .engine
        .eval::<()>("globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);")
        .unwrap();
    let cx = cx.add_empty_window();
    let key = gpui::Keystroke::parse("a").unwrap();
    let ran = |d: &EventDispatcher| d.engine.eval::<bool>("globalThis.ran").unwrap();

    cx.update(|window, cx| {
        let sink: &dyn EventSink = &dispatcher;
        sink.pointer_input("mousemove", node, &mouse_at(1.0, 1.0), window, cx);
        sink.pointer_input("wheel", node, &wheel_at(1.0, 1.0), window, cx);
        sink.pointer_input("mousedown", node, &mouse_at(1.0, 1.0), window, cx);
        sink.pointer_input("mouseup", node, &mouse_at(1.0, 1.0), window, cx);
        sink.key_input(&key, false, &EventPayload::None, window, cx);
    });
    assert!(!ran(&dispatcher), "no listener, no drain");

    for event in ["mousemove", "wheel", "keydown"] {
        // A key goes to the root when nothing is focused.
        let listener = if event == "keydown" {
            host.borrow().root()
        } else {
            node
        };
        listen(&dispatcher, listener, event, "() => {}");
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
            .unwrap();
        cx.update(|window, cx| {
            let sink: &dyn EventSink = &dispatcher;
            match event {
                "keydown" => sink.key_input(&key, false, &EventPayload::None, window, cx),
                "wheel" => sink.pointer_input(event, node, &wheel_at(1.0, 1.0), window, cx),
                _ => sink.pointer_input(event, node, &mouse_at(1.0, 1.0), window, cx),
            }
        });
        assert!(ran(&dispatcher), "a heard {event} drains");
    }
}

#[gpui::test]
fn cancel_bubble_reads_a_stop_requested_after_the_walk(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&host);
    let record = "(e) => { out.push(e.type); globalThis.kept = e; }";
    listen(&dispatcher, child, "click", record);
    fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

    let state = dispatcher
        .engine
        .eval::<String>(
            r"
            kept.stopPropagation();
            String(kept.cancelBubble)
            ",
        )
        .unwrap();

    assert_eq!(state, "true");
}

#[gpui::test]
fn a_script_that_patches_the_builtins_cannot_reach_or_break_event_state(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>(
            r"
            Object.defineProperty(Object.prototype, 'isTrusted', {
                get() {
                    return 'inherited';
                },
                set(value) {
                    globalThis.caught = value;
                },
                configurable: true,
            });
            Object.defineProperty = () => {
                throw new Error('defineProperty');
            };
            Reflect.apply = () => {
                throw new Error('apply');
            };
            ",
        )
        .unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        r"
        (e) => {
            e.preventDefault();
            out.push(e.defaultPrevented, e.isTrusted, globalThis.caught === undefined);
        }
        ",
    );

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    assert_eq!(out, serde_json::json!([true, true, true]));
}

#[gpui_cases]
#[gpui::test]
#[case::control_held(gpui::Modifiers::control())]
#[case::shift_held(gpui::Modifiers::shift())]
#[case::alt_held(gpui::Modifiers::alt())]
#[case::command_held(gpui::Modifiers::command())]
#[case::none_held(gpui::Modifiers::default())]
fn modifier_state_reports_each_held_modifier_and_no_lock(
    cx: &mut TestAppContext,
    held: gpui::Modifiers,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "keydown",
        r"(e) => out.push(['Control', 'Shift', 'Alt', 'Meta', 'Accel', 'CapsLock', 'NumLock']
            .map((name) => e.getModifierState(name)))",
    );
    let accel = |held: [bool; 4]| {
        let [control, _, _, meta] = held;
        if cfg!(target_os = "macos") {
            meta
        } else {
            control
        }
    };
    let flags = [held.control, held.shift, held.alt, held.platform];
    let out = fire_and_read(cx, &dispatcher, node, "keydown", &key_payload(held));
    let want = serde_json::json!([[
        flags[0],
        flags[1],
        flags[2],
        flags[3],
        accel(flags),
        false,
        false
    ]]);
    assert_eq!(out, want);
}

#[gpui_cases]
#[gpui::test]
#[case::mouse("click", mouse_at(1.0, 1.0), [false, true, false, true])]
#[case::wheel("wheel", wheel_at(1.0, 1.0), [false, false, false, true])]
#[case::key("keydown", key_payload(gpui::Modifiers::default()), [false, true, false, true])]
#[case::focus("focus", EventPayload::Focus { related_target: None }, [false, false, false, true])]
#[case::none("scroll", EventPayload::None, [false, false, true, true])]
fn get_modifier_state_lives_on_the_prototype_of_its_kind(
    cx: &mut TestAppContext,
    event: &str,
    payload: EventPayload,
    want: [bool; 4],
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        event,
        r"
        (e) => {
            const proto = Object.getPrototypeOf(e);
            out.push(
                Object.hasOwn(e, 'getModifierState'),
                Object.hasOwn(proto, 'getModifierState'),
                proto === Event.prototype,
                e instanceof Event
            );
        }
        ",
    );

    let out = fire_and_read(cx, &dispatcher, node, event, &payload);

    assert_eq!(out, serde_json::json!(want));
}

#[gpui_cases]
#[gpui::test]
#[case::mouse("click", mouse_at(1.0, 1.0))]
#[case::wheel("wheel", wheel_at(1.0, 1.0))]
#[case::key("keydown", key_payload(gpui::Modifiers::default()))]
#[case::focus("focus", EventPayload::Focus { related_target: None })]
fn a_host_event_reads_event_as_its_string_tag(
    cx: &mut TestAppContext,
    event: &str,
    payload: EventPayload,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        event,
        "(e) => out.push(Object.prototype.toString.call(e))",
    );

    let out = fire_and_read(cx, &dispatcher, node, event, &payload);

    assert_eq!(out, serde_json::json!(["[object Event]"]));
}

#[gpui::test]
fn a_detached_get_modifier_state_throws_type_error(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "keydown",
        r"(e) => { const f = e.getModifierState;
            for (const receiver of [undefined, {}, Object.create(e)]) {
                try { f.call(receiver, 'Control'); out.push('ran'); }
                catch (err) { out.push(err instanceof TypeError); }
            }
            try { f('Control'); out.push('ran'); }
            catch (err) { out.push(err instanceof TypeError); } }",
    );

    let out = fire_and_read(
        cx,
        &dispatcher,
        node,
        "keydown",
        &key_payload(gpui::Modifiers::control()),
    );

    assert_eq!(out, serde_json::json!([true, true, true, true]));
}

#[gpui::test]
fn a_get_modifier_state_throws_type_error_for_an_event_of_another_kind(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    for event in ["click", "keydown"] {
        listen(
            &dispatcher,
            node,
            event,
            "(e) => (globalThis.kept ??= []).push(e)",
        );
    }
    let shift = gpui::Modifiers::shift();
    let mouse = EventPayload::Mouse(MousePayload::at(at(1.0, 1.0), shift));
    fire_and_read(cx, &dispatcher, node, "click", &mouse);
    fire_and_read(cx, &dispatcher, node, "keydown", &key_payload(shift));

    let got = eval_json(
        &dispatcher,
        r"
        (() => {
            const [mouse, key] = kept;
            const call = (f, receiver) => {
                try {
                    return 'ran ' + f.call(receiver, 'Shift');
                } catch (err) {
                    return err instanceof TypeError;
                }
            };
            return [
                call(mouse.getModifierState, key),
                call(key.getModifierState, mouse),
                call(mouse.getModifierState, new Event('x')),
                call(mouse.getModifierState, mouse),
                call(key.getModifierState, key)
            ];
        })()
        ",
    );

    assert_eq!(
        got,
        serde_json::json!([true, true, true, "ran true", "ran true"])
    );
}

#[gpui_cases]
#[gpui::test]
#[case::undefined("undefined", false)]
#[case::number("1", false)]
#[case::object_with_to_string("{ toString: () => 'Control' }", true)]
fn get_modifier_state_converts_its_argument_to_a_string(
    cx: &mut TestAppContext,
    arg: &str,
    want: bool,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "keydown",
        &"(e) => out.push(e.getModifierState(ARG))".replace("ARG", arg),
    );

    let out = fire_and_read(
        cx,
        &dispatcher,
        node,
        "keydown",
        &key_payload(gpui::Modifiers::control()),
    );

    assert_eq!(out, serde_json::json!([want]));
}

#[gpui_cases]
#[gpui::test]
#[case::missing("")]
#[case::symbol("Symbol()")]
fn get_modifier_state_throws_type_error_for_a_missing_or_symbol_argument(
    cx: &mut TestAppContext,
    arg: &str,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "keydown",
        &r"
        (e) => {
            try {
                e.getModifierState(ARG);
                out.push('ran');
            } catch (err) {
                out.push(err instanceof TypeError);
            }
        }
        "
        .replace("ARG", arg),
    );

    let out = fire_and_read(
        cx,
        &dispatcher,
        node,
        "keydown",
        &key_payload(gpui::Modifiers::control()),
    );

    assert_eq!(out, serde_json::json!([true]));
}

#[gpui_cases]
#[gpui::test]
#[case::shared_method("Event.prototype", "preventDefault")]
#[case::modifier_lookup("Object.getPrototypeOf(e)", "getModifierState")]
fn an_event_method_descriptor_reads_writable_enumerable_configurable(
    cx: &mut TestAppContext,
    owner: &str,
    key: &str,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "keydown",
        &r"
        (e) => {
            const d = Object.getOwnPropertyDescriptor(OWNER, 'KEY');
            out.push([d.writable, d.enumerable, d.configurable].join());
        }
        "
        .replace("OWNER", owner)
        .replace("KEY", key),
    );

    let out = fire_and_read(
        cx,
        &dispatcher,
        node,
        "keydown",
        &key_payload(gpui::Modifiers::default()),
    );

    assert_eq!(out, serde_json::json!(["true,true,true"]));
}

#[gpui_cases]
#[gpui::test]
#[case::method("Event.prototype.preventDefault", "preventDefault")]
#[case::modifier_lookup("Object.getPrototypeOf(e).getModifierState", "getModifierState")]
#[case::getter(
    "Object.getOwnPropertyDescriptor(Event.prototype, 'srcElement').get",
    "get srcElement"
)]
#[case::setter(
    "Object.getOwnPropertyDescriptor(Event.prototype, 'returnValue').set",
    "set returnValue"
)]
fn an_event_function_carries_the_name_of_its_member(
    cx: &mut TestAppContext,
    function: &str,
    want: &str,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "keydown",
        &"(e) => out.push(FUNCTION.name)".replace("FUNCTION", function),
    );

    let out = fire_and_read(
        cx,
        &dispatcher,
        node,
        "keydown",
        &key_payload(gpui::Modifiers::default()),
    );

    assert_eq!(out, serde_json::json!([want]));
}
