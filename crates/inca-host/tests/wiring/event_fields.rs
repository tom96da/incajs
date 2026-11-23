// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fields of an event that the window state fills.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_document::{EventPayload, EventTargetId};
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::support::*;

fn mouse_button_payload(button: u8) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        button,
        ..MousePayload::at(1.0, 1.0, inca_document::Modifiers::none())
    })
}
use inca_document::{EVENTS, MousePayload, PointerSource};
use inca_gpui::{EventSink, InputLayout};
use inca_node::NodeId;

/// One body: the presses set the held-button tracker that the move reads.
#[gpui_cases]
#[gpui::test]
#[case::right(&[2], 3)]
#[case::middle_before_right(&[2, 1], 2)]
fn a_move_names_its_lowest_held_button_in_which(
    cx: &mut TestAppContext,
    pressed: &[u8],
    want: u32,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.document.borrow_mut().create_node("div").unwrap();
    listen(&dispatcher, node, "mousemove", "(e) => out.push(e.which)");
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        for &button in pressed {
            dispatcher.dispatcher().pointer_input(
                "mousedown",
                node,
                &mouse_button_payload(button),
                window,
                cx,
            );
        }
        dispatcher.fire(node, "mousemove", &mouse_at(1.0, 1.0), window, cx);
    });

    assert_eq!(read_out(&dispatcher), serde_json::json!([want]));
}

/// One body: the presses set the held-button tracker that the next press and the wheel read.
#[gpui_cases]
#[gpui::test]
#[case::mousedown("mousedown", &[1], mouse_button(2), 3)]
#[case::wheel("wheel", &[2], wheel_at(1.0, 1.0), 0)]
fn which_of_a_press_or_a_wheel_skips_the_held_buttons(
    cx: &mut TestAppContext,
    event: &str,
    pressed: &[u8],
    payload: EventPayload,
    want: u32,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.document.borrow_mut().create_node("div").unwrap();
    listen(&dispatcher, node, event, "(e) => out.push(e.which)");
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        for &button in pressed {
            dispatcher.dispatcher().pointer_input(
                "mousedown",
                node,
                &mouse_button_payload(button),
                window,
                cx,
            );
        }
        dispatcher.fire(node, event, &payload, window, cx);
    });

    let out = read_out(&dispatcher);

    assert_eq!(
        out.as_array().unwrap().last().unwrap(),
        &serde_json::json!(want)
    );
}

#[gpui::test]
fn a_key_event_with_nothing_focused_starts_at_the_host_root(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let (root, document) = {
        let host = host.document.borrow();
        (host.root(), host.document())
    };
    dispatcher.engine.eval::<()>(HOOK).unwrap();
    listen(&dispatcher, root, "keydown", &note_walk("root"));
    listen(&dispatcher, document, "keydown", &note_walk("document"));
    listen_global(
        &dispatcher,
        "$window",
        "keydown",
        false,
        &note_walk("window"),
    );
    let key = gpui::Keystroke::parse("a").unwrap();
    let cx = cx.add_empty_window();

    cx.update(|window, cx| {
        let sink: &dyn EventSink = dispatcher.dispatcher();
        sink.key_input(&key, false, &EventPayload::None, window, cx);
    });

    let path = serde_json::json!([root, document, "$window"]);
    assert_eq!(
        read_out(&dispatcher),
        serde_json::json!([
            ["root", root, root, 2, path],
            ["document", root, document, 3, path],
            ["window", root, "$window", 3, path],
        ])
    );
}
