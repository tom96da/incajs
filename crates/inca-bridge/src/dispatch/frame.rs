// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The work a window owes before a frame and when it gains or loses focus.

use gpui::{App, Window};
use inca_document::EventPayload;
use inca_gpui::event_sink::with_screen;

use super::EventDispatcher;
use crate::EventTargetId;
use crate::focus::dispatch_transition;

impl EventDispatcher {
    /// Runs the work a window owes before it draws a frame: prints the warnings of the `<style>`
    /// sheets, syncs the modifier keys, applies a requested focus, writes the focus that style
    /// rules read and fires the resulting `focus`/`blur` events, frees the nodes nothing reaches,
    /// reveals the focused node and fires pending `scroll` events, and requests another frame
    /// while a focus request waits. Call it first in the window's render.
    pub fn begin_frame(&self, window: &mut Window, cx: &mut App) {
        self.warn_style_sheets();
        self.sync_modifiers(window, false);
        let transitions = {
            let host = &mut *self.host.document.borrow_mut();
            let transitions = host
                .focus
                .apply_pending(&host.tree, &host.style, window, cx);
            host.sync_focus_style();
            transitions
        };
        for transition in &transitions {
            self.input_focus_moved(transition, window, cx);
            dispatch_transition(transition, self, window, cx);
        }
        self.release_orphans(window, cx);
        self.host.reveal_focus(window);
        self.fire_scrolls(window, cx);
        // A refresh requested while drawing has no effect, so a listener's own focus changes need
        // the next frame.
        if self.host.document.borrow().focus.has_queued_work() {
            window.request_animation_frame();
        }
    }

    /// Prints each warning of the sheets parsed since the last frame, prefixed with `<style>: `.
    fn warn_style_sheets(&self) {
        let host = self.host.document.borrow();
        for line in host.style.take_style_warnings(&host.tree) {
            host.warnings.emit(&format!("<style>: {line}"));
        }
    }

    /// Frees the orphans of the tree and tells the script through the global
    /// `__inca_released__(nodeIds, callbackIds)`. The global is looked up per
    /// call and skipped when it is not a function. A thrown error is
    /// reported and the frame goes on. Nodes the hook leaves orphaned wait
    /// for the next frame, which this requests.
    fn release_orphans(&self, window: &mut Window, cx: &mut App) {
        let swept = self.host.sweep_orphans();
        let Some((nodes, callback_ids)) = swept else {
            return;
        };
        self.host.document.borrow_mut().focus.observe(window, cx);
        let entered = self.script.release(&nodes, &callback_ids);
        if entered {
            self.drain_observed(window);
        }
        // The requests end when a frame leaves no orphan.
        if self.host.document.borrow().tree.has_orphans() {
            window.request_animation_frame();
        }
    }

    /// Fires `event` at `document` or `window` alone, with no node on its path.
    pub(super) fn fire_at_global(
        &self,
        target: EventTargetId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.host.document.borrow_mut().focus.observe(window, cx);
        let payload = self.with_held_buttons(event, payload.clone());
        let payload = with_screen(payload, window);
        let (_, entered) = self.walk(target, event, &payload, &[]);
        if entered {
            self.drain_observed(window);
        }
    }

