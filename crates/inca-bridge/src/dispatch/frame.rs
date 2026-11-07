// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The work a window owes before a frame and when it gains or loses focus.

use gpui::{App, Window};
use inca_gpui::event_sink::with_screen;
use inca_gpui::{EventPayload, MousePayload, NO_BUTTON};
use inca_jsenv::{EngineError, callbacks};
use rquickjs::Function;

use super::EventDispatcher;
use crate::EventTargetId;
use crate::event;
use crate::focus::FocusTransition;

/// The payload of the window's `focus` and `blur`.
const FOCUS_PAYLOAD: EventPayload = EventPayload::Focus {
    related_target: None,
};

impl EventDispatcher {
    /// Runs the work a window owes before it draws a frame: prints the warnings of the `<style>`
    /// sheets, syncs the modifier keys, applies a requested focus, writes the focus that style
    /// rules read and fires the resulting `focus`/`blur` events, frees the nodes nothing reaches,
    /// reveals the focused node and fires pending `scroll` events. Call it first in the window's
    /// render.
    pub fn begin_frame(&self, window: &mut Window, cx: &mut App) {
        self.warn_style_sheets();
        self.sync_modifiers(window, false);
        let transitions = {
            let host = &mut *self.host.borrow_mut();
            let transitions = host.focus.apply_pending(&host.tree, window, cx);
            host.sync_focus_style();
            transitions
        };
        for transition in &transitions {
            self.input_focus_moved(transition, window, cx);
            transition.dispatch(self, window, cx);
        }
        self.release_orphans(window, cx);
        self.host.borrow_mut().reveal_focus(window);
        self.fire_scrolls(window, cx);
    }

    /// Prints each warning of the sheets parsed since the last frame, prefixed with `<style>: `.
    fn warn_style_sheets(&self) {
        let host = self.host.borrow();
        for line in host.tree.take_style_warnings() {
            (host.warn)(&format!("<style>: {line}"));
        }
    }

    /// Frees the orphans of the tree and tells the script through the global
    /// `__inca_released__(nodeIds, callbackIds)`. The global is looked up per
    /// call and skipped when it is not a function. A thrown error is
    /// reported and the frame goes on. Nodes the hook leaves orphaned wait
    /// for the next frame, which this requests.
    fn release_orphans(&self, window: &mut Window, cx: &mut App) {
        let swept = self.host.borrow_mut().sweep_orphans();
        let Some((nodes, callback_ids)) = swept else {
            return;
        };
        self.host.borrow_mut().focus.observe(window, cx);
        let host = &self.host;
        let entered = self.engine.with(|ctx| {
            let released = callbacks::release(&ctx, &callback_ids);
            let hook = ctx.globals().get::<_, Function>("__inca_released__").ok();
            let called = released.and_then(|()| {
                let Some(hook) = &hook else {
                    return Ok(());
                };
                let nodes = event::array_of(&ctx, nodes)?;
                let callback_ids = event::array_of(&ctx, callback_ids)?;
                hook.call::<_, ()>((nodes, callback_ids))
            });
            if let Err(err) = called {
                host.borrow_mut()
                    .failures
                    .push(EngineError::capture(&ctx, &err));
            }
            hook.is_some()
        });
        if entered {
            self.drain_observed(window);
        }
        // The requests end when a frame leaves no orphan.
        if self.host.borrow().tree.has_orphans() {
            window.request_animation_frame();
        }
    }

    /// Handles the window gaining (`active` true) or losing focus. The modifier keys are re-read,
    /// a lost window clears the held presses, style rules see no focus while the window is
    /// inactive, the focused node receives `focus` or `blur` while keeping its focus, and an
    /// edited focused `input` fires `change` before its `blur`. `window` receives `focus` before
    /// the node and `blur` after it.
    pub fn window_activated(&self, active: bool, window: &mut Window, cx: &mut App) {
        self.sync_modifiers(window, true);
        if !active {
            inca_gpui::clear_presses(cx);
            self.reset_pointer(window, cx);
        }
        let changed = {
            let host = &mut *self.host.borrow_mut();
            host.focus.set_window_blurred(!active);
            host.focus.observe(window, cx);
            host.sync_focus_style()
        };
        self.restyle(changed, window);
        let focused = self.host.borrow().focus.focused_node(window, cx);
        if active {
            self.fire_at_global(EventTargetId::Window, "focus", &FOCUS_PAYLOAD, window, cx);
        }
        if let Some(node) = focused {
            if !active {
                self.input_window_blur(node, window, cx);
            }
            FocusTransition::window_activation(node, active).dispatch(self, window, cx);
        }
        if !active {
            self.fire_at_global(EventTargetId::Window, "blur", &FOCUS_PAYLOAD, window, cx);
        }
    }

