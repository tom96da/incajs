// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use crate::test_support::callback_ids;
/// `inca-gpui` only ever produces a `button` index in `0..=4`, but
/// `update_held_buttons` takes a plain `u8`. A value outside that
/// range must not panic, only fail to set any bit.
#[test]
fn an_out_of_range_button_does_not_panic() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    assert_eq!(dispatcher.update_held_buttons("mousedown", 200), 0);
    assert_eq!(dispatcher.state.held_buttons.get(), 0);
}

#[test]
fn a_right_mousedown_sets_the_right_buttons_bit() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    assert_eq!(dispatcher.update_held_buttons("mousedown", 2), 0b0_0010);
}

#[test]
fn a_middle_mousedown_sets_the_middle_buttons_bit() {
    let (dispatcher, _host, _reported) = dispatcher_with_engine();
    assert_eq!(dispatcher.update_held_buttons("mousedown", 1), 0b0_0100);
}

#[gpui::test]
fn base_fields_follow_the_spec_rows_and_prevent_default_needs_cancelable(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
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
        ("wheel", true, true, true),
        ("scroll", false, false, false),
        ("keydown", true, true, true),
        ("keyup", true, true, true),
        ("focus", false, false, true),
        ("blur", false, false, true),
        ("focusin", true, false, true),
        ("focusout", true, false, true),
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
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
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
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let parent = host.borrow_mut().tree.create_node("div").unwrap();
    let child = host.borrow_mut().tree.create_node("div").unwrap();
    host.borrow_mut().tree.append_child(parent, child).unwrap();
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
        host.borrow_mut().listeners = crate::bindings::EventListeners::default();
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

#[gpui::test]
fn time_stamp_counts_milliseconds_from_the_host_start_and_grows(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher.engine.eval::<()>(RECORDER).unwrap();
    listen(&dispatcher, node, "click", "record");
    let cx = cx.add_empty_window();
    for _ in 0..3 {
        cx.update(|window, cx| {
            dispatcher.fire(node, "click", &EventPayload::None, window, cx);
        });
    }
    let stamps: Vec<f64> = seen_rows(&dispatcher)
        .iter()
        .map(|r| r[9].as_f64().unwrap())
        .collect();
    assert!(stamps[0] > 0.0, "{stamps:?}");
    assert!(stamps.windows(2).all(|w| w[1] >= w[0]), "{stamps:?}");
}

#[gpui::test]
fn the_legacy_members_follow_the_event_state(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
    dispatcher
        .engine
        .eval::<()>("globalThis.probe = () => ''; globalThis.out = '';")
        .unwrap();
    listen(
        &dispatcher,
        node,
        "click",
        "(e) => { globalThis.out = String(globalThis.probe(e)); }",
    );
    // Each entry runs in its own dispatch and reports one string.
    let probes = [
        (
            "[e.NONE, e.CAPTURING_PHASE, e.AT_TARGET, e.BUBBLING_PHASE].join()",
            "0,1,2,3",
        ),
        ("e.srcElement === e.target", "true"),
        (
            "[e.cancelBubble, (e.cancelBubble = true, e.cancelBubble)].join()",
            "false,true",
        ),
        ("(e.cancelBubble = false, e.cancelBubble)", "false"),
        ("(e.cancelBubble = 1, e.cancelBubble)", "true"),
        ("(e.cancelBubble = '', e.cancelBubble)", "false"),
        (
            "[e.returnValue, (e.returnValue = false, e.returnValue), e.defaultPrevented].join()",
            "true,false,true",
        ),
        ("(e.returnValue = true, e.defaultPrevented)", "false"),
        ("(e.returnValue = 0, e.defaultPrevented)", "true"),
        ("(e.returnValue = 'no', e.defaultPrevented)", "false"),
        (
            "(e.preventDefault(), [e.defaultPrevented, e.returnValue].join())",
            "true,false",
        ),
        (
            "(() => { const p = e.preventDefault; try { p(); } catch (err) { return `${err instanceof TypeError}:${e.defaultPrevented}`; } return 'ran'; })()",
            "true:false",
        ),
        (
            "(e.preventDefault(), (() => { try { e.defaultPrevented = false; } catch {} return e.defaultPrevented; })())",
            "true",
        ),
    ];
    let cx = cx.add_empty_window();
    let mut mismatches = Vec::new();
    for (expr, want) in probes {
        dispatcher
            .engine
            .eval::<()>(&format!(
                "globalThis.out = ''; globalThis.probe = (e) => {expr};"
            ))
            .unwrap();
        cx.update(|window, cx| {
            dispatcher.fire(node, "click", &EventPayload::None, window, cx);
        });
        let got = dispatcher.engine.eval::<String>("globalThis.out").unwrap();
        if got != want {
            mismatches.push(format!("{expr}: {got} (want {want})"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[gpui::test]
fn a_keyboard_click_starts_at_the_innermost_node_with_a_listener(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let outer = host.borrow_mut().tree.create_node("div").unwrap();
    let parent = host.borrow_mut().tree.create_node("div").unwrap();
    let button = host.borrow_mut().tree.create_node("button").unwrap();
    host.borrow_mut().tree.append_child(outer, parent).unwrap();
    host.borrow_mut().tree.append_child(parent, button).unwrap();
    dispatcher.engine.eval::<()>(RECORDER).unwrap();
    listen(&dispatcher, parent, "click", "record");
    listen(&dispatcher, outer, "click", "record");
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(
            button,
            "click",
            &EventPayload::Mouse(MousePayload::keyboard_click(gpui::Modifiers::none())),
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

const PATHS: &str = r"
    globalThis.seen = [];
    globalThis.paths = (e) => {
        globalThis.seen.push([e.target, e.currentTarget, e.eventPhase,
            e.composedPath()]); globalThis.kept = e; };
    ";

#[gpui::test]
fn every_event_fired_at_a_node_targets_that_node(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, _, child] = tree_of_three(&host);
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
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [outer, parent, child] = tree_of_three(&host);
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
    assert_eq!(dispatcher.path_from(child), [child, parent, outer]);
    let after = dispatcher
        .engine
        .eval::<usize>("globalThis.kept.composedPath().length")
        .unwrap();
    assert_eq!(after, 0, "the path is empty once dispatch ends");
}

#[test]
fn a_target_gone_from_the_tree_falls_back_to_the_current_target() {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let [_, parent, child] = tree_of_three(&host);
    assert_eq!(dispatcher.resolve(Some(child), parent), child);
    host.borrow_mut().tree.destroy_node(child);
    assert_eq!(dispatcher.resolve(Some(child), parent), parent);
    assert_eq!(dispatcher.resolve(None, parent), parent);
}

/// Every mouse-shaped event carries the `x`/`y` aliases and a null
/// `relatedTarget`. A payload with a pointer source adds the pointer
/// fields.
#[gpui::test]
fn mouse_events_carry_the_aliases_and_the_pointer_fields_of_their_type(cx: &mut TestAppContext) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = host.borrow_mut().tree.create_node("div").unwrap();
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
    let cx = cx.add_empty_window();
    let plain = || MousePayload::at(at(7.0, 9.0), gpui::Modifiers::none());
    let pointer = |source| MousePayload {
        pointer: Some(source),
        ..plain()
    };
    let wheel = WheelPayload {
        mouse: plain(),
        delta_x: 0.0,
        delta_y: 0.0,
        delta_z: 0.0,
        delta_mode: 0,
    };
    let blank = ":::::";
    let mut cases = vec![
        (
            "click",
            EventPayload::Mouse(pointer(PointerSource::Keyboard)),
            "-1::false:1:1:0",
        ),
        ("wheel", EventPayload::Wheel(wheel), blank),
    ];
    for event in ["click", "auxclick", "contextmenu"] {
        let payload = EventPayload::Mouse(pointer(PointerSource::Mouse));
        cases.push((event, payload, "1:mouse:true:1:1:0"));
    }
    for event in [
        "mousedown",
        "mouseup",
        "mousemove",
        "mouseenter",
        "mouseleave",
        "mouseover",
        "mouseout",
        "dblclick",
    ] {
        cases.push((event, EventPayload::Mouse(plain()), blank));
    }
    let mut mismatches = Vec::new();
    for (event, payload, fields) in cases {
        if callback_ids(&host.borrow().listeners, node, event).is_empty() {
            listen(&dispatcher, node, event, "fields");
        }
        cx.update(|window, cx| dispatcher.fire(node, event, &payload, window, cx));
        let want = format!("{event}:true:true:{fields}");
        let got = seen(&dispatcher);
        if got != serde_json::json!([want]) {
            mismatches.push(format!("{event}: {got:?} != {want}"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:?}");
}