    /// Fires `scroll` at each container whose offset changed since the last
    /// call and that a `scroll` listener reaches. A script write and a wheel
    /// scroll each fire once. Only the containers a listener reaches run JS.
    pub fn fire_scrolls(&self, window: &mut Window, cx: &mut App) {
        let moved = self.host.scrolled_nodes();
        let any = !moved.is_empty();
        for node in moved {
            self.fire(node, "scroll", &EventPayload::None, window, cx);
        }
        if any {
            window.request_animation_frame();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::test_support::Reported;
    use crate::tests::{
        Frame, callback_stored, dispatcher_with_engine, draw, eval_json, focus_scene,
        focusable_node, is_focused,
    };
    use crate::{EventDispatcher, Host};
    use gpui::{AppContext as _, TestAppContext};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[gpui::test]
    fn a_frame_prints_the_warnings_of_a_style_sheet_once(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let printed = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&printed);
        host.document
            .borrow_mut()
            .warnings
            .set_sink(Rc::new(move |line| sink.borrow_mut().push(line.to_owned())));
        {
            let mut host = host.document.borrow_mut();
            let root = host.root();
            let style = host.tree.create_node("style").unwrap();
            let text = host.tree.create_node("#text").unwrap();
            host.tree.set_data(text, ".a { bogus: 1 }").unwrap();
            host.tree.append_child(style, text).unwrap();
            host.tree.append_child(root, style).unwrap();
        }
        let window = cx.add_window(|_, _| Frame(Rc::new(dispatcher)));

        draw(cx, window);
        draw(cx, window);

        assert_eq!(
            *printed.borrow(),
            ["<style>: ignoring unknown style key `bogus`"]
        );
    }

    const RELEASED: &str = r"
        globalThis.released = [];
        globalThis.__inca_released__ = (nodes, callbacks) => {
            globalThis.released.push([nodes, callbacks]);
        };
        ";

    struct Scene {
        dispatcher: Rc<EventDispatcher>,
        host: Rc<Host>,
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
            globalThis.cb = n.addEventListener(leaf, 'click', () => {}, false, false);
            ");

        draw(cx, scene.window);
        draw(cx, scene.window);

        assert_eq!(
            scene.json("[globalThis.released, [top, leaf, cb]]"),
            serde_json::json!([[[[2, 3], [1]]], [2, 3, 1]])
        );
        assert!(scene.host.document.borrow().tree.get(2).is_none());
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
        let node = focusable_node(&scene.dispatcher, &scene.host);
        scene
            .host
            .document
            .borrow_mut()
            .focus
            .request_focus(node, true);

        draw(cx, scene.window);

        let reported = scene.reported.borrow();
        assert_eq!(reported.len(), 1);
        assert!(reported[0].message().contains("boom"));
    }

    /// A scene with an orphan that has a listener, whose callback table is frozen.
    fn scene_with_frozen_callbacks(cx: &mut TestAppContext) -> Scene {
        let scene = scene(cx, RELEASED);
        scene.js(r"
            const n = __inca_native__;
            n.addEventListener(n.createNode('div'), 'click', () => {}, false, false);
            ");
        scene.dispatcher.engine.with(|ctx| {
            let table: rquickjs::Object =
                inca_jsenv::vault(&ctx).unwrap().get("callbacks").unwrap();
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
        let node = focusable_node(&scene.dispatcher, &scene.host);
        scene
            .host
            .document
            .borrow_mut()
            .focus
            .request_focus(node, true);

        draw(cx, scene.window);

        assert_eq!(
            scene.json("globalThis.seen"),
            serde_json::json!([format!("focus:{node}")])
        );
        assert!(!scene.host.document.borrow().tree.has_orphans());
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

        assert!(scene.host.document.borrow().tree.get(2).is_none());
        assert!(scene.host.document.borrow().listeners.is_empty());
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
        let node = focusable_node(&scene.dispatcher, &scene.host);
        scene
            .host
            .document
            .borrow_mut()
            .focus
            .request_focus(node, true);
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
        assert!(scene.host.document.borrow().tree.get(node).is_none());
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

    #[gpui::test]
    fn a_frame_writes_the_focus_a_request_asked_for(cx: &mut TestAppContext) {
        let (_dispatcher, host, window, node) = focus_scene(cx);
        host.document.borrow_mut().focus.request_focus(node, true);

        draw(cx, window);

        assert!(is_focused(&host.document.borrow(), node));
    }

    #[gpui::test]
    fn a_frame_clears_the_focus_of_a_node_that_stops_being_focusable(cx: &mut TestAppContext) {
        let (_dispatcher, host, window, node) = focus_scene(cx);
        host.document.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);
        {
            let host = &mut *host.document.borrow_mut();
            host.tree.remove_attribute(node, "tabindex").unwrap();
            host.focus.mark_tab_dirty(node);
        }

        draw(cx, window);

        assert!(!is_focused(&host.document.borrow(), node));
    }

    /// A scene with `count` focusable nodes, settled by one draw.
    fn settled(cx: &mut TestAppContext, count: usize) -> (Scene, Vec<u32>) {
        let scene = scene(cx, "");
        let nodes = (0..count)
            .map(|_| focusable_node(&scene.dispatcher, &scene.host))
            .collect();
        draw(cx, scene.window);
        scene.requested_frames(cx);
        (scene, nodes)
    }

    #[gpui::test]
    fn a_focus_requested_in_a_focus_listener_applies_on_the_next_frame(cx: &mut TestAppContext) {
        let (scene, nodes) = settled(cx, 2);
        let (first, second) = (nodes[0], nodes[1]);
        scene.js(&format!(
            r"
            __inca_test_listen__({first}, 'focus', () => {{
                __inca_native__.focusNode({second}, true);
            }}, false, false);
            "
        ));
        scene
            .host
            .document
            .borrow_mut()
            .focus
            .request_focus(first, false);

        draw(cx, scene.window);
        assert!(!is_focused(&scene.host.document.borrow(), second));
        assert_eq!(scene.requested_frames(cx), 1);
        draw(cx, scene.window);

        assert!(is_focused(&scene.host.document.borrow(), second));
    }

    #[gpui::test]
    fn listeners_focusing_each_other_move_the_focus_once_per_frame(cx: &mut TestAppContext) {
        let (scene, nodes) = settled(cx, 2);
        let (first, second) = (nodes[0], nodes[1]);
        scene.js(&format!(
            r"
            __inca_test_listen__({first}, 'focus', () => {{
                __inca_native__.focusNode({second}, true);
            }}, false, false);
            __inca_test_listen__({second}, 'focus', () => {{
                __inca_native__.focusNode({first}, true);
            }}, false, false);
            "
        ));
        scene
            .host
            .document
            .borrow_mut()
            .focus
            .request_focus(first, false);

        draw(cx, scene.window);
        assert!(is_focused(&scene.host.document.borrow(), first));
        assert!(!is_focused(&scene.host.document.borrow(), second));
        // Running a requested frame draws it, and each draw requests one frame in turn.
        assert_eq!(scene.requested_frames(cx), 1);
        assert!(is_focused(&scene.host.document.borrow(), second));
        assert_eq!(scene.requested_frames(cx), 1);

        assert!(is_focused(&scene.host.document.borrow(), first));
    }

    #[gpui::test]
    fn a_settled_frame_with_no_owed_work_requests_no_frame(cx: &mut TestAppContext) {
        let (scene, nodes) = settled(cx, 1);
        scene
            .host
            .document
            .borrow_mut()
            .focus
            .request_focus(nodes[0], false);

        draw(cx, scene.window);
        scene.requested_frames(cx);
        draw(cx, scene.window);

        assert_eq!(scene.requested_frames(cx), 0);
    }

    #[gpui::test]
    fn a_tabindex_removed_in_a_focus_listener_blurs_the_node_on_the_next_frame(
        cx: &mut TestAppContext,
    ) {
        let (scene, nodes) = settled(cx, 1);
        let node = nodes[0];
        scene.js(&format!(
            r"
            __inca_test_listen__({node}, 'focus', () => {{
                __inca_native__.removeAttribute({node}, 'tabindex');
            }}, false, false);
            "
        ));
        scene
            .host
            .document
            .borrow_mut()
            .focus
            .request_focus(node, false);

        draw(cx, scene.window);
        assert!(is_focused(&scene.host.document.borrow(), node));
        assert_eq!(scene.requested_frames(cx), 1);
        draw(cx, scene.window);

        assert!(!is_focused(&scene.host.document.borrow(), node));
    }
}
