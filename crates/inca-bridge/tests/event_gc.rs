// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests whose failure aborts the process: a JS value an event does not trace
//! makes the engine's drop abort. Each test runs its body in a child process
//! of this binary, so an abort fails that one test.

#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use gpui::{TestAppContext, point, px};
use inca_bridge::test_support::engine_for;
use inca_bridge::{EventDispatcher, Host};
use inca_gpui::{EventPayload, MousePayload, NodeId};
use inca_jsenv::Engine;

/// Set in the child process that runs a test's body.
const CHILD: &str = "INCA_EVENT_GC_CHILD";

/// Whether this process is the child that runs a test's body.
fn in_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

/// Runs the test `name` of this binary in a child process and fails unless
/// the child exits with success after running exactly that one test. An
/// abort in the child fails only this test.
fn run_in_child(name: &str) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{}\n{stdout}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A dispatcher over a host with three nested nodes, its engine, and the
/// nodes, outermost first. `__inca_targets__` gives a new `{ node }` object
/// per id, so only the event keeps it.
fn three_nodes() -> (EventDispatcher, Rc<Engine>, [NodeId; 3]) {
    let host = Rc::new(RefCell::new(Host::default()));
    let nodes = {
        let mut h = host.borrow_mut();
        let outer = h.tree.create_node("div").unwrap();
        let parent = h.tree.create_node("div").unwrap();
        let child = h.tree.create_node("div").unwrap();
        h.tree.append_child(outer, parent).unwrap();
        h.tree.append_child(parent, child).unwrap();
        [outer, parent, child]
    };
    let engine = engine_for(&host);
    engine
        .eval::<()>("globalThis.__inca_targets__ = (ids) => ids.map((id) => ({ node: id }));")
        .unwrap();
    (
        EventDispatcher::new(Rc::clone(&engine), host),
        engine,
        nodes,
    )
}

/// A mouse payload whose related node is `related`.
fn mouse_related_to(related: NodeId) -> EventPayload {
    EventPayload::Mouse(MousePayload {
        related_target: Some(related),
        ..MousePayload::at(point(px(1.0), px(1.0)), gpui::Modifiers::none())
    })
}

/// Registers `listener` for `event` on the innermost node, fires `event`
/// there with the middle node as its related node, runs the GC and returns
/// whether `globalThis.weak` lost its event.
fn collected_after(cx: &mut TestAppContext, event: &str, listener: &str) -> bool {
    let (dispatcher, engine, [_, parent, child]) = three_nodes();
    let payload = mouse_related_to(parent);
    engine
        .eval::<()>(&format!(
            "__inca_test_listen__({child}, '{event}', {listener});"
        ))
        .unwrap();
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(child, event, &payload, window, cx);
    });
    engine.with(|ctx| ctx.run_gc());
    engine
        .eval::<bool>("globalThis.weak.deref() === undefined")
        .unwrap()
}

#[gpui::test]
fn a_cycle_through_an_event_target_is_collected(cx: &mut TestAppContext) {
    if !in_child() {
        return run_in_child("a_cycle_through_an_event_target_is_collected");
    }
    assert!(collected_after(
        cx,
        "click",
        r"
        (e) => {
            e.target.event = e;
            globalThis.weak = new WeakRef(e);
        }
        ",
    ));
}

#[gpui::test]
fn a_cycle_through_an_event_related_target_is_collected(cx: &mut TestAppContext) {
    if !in_child() {
        return run_in_child("a_cycle_through_an_event_related_target_is_collected");
    }
    assert!(collected_after(
        cx,
        "mouseover",
        r"
        (e) => {
            e.relatedTarget.event = e;
            globalThis.weak = new WeakRef(e);
        }
        ",
    ));
}

#[test]
fn a_cycle_through_a_custom_event_detail_is_collected() {
    if !in_child() {
        return run_in_child("a_cycle_through_a_custom_event_detail_is_collected");
    }
    let (_dispatcher, engine, _) = three_nodes();
    engine
        .eval::<()>(
            r"
            (() => {
                const detail = {};
                detail.event = new CustomEvent('x', { detail });
                globalThis.weak = new WeakRef(detail.event);
            })();
            ",
        )
        .unwrap();

    engine.with(|ctx| ctx.run_gc());

    let gone = engine
        .eval::<bool>("globalThis.weak.deref() === undefined")
        .unwrap();
    assert!(gone);
}

/// The events stay reachable from globals until the engine drops. A field
/// whose value is not traced, a primitive one included, aborts the drop.
#[gpui::test]
fn events_alive_at_engine_drop_are_freed_with_it(cx: &mut TestAppContext) {
    if !in_child() {
        return run_in_child("events_alive_at_engine_drop_are_freed_with_it");
    }
    let (dispatcher, engine, [_, parent, child]) = three_nodes();
    engine
        .eval::<()>(&format!(
            r"
            __inca_test_listen__({child}, 'mouseover', (e) => {{
                globalThis.keep = e;
                e.target.event = e;
                e.relatedTarget.event = e;
            }});
            globalThis.keepCustom = new CustomEvent('x', {{ detail: 1 }});
            "
        ))
        .unwrap();
    let cx = cx.add_empty_window();
    cx.update(|window, cx| {
        dispatcher.fire(child, "mouseover", &mouse_related_to(parent), window, cx);
    });

    assert!(engine.eval::<bool>("globalThis.keep.isTrusted").unwrap());
    drop(dispatcher);
    drop(engine);
}
