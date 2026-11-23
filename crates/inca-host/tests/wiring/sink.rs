// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of the events the window reports.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, TestAppContext};
use inca_document::{EventPayload, EventTargetId};
use inca_test_macros::gpui_cases;
use rstest::rstest;

use super::support::*;
use inca_document::{EVENTS, MousePayload, PointerSource};
use inca_gpui::{EventSink, InputLayout};
use inca_node::NodeId;

#[gpui::test]
fn base_fields_follow_the_spec_rows_and_prevent_default_needs_cancelable(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
    dispatcher.engine.eval::<()>(RECORDER).unwrap();
    let cx = cx.add_empty_window();
    let mut mismatches = Vec::new();
    // (event, bubbles, cancelable, composed)
    let rows = [
        ("click", true, true, true),
        ("dblclick", true, true, true),
        ("auxclick", true, true, true),
        ("contextmenu", true, true, true),
        ("mouseenter", false, false, false),
        ("mouseleave", false, false, false),
        ("mouseover", true, true, true),
        ("mouseout", true, true, true),
        ("mousedown", true, true, true),
        ("mouseup", true, true, true),
        ("mousemove", true, true, true),
        ("pointerdown", true, true, true),
        ("pointerup", true, true, true),
        ("pointermove", true, true, true),
        ("pointerover", true, true, true),
        ("pointerout", true, true, true),
        ("pointerenter", false, false, false),
        ("pointerleave", false, false, false),
        ("gotpointercapture", true, false, true),
        ("lostpointercapture", true, false, true),
        ("wheel", true, true, true),
        ("scroll", false, false, false),
        ("keydown", true, true, true),
        ("keypress", true, true, true),
        ("keyup", true, true, true),
        ("focus", false, false, true),
        ("blur", false, false, true),
        ("focusin", true, false, true),
        ("focusout", true, false, true),
        ("beforeinput", true, true, true),
        ("input", true, false, true),
        ("change", true, false, false),
        ("compositionstart", true, true, true),
        ("compositionupdate", true, true, true),
        ("compositionend", true, true, true),
    ];
    assert_eq!(rows.len(), EVENTS.len(), "every EVENTS row has a test row");
    for (event, bubbles, cancelable, composed) in rows {
        listen(&dispatcher, node, event, "record");
        cx.update(|window, cx| {
            dispatcher.fire(node, event, &EventPayload::None, window, cx);
        });
        let rows = seen_rows(&dispatcher);
        let want = serde_json::json!([
            event, bubbles, cancelable, composed, false, cancelable, 2, node, true
        ]);
        if rows.len() != 1 || rows[0][..9] != *want.as_array().unwrap() {
            mismatches.push(format!("{event}: {rows:?}"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[gpui::test]
fn an_unlisted_event_name_takes_all_three_flags_false(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
    dispatcher.engine.eval::<()>(RECORDER).unwrap();
    listen(&dispatcher, node, "menu:1", "record");
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(node, "menu:1", &EventPayload::None, window, cx);
    });
    let rows = seen_rows(&dispatcher);
    assert_eq!(
        rows[0][1..6],
        serde_json::json!([false, false, false, false, false])
            .as_array()
            .unwrap()[..]
    );
}

#[gpui::test]
fn fire_walks_up_only_bubbling_events_and_reports_prevention(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let parent = cells.document.borrow_mut().create_node("div").unwrap();
    let child = cells.document.borrow_mut().create_node("div").unwrap();
    cells
        .document
        .borrow_mut()
        .append_child(&*cells.window.borrow(), parent, child)
        .unwrap();
    dispatcher.engine.eval::<()>(RECORDER).unwrap();
    dispatcher
        .engine
        .eval::<()>(
            r"
                globalThis.stopper = (e) => { record(e); e.stopPropagation(); };
                ",
        )
        .unwrap();
    let cx = cx.add_empty_window();
    // (event, start, listener function, listening nodes, nodes reached, prevented)
    #[allow(clippy::type_complexity)]
    let rows: [(&str, NodeId, &str, &[NodeId], &[NodeId], bool); 8] = [
        ("click", child, "record", &[], &[], false),
        (
            "click",
            child,
            "record",
            &[child, parent],
            &[child, parent],
            true,
        ),
        ("click", child, "stopper", &[child, parent], &[child], true),
        ("contextmenu", child, "record", &[child], &[child], true),
        ("keydown", child, "record", &[child], &[child], true),
        (
            "mouseenter",
            child,
            "record",
            &[child, parent],
            &[child],
            false,
        ),
        ("focusin", child, "record", &[child], &[child], false),
        ("click", 99, "record", &[parent], &[], false),
    ];
    for (event, start, listener, listening, reached, prevented) in rows {
        drop(cells.document.borrow_mut().remove_every_listener());
        for node in listening {
            listen(&dispatcher, *node, event, listener);
        }
        let got =
            cx.update(|window, cx| dispatcher.fire(start, event, &EventPayload::None, window, cx));
        let nodes: Vec<_> = seen_rows(&dispatcher)
            .iter()
            .map(|r| r[7].clone())
            .collect();
        assert_eq!(got, prevented, "{event} from {start}: prevented");
        assert_eq!(nodes, reached, "{event} from {start}: nodes reached");
    }
}

#[gpui_cases]
#[gpui::test]
#[case::phase_constants(
    "[e.NONE, e.CAPTURING_PHASE, e.AT_TARGET, e.BUBBLING_PHASE].join()",
    "0,1,2,3"
)]
#[case::src_element_is_target("e.srcElement === e.target", "true")]
#[case::cancel_bubble_read_back(
    "[e.cancelBubble, (e.cancelBubble = true, e.cancelBubble)].join()",
    "false,true"
)]
#[case::cancel_bubble_false("(e.cancelBubble = false, e.cancelBubble)", "false")]
#[case::cancel_bubble_truthy_number("(e.cancelBubble = 1, e.cancelBubble)", "true")]
#[case::cancel_bubble_empty_string("(e.cancelBubble = '', e.cancelBubble)", "false")]
#[case::return_value_false_prevents(
    "[e.returnValue, (e.returnValue = false, e.returnValue), e.defaultPrevented].join()",
    "true,false,true"
)]
#[case::return_value_true_not_prevented("(e.returnValue = true, e.defaultPrevented)", "false")]
#[case::return_value_zero_prevents("(e.returnValue = 0, e.defaultPrevented)", "true")]
#[case::return_value_string_not_prevented("(e.returnValue = 'no', e.defaultPrevented)", "false")]
#[case::prevent_default_sets_both(
    "(e.preventDefault(), [e.defaultPrevented, e.returnValue].join())",
    "true,false"
)]
#[case::detached_prevent_default_throws(
    "(() => { const p = e.preventDefault; try { p(); } catch (err) { return `${err instanceof TypeError}:${e.defaultPrevented}`; } return 'ran'; })()",
    "true:false"
)]
#[case::default_prevented_is_read_only(
    "(e.preventDefault(), (() => { try { e.defaultPrevented = false; } catch {} return e.defaultPrevented; })())",
    "true"
)]
fn the_legacy_members_follow_the_event_state(cx: &mut TestAppContext, expr: &str, want: &str) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>(&format!(
            "globalThis.out = ''; globalThis.probe = (e) => {expr};"
        ))
        .unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        "(e) => { globalThis.out = String(globalThis.probe(e)); }",
    );
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(node, "click", &EventPayload::None, window, cx);
    });
    let got = dispatcher.engine.eval::<String>("globalThis.out").unwrap();
    assert_eq!(got, want, "{expr}");
}