    /// Ends the pointer capture with `lostpointercapture`, then forgets the held buttons and a
    /// canceled `pointerdown`. The release of a button the window never sees would leave them set.
    fn reset_pointer(&self, window: &mut Window, cx: &mut App) {
        self.host.borrow_mut().capture_pending = None;
        let payload =
            EventPayload::Mouse(MousePayload::at(gpui::Point::default(), window.modifiers()));
        self.process_capture(&payload, NO_BUTTON, window, cx);
        self.state.pointerdown_canceled.set(false);
        self.state.input_drag.set(None);
        let mut host = self.host.borrow_mut();
        host.held_buttons = 0;
        host.capture_pending = None;
        host.capture_active = None;
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
        self.host.borrow_mut().focus.observe(window, cx);
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
        let moved = self.host.borrow_mut().scrolled_nodes();
        let any = !moved.is_empty();
        for node in moved {
            self.fire(node, "scroll", &EventPayload::None, window, cx);
        }
        // A refresh requested while drawing has no effect, so a listener's own
        // writes need the next frame.
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
        focusable_node, is_focused, listen, listen_global, seen_text, set_active,
    };
    use crate::{EventDispatcher, Host};
    use gpui::{AppContext as _, TestAppContext};
    use inca_gpui::NodeId;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[gpui::test]
    fn a_frame_prints_the_warnings_of_a_style_sheet_once(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let printed = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&printed);
        host.borrow_mut().warn = Rc::new(move |line| sink.borrow_mut().push(line.to_owned()));
        {
            let mut host = host.borrow_mut();
            let root = host.root();
            let style = host.tree.create_node("style").unwrap();
            let text = host.tree.create_node("#text").unwrap();
            host.tree
                .set_attribute(text, "value", ".a { bogus: 1 }")
                .unwrap();
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
            globalThis.cb = n.addEventListener(leaf, 'click', () => {}, false, false);
            ");

        draw(cx, scene.window);
        draw(cx, scene.window);

        assert_eq!(
            scene.json("[globalThis.released, [top, leaf, cb]]"),
            serde_json::json!([[[[1, 2], [1]]], [1, 2, 1]])
        );
        assert!(scene.host.borrow().tree.get(1).is_none());
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
            serde_json::json!([[[1], [1]], [1, 1], true])
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
            __inca_native__.addEventListener(node, 'click', () => {}, false, false);
            ");

        draw(cx, scene.window);

        assert!(scene.host.borrow().tree.get(1).is_none());
        assert!(scene.host.borrow().listeners.is_empty());
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

    #[gpui::test]
    fn a_frame_writes_the_focus_a_request_asked_for(cx: &mut TestAppContext) {
        let (_dispatcher, host, window, node) = focus_scene(cx);
        host.borrow_mut().focus.request_focus(node, true);

        draw(cx, window);

        assert!(is_focused(&host.borrow(), node));
    }

    #[gpui::test]
    fn a_frame_clears_the_focus_of_a_node_that_stops_being_focusable(cx: &mut TestAppContext) {
        let (_dispatcher, host, window, node) = focus_scene(cx);
        host.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);
        {
            let host = &mut *host.borrow_mut();
            host.tree.remove_attribute(node, "tabindex").unwrap();
            host.focus.mark_tab_dirty(node);
        }

        draw(cx, window);

        assert!(!is_focused(&host.borrow(), node));
    }

    type Handle = gpui::WindowHandle<Frame>;

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
        let held = seen_text(&dispatcher);
        set_active(cx, window, &dispatcher, true);

        assert_eq!(held, "[]");
        assert_eq!(seen_text(&dispatcher), format!(r#"["focus:{node}"]"#));
    }

    #[gpui::test]
    fn focusing_then_blurring_in_an_inactive_window_fires_nothing(cx: &mut TestAppContext) {
        let (dispatcher, host, window, node) = inactive_scene(cx);

        host.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);
        host.borrow_mut().focus.request_blur(node);
        draw(cx, window);
        set_active(cx, window, &dispatcher, true);

        assert_eq!(seen_text(&dispatcher), "[]");
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
        assert_eq!(seen_text(&dispatcher), "[]");

        set_active(cx, window, &dispatcher, false);
        assert_eq!(seen_text(&dispatcher), "[]");
    }

