// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of the editing of an input.

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
use inca_host::wiring::Rig;
use inca_node::NodeId;

#[test]
fn a_freed_input_drops_its_state() {
    let rig = Rig::default();
    let engine = rig.engine();
    let cells = Rc::clone(&rig.cells);
    let dispatcher = rig.dispatcher(&engine, inca_jsenv::stderr_reporter());
    engine
        .eval::<()>(
            r"
                const n = __inca_native__;
                const node = n.createNode('input');
                n.appendChild(0, node);
                n.setInputValue(node, 'a');
                globalThis.node = node;
                ",
        )
        .unwrap();
    let node: inca_node::NodeId = engine.eval("globalThis.node").unwrap();
    let drawn = InputLayout {
        height: 12.0,
        ..InputLayout::default()
    };
    dispatcher.dispatcher().record_input(node, drawn.clone());
    assert_eq!(dispatcher.dispatcher().input_layout(node), drawn);
    engine
        .eval::<()>("__inca_native__.destroyNode(globalThis.node)")
        .unwrap();
    assert!(cells.document.borrow().inputs.borrow().is_empty());
    assert_eq!(
        dispatcher.dispatcher().input_layout(node),
        InputLayout::default()
    );
}

#[test]
fn click_on_a_disabled_input_runs_nothing() {
    let (dispatcher, cells, _reported) = dispatcher_with_engine();
    let input = cells
        .document
        .borrow_mut()
        .tree
        .create_node("input")
        .unwrap();
    cells
        .document
        .borrow_mut()
        .tree
        .set_attribute(input, "disabled", true)
        .unwrap();
    listen(&dispatcher, input, "click", "() => out.push('click')");
    let got = eval_json(
        &dispatcher,
        &format!("(__inca_native__.click({input}), out)"),
    );
    assert_eq!(got, serde_json::json!([]));
}

#[gpui::test]
fn typing_fires_beforeinput_then_input(cx: &mut TestAppContext) {
    let s = js_input_scene(cx, "");
    record(&s, s.input, &["keydown", "beforeinput", "input"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    assert_eq!(
        log(&s),
        [
            keydown(false, ""),
            row(
                "beforeinput",
                Some("insertText"),
                Some("x"),
                Some(false),
                true,
                ""
            ),
            row(
                "input",
                Some("insertText"),
                Some("x"),
                Some(false),
                false,
                "x"
            ),
        ]
    );
}

#[gpui::test]
fn enter_after_a_beforeinput_that_destroys_the_input_leaves_no_state(cx: &mut TestAppContext) {
    let s = js_input_scene(cx, "ab");
    record(&s, s.input, &["change"]);
    s.focus(cx, s.input);
    s.key(cx, "x");
    listen(
        &s.dispatcher,
        s.input,
        "beforeinput",
        &format!("() => __inca_native__.destroyNode({})", s.input),
    );
    s.key(cx, "enter");
    assert_eq!(types(&s), Vec::<String>::new());
    assert!(
        s.cells
            .document
            .borrow()
            .inputs
            .borrow()
            .get(&s.input)
            .is_none()
    );
}

#[gpui::test]
fn a_composition_fires_start_update_and_input_pairs(cx: &mut TestAppContext) {
    let s = js_input_scene(cx, "ab");
    record(
        &s,
        s.input,
        &[
            "compositionstart",
            "compositionupdate",
            "beforeinput",
            "input",
        ],
    );
    s.request(cx, mark("k"));
    s.request(cx, mark("か"));
    let composition = |kind, data, value| row(kind, None, Some(data), None, true, value);
    let edit = |kind, data, cancelable, value| {
        row(
            kind,
            Some("insertCompositionText"),
            Some(data),
            Some(true),
            cancelable,
            value,
        )
    };
    assert_eq!(
        log(&s),
        [
            composition("compositionstart", "", "ab"),
            composition("compositionupdate", "k", "ab"),
            edit("beforeinput", "k", false, "ab"),
            edit("input", "k", false, "abk"),
            composition("compositionupdate", "か", "abk"),
            edit("beforeinput", "か", false, "abk"),
            edit("input", "か", false, "abか"),
        ]
    );
}

#[gpui::test]
fn a_composition_beforeinput_cannot_be_canceled(cx: &mut TestAppContext) {
    let s = js_input_scene(cx, "ab");
    cancel_beforeinput(&s);
    s.request(cx, mark("k"));
    assert_eq!(s.state().value(), "abk");
}

#[gpui::test]
fn a_script_that_disables_a_composing_input_ends_the_composition(cx: &mut TestAppContext) {
    let s = js_input_scene(cx, "ab");
    record(&s, s.input, &["compositionend", "blur"]);
    s.focus(cx, s.input);
    s.request(cx, mark("k"));
    eval_input(&s, "__inca_native__.setAttribute(INPUT, 'disabled', true);");
    s.draw(cx);
    cx.run_until_parked();
    assert_eq!(types(&s), ["compositionend", "blur"]);
    assert_eq!(edited_text(&s), ("abk".to_owned(), None));
}
