// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use gpui::{AppContext as _, Context};

const SEEN: &str = r"
    globalThis.seen = [];
    globalThis.mark = (e) => { globalThis.seen.push(e.type + ':' + e.target); };
    ";

fn focusable_node(dispatcher: &EventDispatcher, host: &Rc<RefCell<Host>>) -> NodeId {
    let mut host = host.borrow_mut();
    let node = host.tree.create_node("div").unwrap();
    let root = host.root();
    host.tree.append_child(root, node).unwrap();
    host.tree
        .set_attribute(node, "tabindex", inca_gpui::AttributeValue::from("0"))
        .unwrap();
    host.focus.mark_tab_dirty(node);
    drop(host);
    dispatcher.engine.eval::<()>(SEEN).unwrap();
    for event in ["focus", "blur"] {
        listen(dispatcher, node, event, "mark");
    }
    node
}

/// A view whose render runs `begin_frame`, as a window's render does.
struct Frame(Rc<EventDispatcher>);

impl gpui::Render for Frame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.0.begin_frame(window, cx);
        gpui::div()
    }
}

fn draw(cx: &mut TestAppContext, window: gpui::WindowHandle<Frame>) {
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

fn seen(dispatcher: &EventDispatcher) -> String {
    dispatcher
        .engine
        .eval::<String>("JSON.stringify(globalThis.seen)")
        .unwrap()
}

fn set_active(
    cx: &mut TestAppContext,
    window: gpui::WindowHandle<Frame>,
    dispatcher: &EventDispatcher,
    active: bool,
) {
    cx.update_window(window.into(), |_, window, cx| {
        dispatcher.window_activated(active, window, cx);
    })
    .unwrap();
}

/// A window over one focusable node whose `focus` and `blur` are listened to, with the window
/// deactivated.
fn inactive_scene(
    cx: &mut TestAppContext,
) -> (
    Rc<EventDispatcher>,
    Rc<RefCell<Host>>,
    gpui::WindowHandle<Frame>,
    NodeId,
) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = focusable_node(&dispatcher, &host);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    draw(cx, window);
    set_active(cx, window, &dispatcher, false);
    (dispatcher, host, window, node)
}