#[gpui::test]
fn a_keyboard_click_starts_at_the_innermost_node_with_a_listener(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let outer = cells.document.borrow_mut().create_node("div").unwrap();
    let parent = cells.document.borrow_mut().create_node("div").unwrap();
    let button = cells.document.borrow_mut().create_node("button").unwrap();
    cells
        .document
        .borrow_mut()
        .append_child(&*cells.window.borrow(), outer, parent)
        .unwrap();
    cells
        .document
        .borrow_mut()
        .append_child(&*cells.window.borrow(), parent, button)
        .unwrap();
    dispatcher.engine.eval::<()>(RECORDER).unwrap();
    listen(&dispatcher, parent, "click", "record");
    listen(&dispatcher, outer, "click", "record");
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(
            button,
            "click",
            &EventPayload::Mouse(MousePayload::keyboard_click(
                inca_document::Modifiers::none(),
            )),
            window,
            cx,
        );
    });
    // [currentTarget, defaultPrevented before, eventPhase, isTrusted]
    let rows: Vec<_> = seen_rows(&dispatcher)
        .iter()
        .map(|r| [r[7].clone(), r[4].clone(), r[6].clone(), r[8].clone()])
        .collect();
    let want = |node: NodeId, before: bool, phase: u8| {
        [
            serde_json::json!(node),
            serde_json::json!(before),
            serde_json::json!(phase),
            serde_json::json!(true),
        ]
    };
    assert_eq!(rows, [want(parent, false, 3), want(outer, true, 3)]);
}
#[gpui::test]
fn every_event_fired_at_a_node_targets_that_node(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let [outer, _, child] = tree_of_three(&cells);
    dispatcher.engine.eval::<()>(PATHS).unwrap();
    let events = [
        "mousedown",
        "mouseup",
        "mousemove",
        "wheel",
        "click",
        "keydown",
        "keyup",
        "focus",
        "blur",
        "mouseenter",
        "mouseleave",
        "mouseover",
        "mouseout",
    ];
    for event in events {
        listen(&dispatcher, child, event, "paths");
    }
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        for event in events {
            dispatcher.fire(child, event, &EventPayload::None, window, cx);
        }
    });
    let rows = seen_rows(&dispatcher);
    assert_eq!(rows.len(), events.len());
    for row in rows {
        assert_eq!(row[0], serde_json::json!(child));
        assert_eq!(row[1], serde_json::json!(child));
        assert_eq!(row[2], serde_json::json!(2));
        assert_eq!(
            row[3].as_array().unwrap().last(),
            Some(&serde_json::json!(outer))
        );
    }
}

