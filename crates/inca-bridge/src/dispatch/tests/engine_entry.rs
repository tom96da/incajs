// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

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

    host.borrow_mut().listeners.unregister(parent, "focus", 0);
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
    assert!(!dispatcher.tracks_pointer());

    listen(&dispatcher, node, "keydown", "() => {}");
    assert!(dispatcher.tracks_pointer());
    host.borrow_mut().listeners.unregister(node, "keydown", 0);
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
            host.borrow().root
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
fn stopping_a_handled_event_changes_nothing(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    let record = "(e) => { out.push(e.type); if (!globalThis.kept) globalThis.kept = e; }";
    listen(&dispatcher, child, "click", record);
    listen(&dispatcher, parent, "click", record);
    fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
    let state = dispatcher
        .engine
        .eval::<String>(
            r"
            kept.stopPropagation(); kept.stopImmediatePropagation(); kept.cancelBubble = true;
            String(kept.cancelBubble)
            ",
        )
        .unwrap();
    assert_eq!(state, "false");

    let again = fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);
    assert_eq!(
        again,
        serde_json::json!(["click", "click", "click", "click"])
    );
}

#[gpui::test]
fn a_script_that_patches_the_builtins_cannot_reach_or_break_event_state(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>(
            r"
            WeakMap.prototype.get = () => { throw new Error('get'); };
            WeakMap.prototype.set = () => { throw new Error('set'); };
            Reflect.apply = () => { throw new Error('apply'); };
            ",
        )
        .unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        r"
        (e) => {
        const proto = Object.getPrototypeOf(e);
        const names = (o) => [
            ...Object.getOwnPropertyNames(o),
            ...Object.getOwnPropertySymbols(o).map(String)
        ].sort();
        e.preventDefault();
        out.push(e.defaultPrevented, names(e), names(proto)); }
        ",
    );

    let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

    assert_eq!(
        out,
        serde_json::json!([
            true,
            [
                "bubbles",
                "cancelable",
                "composed",
                "currentTarget",
                "eventId",
                "eventPhase",
                "isTrusted",
                "target",
                "timeStamp",
                "type"
            ],
            [
                "AT_TARGET",
                "BUBBLING_PHASE",
                "CAPTURING_PHASE",
                "NONE",
                "cancelBubble",
                "composedPath",
                "constructor",
                "defaultPrevented",
                "preventDefault",
                "returnValue",
                "srcElement",
                "stopImmediatePropagation",
                "stopPropagation"
            ]
        ])
    );
}