#[gpui::test]
fn focusing_in_an_inactive_window_fires_focus_when_it_activates(cx: &mut TestAppContext) {
    let (dispatcher, host, window, node) = inactive_scene(cx);

    host.borrow_mut().focus.request_focus(node, true);
    draw(cx, window);
    let held = seen(&dispatcher);
    set_active(cx, window, &dispatcher, true);

    assert_eq!(held, "[]");
    assert_eq!(seen(&dispatcher), format!(r#"["focus:{node}"]"#));
}

#[gpui::test]
fn focusing_then_blurring_in_an_inactive_window_fires_nothing(cx: &mut TestAppContext) {
    let (dispatcher, host, window, node) = inactive_scene(cx);

    host.borrow_mut().focus.request_focus(node, true);
    draw(cx, window);
    host.borrow_mut().focus.request_blur(node);
    draw(cx, window);
    set_active(cx, window, &dispatcher, true);

    assert_eq!(seen(&dispatcher), "[]");
}

#[gpui::test]
fn toggling_the_activation_of_a_window_with_nothing_focused_fires_no_focus_event(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, window, node) = inactive_scene(cx);
    let root = host.borrow().root();
    for target in [root, node] {
        for event in ["blur", "focus", "focusout", "focusin"] {
            listen(&dispatcher, target, event, "mark");
        }
    }

    set_active(cx, window, &dispatcher, true);
    assert_eq!(seen(&dispatcher), "[]");

    set_active(cx, window, &dispatcher, false);
    assert_eq!(seen(&dispatcher), "[]");
}

#[gpui::test]
fn a_node_made_inert_while_the_window_is_inactive_fires_no_second_blur(cx: &mut TestAppContext) {
    let (dispatcher, host, window, node) = inactive_scene(cx);
    set_active(cx, window, &dispatcher, true);
    host.borrow_mut().focus.request_focus(node, true);
    draw(cx, window);
    dispatcher
        .engine
        .eval::<()>("globalThis.seen = [];")
        .unwrap();

    set_active(cx, window, &dispatcher, false);
    {
        let host = &mut *host.borrow_mut();
        host.tree.set_attribute(node, "inert", true).unwrap();
        host.focus.mark_subtree_dirty(&host.tree, node);
    }
    draw(cx, window);
    set_active(cx, window, &dispatcher, true);

    assert_eq!(seen(&dispatcher), format!(r#"["blur:{node}"]"#));
}

#[gpui::test]
fn window_activation_reports_the_four_focus_events_without_a_related_target(
    cx: &mut TestAppContext,
) {
    let (dispatcher, host, window, node) = inactive_scene(cx);
    set_active(cx, window, &dispatcher, true);
    host.borrow_mut().focus.request_focus(node, true);
    draw(cx, window);
    dispatcher
        .engine
        .eval::<()>("globalThis.seen = [];")
        .unwrap();
    for event in ["focusin", "focusout"] {
        listen(
            &dispatcher,
            node,
            event,
            "(e) => seen.push(e.type + ':' + e.relatedTarget)",
        );
    }

    set_active(cx, window, &dispatcher, false);
    set_active(cx, window, &dispatcher, true);

    assert_eq!(
        seen(&dispatcher),
        format!(r#"["blur:{node}","focusout:null","focus:{node}","focusin:null"]"#)
    );
}

/// Focuses a node, then drops its focus the way `removeChild` does, with no
/// frame in between.
fn focused_then_dropped(
    cx: &mut TestAppContext,
) -> (Rc<EventDispatcher>, gpui::WindowHandle<Frame>, NodeId) {
    let (dispatcher, host, _reported) = dispatcher_with_engine();
    let node = focusable_node(&dispatcher, &host);
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    host.borrow_mut().focus.request_focus(node, true);
    draw(cx, window);
    dispatcher
        .engine
        .eval::<()>("globalThis.seen = [];")
        .unwrap();
    {
        let host = &mut *host.borrow_mut();
        host.focus.drop_focus_in(&host.tree, node);
    }
    (dispatcher, window, node)
}

#[gpui::test]
fn a_window_that_deactivates_and_reactivates_after_a_drop_fires_nothing(cx: &mut TestAppContext) {
    let (dispatcher, window, _node) = focused_then_dropped(cx);

    for active in [false, true] {
        set_active(cx, window, &dispatcher, active);
    }

    assert_eq!(seen(&dispatcher), "[]");
}

const RELEASED: &str = r"
    globalThis.released = [];
    globalThis.__inca_released__ = (nodes, callbacks) => {
        globalThis.released.push([nodes, callbacks]);
    };
    ";

struct Scene {
    dispatcher: Rc<EventDispatcher>,
    host: Rc<RefCell<Host>>,
    reported: Reported,
    window: gpui::WindowHandle<Frame>,
}

/// A window over a fresh host whose engine runs `hook_js` first.
fn scene(cx: &mut TestAppContext, hook_js: &str) -> Scene {
    let (dispatcher, host, reported) = dispatcher_with_engine();
    dispatcher.engine.eval::<()>(hook_js).unwrap();
    let dispatcher = Rc::new(dispatcher);
    let window = cx.add_window(|_, _| Frame(Rc::clone(&dispatcher)));
    Scene {
        dispatcher,
        host,
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

#[gpui::test]
fn the_hook_receives_the_freed_ids_once(cx: &mut TestAppContext) {
    let scene = scene(cx, RELEASED);
    scene.js(r"
        const n = __inca_native__;
        globalThis.top = n.createNode('div');
        globalThis.leaf = n.createNode('div');
        n.appendChild(top, leaf);
        globalThis.cb = n.addEventListener(leaf, 'click', false, false);
        ");

    draw(cx, scene.window);
    draw(cx, scene.window);

    assert_eq!(
        scene.json("[globalThis.released, [top, leaf, cb]]"),
        serde_json::json!([[[[1, 2], [0]]], [1, 2, 0]])
    );
    assert!(scene.host.borrow().tree.get(1).is_none());
}

#[gpui::test]
fn a_hook_that_throws_is_reported(cx: &mut TestAppContext) {
    let scene = scene(
        cx,
        "globalThis.__inca_released__ = () => { throw new Error('boom'); };",
    );
    scene.js("__inca_native__.createNode('div');");
    let node = focusable_node(&scene.dispatcher, &scene.host);
    scene.host.borrow_mut().focus.request_focus(node, true);

    draw(cx, scene.window);

    let reported = scene.reported.borrow();
    assert_eq!(reported.len(), 1);
    assert!(reported[0].message().contains("boom"));
}

#[gpui::test]
fn a_hook_that_throws_lets_the_frame_finish(cx: &mut TestAppContext) {
    let scene = scene(
        cx,
        "globalThis.__inca_released__ = () => { throw new Error('boom'); };",
    );
    scene.js("__inca_native__.createNode('div');");
    let node = focusable_node(&scene.dispatcher, &scene.host);
    scene.host.borrow_mut().focus.request_focus(node, true);

    draw(cx, scene.window);

    assert_eq!(
        scene.json("globalThis.seen"),
        serde_json::json!([format!("focus:{node}")])
    );
    assert!(!scene.host.borrow().tree.has_orphans());
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
    assert_eq!(scene.json("globalThis.released"), serde_json::json!([[1]]));
    assert!(scene.requested_frames(cx) > 0);
    assert!(scene.reported.borrow().is_empty());

    draw(cx, scene.window);
    assert_eq!(
        scene.json("globalThis.released"),
        serde_json::json!([[1], [2]])
    );
    assert_eq!(scene.requested_frames(cx), 0);
}

#[gpui::test]
fn without_a_hook_a_frame_still_frees_the_orphans(cx: &mut TestAppContext) {
    let scene = scene(cx, "");
    scene.js(r"
        globalThis.node = __inca_native__.createNode('div');
        __inca_native__.addEventListener(node, 'click', false, false);
        ");

    draw(cx, scene.window);

    assert!(scene.host.borrow().tree.get(1).is_none());
    assert!(scene.host.borrow().listeners.is_empty());
}

#[gpui::test]
fn without_a_hook_a_frame_reports_nothing(cx: &mut TestAppContext) {
    let scene = scene(cx, "");
    scene.js(r"
        globalThis.node = __inca_native__.createNode('div');
        __inca_native__.addEventListener(node, 'click', false, false);
        ");

    draw(cx, scene.window);

    assert!(scene.reported.borrow().is_empty());
}

#[gpui::test]
fn a_removed_focused_node_is_swept_without_a_blur(cx: &mut TestAppContext) {
    let scene = scene(cx, RELEASED);
    let node = focusable_node(&scene.dispatcher, &scene.host);
    scene.host.borrow_mut().focus.request_focus(node, true);
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
    assert!(scene.host.borrow().tree.get(node).is_none());
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
