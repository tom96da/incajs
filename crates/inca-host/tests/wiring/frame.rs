// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the script sees of a frame: the released-node hook, the failures of a release and the
//! focus a listener applies.

#![allow(clippy::unwrap_used, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use crate::support::Reported;
use crate::support::Wired;
use gpui::{AppContext as _, TestAppContext};
use inca_node::NodeId;
use inca_window::Cells;

use super::support::*;

const RELEASED: &str = r"
        globalThis.released = [];
        globalThis.__inca_released__ = (nodes, callbacks) => {
            globalThis.released.push([nodes, callbacks]);
        };
        ";

struct Scene {
    dispatcher: Wired,
    cells: Rc<Cells>,
    reported: Reported,
    window: gpui::WindowHandle<Frame>,
}

/// A window over a fresh cells whose engine runs `hook_js` first.
fn scene(cx: &mut TestAppContext, hook_js: &str) -> Scene {
    let (dispatcher, cells, reported) = dispatcher_with_engine();
    dispatcher.engine.eval::<()>(hook_js).unwrap();
    let frame = Rc::new(dispatcher.dispatcher().clone());
    let window = cx.add_window(|_, _| Frame(frame));
    Scene {
        dispatcher,
        cells,
        reported,
        window,
    }
}

impl Scene {
    fn js(&self, source: &str) {
        self.dispatcher.engine.eval::<()>(source).unwrap();
    }

    fn json(&self, js: &str) -> serde_json::Value {
        eval_json(&self.dispatcher, js)
    }

    /// Frame callbacks the last draw asked for.
    fn requested_frames(&self, cx: &mut TestAppContext) -> usize {
        cx.update_window(self.window.into(), |_, window, cx| {
            window.simulate_next_frame(cx)
        })
        .unwrap()
    }
}

