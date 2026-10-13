// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn click_runs_an_untrusted_click_that_bubbles_with_the_pointer_fields_of_a_key_click() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen(
        &dispatcher,
        child,
        "click",
        r"
        (e) => out.push([
            e.type, e.isTrusted, e.bubbles, e.cancelable, e.target,
            e.currentTarget, e.pointerId, e.pointerType, e.button, e.buttons, e.detail,
            e.ctrlKey, e.eventId
        ])
        ",
    );
    listen(
        &dispatcher,
        parent,
        "click",
        "(e) => out.push(['parent', e.currentTarget, e.eventPhase])",
    );
    let got = eval_json(
        &dispatcher,
        &format!("(__inca_native__.click({child}), out)"),
    );
    assert_eq!(
        got,
        serde_json::json!([
            [
                "click", false, true, true, child, child, -1, "", 0, 0, 0, false, 1
            ],
            ["parent", parent, 3]
        ])
    );
}

#[test]
fn click_on_a_disabled_button_does_nothing_and_on_an_enabled_one_runs() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let button = host.borrow_mut().tree.create_node("button").unwrap();
    listen(&dispatcher, button, "click", "() => out.push('click')");
    host.borrow_mut()
        .tree
        .set_attribute(button, "disabled", true)
        .unwrap();
    let disabled = eval_json(
        &dispatcher,
        &format!("(__inca_native__.click({button}), out.length)"),
    );
    host.borrow_mut()
        .tree
        .remove_attribute(button, "disabled")
        .unwrap();
    let enabled = eval_json(
        &dispatcher,
        &format!("(__inca_native__.click({button}), out.length)"),
    );
    assert_eq!(
        (disabled, enabled),
        (serde_json::json!(0), serde_json::json!(1))
    );
}

#[test]
fn click_inside_the_nodes_own_click_does_nothing_and_a_later_click_runs() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        &format!("() => {{ out.push('click'); __inca_native__.click({node}); }}"),
    );
    let got = eval_json(
        &dispatcher,
        &format!("(__inca_native__.click({node}), __inca_native__.click({node}), out)"),
    );
    assert_eq!(got, serde_json::json!(["click", "click"]));
    assert!(host.borrow().clicking.is_empty());
}

#[test]
fn click_on_one_node_inside_the_click_of_another_runs() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    listen(
        &dispatcher,
        parent,
        "click",
        &format!(
            r"
            (e) => {{
                out.push(e.target);
                if (e.target === {parent}) __inca_native__.click({child});
            }}
            "
        ),
    );
    listen(&dispatcher, child, "click", "() => {}");
    let got = eval_json(
        &dispatcher,
        &format!("(__inca_native__.click({parent}), out)"),
    );
    assert_eq!(got, serde_json::json!([parent, child]));
}

#[test]
fn click_on_an_unheard_or_unknown_node_takes_no_event_id_and_throws_nothing() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>(&format!(
            "__inca_native__.click({node}); __inca_native__.click(99);"
        ))
        .unwrap();
    assert_eq!(host.borrow().last_event_id, 0);
}

#[test]
fn click_reports_a_throwing_listener_through_the_pending_failures() {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        "() => { throw new Error('boom'); }",
    );
    dispatcher
        .engine
        .eval::<()>(&format!("__inca_native__.click({node});"))
        .unwrap();
    dispatcher.report_pending();
    assert_eq!(reported.borrow()[0].message(), "Error: boom");
}
