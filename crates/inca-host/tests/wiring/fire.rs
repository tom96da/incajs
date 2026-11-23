// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of an event the dispatch fires.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_document::{EventPayload, EventTargetId};
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::support::*;
use crate::support::Wired;
use inca_document::{EVENTS, MousePayload, PointerSource};
use inca_gpui::{EventSink, InputLayout};
use inca_node::NodeId;

#[gpui::test]
fn only_a_listener_the_event_reaches_enters_the_engine(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&cells);
    let cx = cx.add_empty_window();
    let mut drained = |dispatcher: &Wired| {
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

    listen_with(&dispatcher, parent, "focus", (true, false), "() => {}");
    assert!(drained(&dispatcher), "capture listener on an ancestor");
}

#[gpui::test]
fn an_event_nobody_keeps_is_collected_with_its_state(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
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
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
    listen(&dispatcher, node, "click", "() => {}");
    dispatcher
        .engine
        .eval::<()>("globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);")
        .unwrap();
    let cx = cx.add_empty_window();
    let key = gpui::Keystroke::parse("a").unwrap();
    let ran = |d: &Wired| d.engine.eval::<bool>("globalThis.ran").unwrap();

    cx.update(|window, cx| {
        let sink: &dyn EventSink = dispatcher.dispatcher();
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
            cells.document.borrow().root()
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
            let sink: &dyn EventSink = dispatcher.dispatcher();
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
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let [_, _, child] = tree_of_three(&cells);
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
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
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