/// A scene with an orphan that has a listener, whose callback table is frozen.
fn scene_with_frozen_callbacks(cx: &mut TestAppContext) -> Scene {
    let scene = scene(cx, RELEASED);
    scene.js(r"
            const n = __inca_native__;
            n.addEventListener(n.createNode('div'), 'click', () => {}, false, false);
            ");
    scene.dispatcher.engine.with(|ctx| {
        let table: rquickjs::Object = inca_jsenv::vault(&ctx).unwrap().get("callbacks").unwrap();
        let object: rquickjs::Object = ctx.globals().get("Object").unwrap();
        let freeze: rquickjs::Function = object.get("freeze").unwrap();
        freeze.call::<_, ()>((table,)).unwrap();
    });
    scene
}

/// A scene with an orphan whose listener names a signal, and a signal table that is gone.
fn scene_with_broken_signal_table(cx: &mut TestAppContext) -> Scene {
    let scene = scene(cx, RELEASED);
    scene.js(r"
            const n = __inca_native__;
            globalThis.signal = new AbortController().signal;
            n.addEventListener(n.createNode('div'), 'click', () => {}, false, false, signal);
            ");
    scene.dispatcher.engine.with(|ctx| {
        inca_jsenv::vault(&ctx).unwrap().set("abort", 0).unwrap();
    });
    scene
}

#[gpui::test]
fn the_hook_receives_the_freed_ids_once(cx: &mut TestAppContext) {
    let scene = scene(cx, RELEASED);
    scene.js(r"
            const n = __inca_native__;
            globalThis.top = n.createNode('div');
            globalThis.leaf = n.createNode('div');
            n.appendChild(top, leaf);
            globalThis.cb = n.addEventListener(leaf, 'click', () => {}, false, false);
            ");

    draw(cx, scene.window);
    draw(cx, scene.window);

    assert_eq!(
        scene.json("[globalThis.released, [top, leaf, cb]]"),
        serde_json::json!([[[[2, 3], [1]]], [2, 3, 1]])
    );
    assert!(scene.cells.document.borrow().tree().get(2).is_none());
}

#[gpui::test]
fn a_swept_node_unpins_the_dependent_its_listener_names(cx: &mut TestAppContext) {
    let scene = scene(cx, RELEASED);
    scene.js(r"
            const n = __inca_native__;
            globalThis.keep = new AbortController();
            (() => {
                const dependent = AbortSignal.any([keep.signal]);
                const orphan = n.createNode('div');
                n.addEventListener(orphan, 'click', () => {}, false, false, dependent);
                globalThis.w = new WeakRef(dependent);
            })();
            ");

    draw(cx, scene.window);
    for _ in 0..2 {
        scene.dispatcher.engine.with(|ctx| ctx.run_gc());
        drop(scene.dispatcher.engine.run_jobs());
    }

    assert_eq!(
        scene.json("w.deref() === undefined"),
        serde_json::json!(true)
    );
}

#[gpui::test]
fn the_hook_receives_the_content_ids_of_a_swept_template(cx: &mut TestAppContext) {
    let scene = scene(cx, RELEASED);
    scene.js(r"
            const n = __inca_native__;
            globalThis.template = n.createNode('template');
            globalThis.content = n.templateContent(template);
            ");

    draw(cx, scene.window);

    assert_eq!(
        scene.json("[globalThis.released.map(([n]) => n), [template, content]]"),
        serde_json::json!([[[2, 3]], [2, 3]])
    );
}

#[gpui::test]
fn the_released_hook_receives_its_ids_past_an_inherited_index_setter(cx: &mut TestAppContext) {
    let scene = scene(
        cx,
        "globalThis.__inca_released__ = (nodes, callbacks) => (globalThis.last = [nodes, callbacks]);",
    );
    scene.js(r"
            Object.defineProperty(Array.prototype, 0, {
                set(value) {
                    globalThis.caught = value;
                },
                configurable: true,
            });
            const n = __inca_native__;
            globalThis.top = n.createNode('div');
            globalThis.cb = n.addEventListener(top, 'click', () => {}, false, false);
            ");

    draw(cx, scene.window);

    assert_eq!(
        scene.json("[globalThis.last, [top, cb], globalThis.caught === undefined]"),
        serde_json::json!([[[2], [1]], [2, 1], true])
    );
}

#[gpui::test]
fn a_hook_that_throws_is_reported(cx: &mut TestAppContext) {
    let scene = scene(
        cx,
        "globalThis.__inca_released__ = () => { throw new Error('boom'); };",
    );
    scene.js("__inca_native__.createNode('div');");
    let node = focusable_node(&scene.dispatcher, &scene.cells);
    scene.cells.document.borrow_mut().request_focus(node, true);

    draw(cx, scene.window);

    let reported = scene.reported.borrow();
    assert_eq!(reported.len(), 1);
    assert!(reported[0].message().contains("boom"));
}

#[gpui::test]
fn a_failed_release_of_callback_entries_is_reported(cx: &mut TestAppContext) {
    let scene = scene_with_frozen_callbacks(cx);

    draw(cx, scene.window);

    assert_eq!(scene.reported.borrow().len(), 1);
}

#[gpui::test]
fn a_failed_release_of_callback_entries_still_calls_the_hook(cx: &mut TestAppContext) {
    let scene = scene_with_frozen_callbacks(cx);

    draw(cx, scene.window);

    assert_eq!(
        scene.json("globalThis.released.length"),
        serde_json::json!(1)
    );
}

#[gpui::test]
fn a_failed_settle_of_the_signal_links_is_reported(cx: &mut TestAppContext) {
    let scene = scene_with_broken_signal_table(cx);

    draw(cx, scene.window);

    let reported = scene.reported.borrow();
    assert_eq!(reported.len(), 1);
    assert!(reported[0].message().contains("Error converting"));
}

#[gpui::test]
fn a_failed_settle_of_the_signal_links_still_calls_the_hook(cx: &mut TestAppContext) {
    let scene = scene_with_broken_signal_table(cx);

    draw(cx, scene.window);

    assert_eq!(
        scene.json("globalThis.released.length"),
        serde_json::json!(1)
    );
}

#[gpui::test]
fn a_hook_that_throws_lets_the_frame_finish(cx: &mut TestAppContext) {
    let scene = scene(
        cx,
        "globalThis.__inca_released__ = () => { throw new Error('boom'); };",
    );
    scene.js("__inca_native__.createNode('div');");
    let node = focusable_node(&scene.dispatcher, &scene.cells);
    scene.cells.document.borrow_mut().request_focus(node, true);

    draw(cx, scene.window);

    assert_eq!(
        scene.json("globalThis.seen"),
        serde_json::json!([format!("focus:{node}")])
    );
    assert!(!scene.cells.document.borrow().tree().has_orphans());
}

#[gpui::test]
fn a_hook_that_creates_nodes_and_fires_events_leaves_the_next_frame_to_free_them(
    cx: &mut TestAppContext,
) {
    let scene = scene(
        cx,
        r"
            globalThis.released = [];
            globalThis.__inca_released__ = (nodes) => {
                globalThis.released.push(nodes);
                if (globalThis.released.length > 1) return;
                __inca_native__.createNode('div');
                __inca_native__.click(__inca_native__.rootNodeId());
            };
            ",
    );
    scene.js("__inca_native__.createNode('div');");

    draw(cx, scene.window);
    assert_eq!(scene.json("globalThis.released"), serde_json::json!([[2]]));
    assert!(scene.requested_frames(cx) > 0);
    assert!(scene.reported.borrow().is_empty());

    draw(cx, scene.window);
    assert_eq!(
        scene.json("globalThis.released"),
        serde_json::json!([[2], [3]])
    );
    assert_eq!(scene.requested_frames(cx), 0);
}

#[gpui::test]
fn without_a_hook_a_frame_still_frees_the_orphans(cx: &mut TestAppContext) {
    let scene = scene(cx, "");
    scene.js(r"
            globalThis.node = __inca_native__.createNode('div');
            __inca_native__.addEventListener(node, 'click', () => {}, false, false);
            ");

    draw(cx, scene.window);

    assert!(scene.cells.document.borrow().tree().get(2).is_none());
    assert!(scene.cells.document.borrow().listeners.is_empty());
}

#[gpui::test]
fn a_swept_node_releases_the_table_entries_of_its_listeners(cx: &mut TestAppContext) {
    let scene = scene(cx, "");
    scene.js(r"
            globalThis.node = __inca_native__.createNode('div');
            __inca_native__.addEventListener(node, 'click', () => {}, false, false);
            ");

    draw(cx, scene.window);

    assert!(!callback_stored(&scene.dispatcher.engine, 1));
}

#[gpui::test]
fn without_a_hook_a_frame_reports_nothing(cx: &mut TestAppContext) {
    let scene = scene(cx, "");
    scene.js(r"
            globalThis.node = __inca_native__.createNode('div');
            __inca_native__.addEventListener(node, 'click', () => {}, false, false);
            ");

    draw(cx, scene.window);

    assert!(scene.reported.borrow().is_empty());
}

#[gpui::test]
fn a_removed_focused_node_is_swept_without_a_blur(cx: &mut TestAppContext) {
    let scene = scene(cx, RELEASED);
    let node = focusable_node(&scene.dispatcher, &scene.cells);
    scene.cells.document.borrow_mut().request_focus(node, true);
    draw(cx, scene.window);
    scene.js("globalThis.seen = [];");

    scene.js(&format!(
        "__inca_native__.removeChild(__inca_native__.rootNodeId(), {node});"
    ));
    draw(cx, scene.window);

    assert_eq!(scene.json("globalThis.seen"), serde_json::json!([]));
    assert_eq!(
        scene.json("globalThis.released.map(([n]) => n)"),
        serde_json::json!([[node]])
    );
    assert!(scene.cells.document.borrow().tree().get(node).is_none());
}

#[gpui::test]
fn a_frame_without_orphans_does_not_call_the_hook(cx: &mut TestAppContext) {
    let scene = scene(cx, RELEASED);
    scene.js(
            "__inca_native__.appendChild(__inca_native__.rootNodeId(), __inca_native__.createNode('div'));",
        );

    draw(cx, scene.window);

    assert_eq!(scene.json("globalThis.released"), serde_json::json!([]));
}

/// Two focusable nodes, drawn once, whose first node's `focus` listener focuses the second and
/// stores in `out` what `matches(':focus')`, `querySelector` and `closest` answer then.
fn focus_and_query_in_a_focus_listener(cx: &mut TestAppContext) -> (Scene, NodeId, NodeId) {
    let scene = scene(cx, "");
    let first = focusable_node(&scene.dispatcher, &scene.cells);
    let second = focusable_node(&scene.dispatcher, &scene.cells);
    draw(cx, scene.window);
    listen(
        &scene.dispatcher,
        first,
        "focus",
        &format!(
            r"() => {{
                const n = __inca_native__;
                n.focusNode({second});
                globalThis.out = [
                    n.nodeHandle({first})[0].matches(':focus'),
                    n.nodeHandle({second})[0].matches(':focus'),
                    n.querySelector(n.rootNodeId(), ':focus') === {second},
                    n.closest({second}, ':focus') === {second},
                ];
            }}"
        ),
    );
    scene
        .cells
        .document
        .borrow_mut()
        .request_focus(first, false);
    draw(cx, scene.window);
    (scene, first, second)
}

#[gpui::test]
fn a_focus_query_in_a_focus_listener_keeps_the_drawn_focus_of_the_frame(cx: &mut TestAppContext) {
    let (scene, first, second) = focus_and_query_in_a_focus_listener(cx);

    let document = scene.cells.document.borrow();
    assert!(document.tree().is_focused(first));
    assert!(!document.tree().is_focused(second));
}

#[gpui::test]
fn a_focus_query_in_a_focus_listener_sees_the_focus_it_queued(cx: &mut TestAppContext) {
    let (scene, _first, _second) = focus_and_query_in_a_focus_listener(cx);

    assert_eq!(
        read_out(&scene.dispatcher),
        serde_json::json!([false, true, true, true])
    );
}