    #[gpui::test]
    fn a_node_made_inert_while_the_window_is_inactive_fires_no_second_blur(
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

        set_active(cx, window, &dispatcher, false);
        {
            let host = &mut *host.borrow_mut();
            host.tree.set_attribute(node, "inert", true).unwrap();
            host.focus.mark_subtree_dirty(&host.tree, node);
        }
        draw(cx, window);
        set_active(cx, window, &dispatcher, true);

        assert_eq!(seen_text(&dispatcher), format!(r#"["blur:{node}"]"#));
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
            seen_text(&dispatcher),
            format!(r#"["blur:{node}","focusout:null","focus:{node}","focusin:null"]"#)
        );
    }

    /// Marks every `focus` and `blur` that reaches `window`, as `type:target` in `seen`.
    fn listen_at_window(dispatcher: &EventDispatcher) {
        for event in ["focus", "blur"] {
            listen_global(dispatcher, "$window", event, false, "mark");
        }
    }

    #[gpui::test]
    fn activating_the_window_fires_focus_at_window_before_the_focused_node(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, window, node) = inactive_scene(cx);
        listen_at_window(&dispatcher);
        host.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);

        set_active(cx, window, &dispatcher, true);

        assert_eq!(
            seen_text(&dispatcher),
            format!(r#"["focus:$window","focus:{node}"]"#)
        );
    }

    #[gpui::test]
    fn deactivating_the_window_fires_blur_at_window_after_the_focused_node(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, window, node) = inactive_scene(cx);
        set_active(cx, window, &dispatcher, true);
        host.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);
        listen_at_window(&dispatcher);
        dispatcher
            .engine
            .eval::<()>("globalThis.seen = [];")
            .unwrap();

        set_active(cx, window, &dispatcher, false);

        assert_eq!(
            seen_text(&dispatcher),
            format!(r#"["blur:{node}","blur:$window"]"#)
        );
    }

    #[gpui::test]
    fn toggling_the_activation_with_nothing_focused_fires_focus_and_blur_at_window(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, _host, window, _node) = inactive_scene(cx);
        listen_at_window(&dispatcher);

        set_active(cx, window, &dispatcher, true);
        set_active(cx, window, &dispatcher, false);

        assert_eq!(
            seen_text(&dispatcher),
            r#"["focus:$window","blur:$window"]"#
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
    fn a_window_that_deactivates_and_reactivates_after_a_drop_fires_nothing(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, window, _node) = focused_then_dropped(cx);

        for active in [false, true] {
            set_active(cx, window, &dispatcher, active);
        }

        assert_eq!(seen_text(&dispatcher), "[]");
    }

    /// Whether `node` is focused for style rules right after `window_activated(active)` returned.
    fn focused_after_activation(
        cx: &mut TestAppContext,
        (dispatcher, host, window): (&crate::EventDispatcher, &RefCell<crate::Host>, Handle),
        active: bool,
        node: u32,
    ) -> bool {
        cx.update_window(window.into(), |_, window, cx| {
            dispatcher.window_activated(active, window, cx);
            is_focused(&host.borrow(), node)
        })
        .unwrap()
    }

    #[gpui::test]
    fn deactivating_the_window_clears_the_focus_state(cx: &mut TestAppContext) {
        let (dispatcher, host, window, node) = focus_scene(cx);
        host.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);

        assert!(!focused_after_activation(
            cx,
            (&dispatcher, &host, window),
            false,
            node
        ));
    }

    #[gpui::test]
    fn activating_the_window_restores_the_focus_state(cx: &mut TestAppContext) {
        let (dispatcher, host, window, node) = focus_scene(cx);
        host.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);
        set_active(cx, window, &dispatcher, false);

        assert!(focused_after_activation(
            cx,
            (&dispatcher, &host, window),
            true,
            node
        ));
    }

    #[gpui::test]
    fn activating_the_window_writes_the_node_gpui_focused_meanwhile(cx: &mut TestAppContext) {
        let (dispatcher, host, window, first) = focus_scene(cx);
        let second = focusable_node(&dispatcher, &host);
        host.borrow_mut().focus.request_focus(first, true);
        draw(cx, window);
        set_active(cx, window, &dispatcher, false);
        let handle = host.borrow().focus.handle(second).unwrap();

        let focused = cx
            .update_window(window.into(), |_, window, cx| {
                handle.focus(window, cx);
                dispatcher.window_activated(true, window, cx);
                let host = host.borrow();
                (is_focused(&host, second), is_focused(&host, first))
            })
            .unwrap();

        assert_eq!(focused, (true, false));
    }
}