#[gpui::test]
fn composed_path_lists_the_target_and_its_ancestors(cx: &mut TestAppContext) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&cells);
    dispatcher.engine.eval::<()>(PATHS).unwrap();
    listen(&dispatcher, parent, "click", "paths");
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(parent, "click", &EventPayload::None, window, cx);
    });
    assert_eq!(
        seen_rows(&dispatcher)[0][3],
        serde_json::json!([parent, outer])
    );
    assert_eq!(
        cells
            .document
            .borrow()
            .tree()
            .ancestors(child)
            .collect::<Vec<_>>(),
        [child, parent, outer]
    );
    let after = dispatcher
        .engine
        .eval::<usize>("globalThis.kept.composedPath().length")
        .unwrap();
    assert_eq!(after, 0, "the path is empty once dispatch ends");
}

/// Every mouse-shaped event carries the `x`/`y` aliases and a null
/// `relatedTarget`. A payload with a pointer source adds the pointer
/// fields.
#[gpui_cases]
#[gpui::test]
#[case::click_from_keyboard("click", pointer_payload(PointerSource::Keyboard), "-1::false:1:1:0")]
#[case::wheel_is_blank("wheel", wheel_payload(), ":::::")]
#[case::click_from_mouse("click", pointer_payload(PointerSource::Mouse), "1:mouse:false:1:1:0")]
#[case::auxclick_from_mouse(
    "auxclick",
    pointer_payload(PointerSource::Mouse),
    "1:mouse:false:1:1:0"
)]
#[case::contextmenu_from_mouse(
    "contextmenu",
    pointer_payload(PointerSource::Mouse),
    "1:mouse:false:1:1:0"
)]
#[case::mousedown_without_a_source("mousedown", EventPayload::Mouse(plain_mouse()), ":::::")]
#[case::mouseup_without_a_source("mouseup", EventPayload::Mouse(plain_mouse()), ":::::")]
#[case::mousemove_without_a_source("mousemove", EventPayload::Mouse(plain_mouse()), ":::::")]
#[case::mouseenter_without_a_source("mouseenter", EventPayload::Mouse(plain_mouse()), ":::::")]
#[case::mouseleave_without_a_source("mouseleave", EventPayload::Mouse(plain_mouse()), ":::::")]
#[case::mouseover_without_a_source("mouseover", EventPayload::Mouse(plain_mouse()), ":::::")]
#[case::mouseout_without_a_source("mouseout", EventPayload::Mouse(plain_mouse()), ":::::")]
#[case::dblclick_without_a_source("dblclick", EventPayload::Mouse(plain_mouse()), ":::::")]
fn mouse_events_carry_the_aliases_and_the_pointer_fields_of_their_type(
    cx: &mut TestAppContext,
    event: &str,
    payload: EventPayload,
    fields: &str,
) {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let node = cells.document.borrow_mut().create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>(
            r"
                globalThis.seen = []; globalThis.fields = (e) => {
                globalThis.seen.push([e.type, e.x === e.clientX && e.y === e.clientY,
                e.relatedTarget === null, e.pointerId, e.pointerType, e.isPrimary,
                e.width, e.height, e.pressure].join(':')); };
                ",
        )
        .unwrap();
    listen(&dispatcher, node, event, "fields");
    let cx = cx.add_empty_window();

    cx.update(|window, cx| dispatcher.fire(node, event, &payload, window, cx));

    let want = format!("{event}:true:true:{fields}");
    assert_eq!(seen(&dispatcher), serde_json::json!([want]));
}
