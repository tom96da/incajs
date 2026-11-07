// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Fires one event along the path of a node and resolves its targets.

use gpui::{App, Window};
use inca_gpui::event_sink::{flags_of, with_screen};
use inca_gpui::{EventPayload, MousePayload, PointerSource};
use inca_node::{NodeId, is_disabled_control};

use super::EventDispatcher;
use crate::EventTargetId;
use crate::event::{self, Walk};

impl EventDispatcher {
    /// Dispatches `event` along the path of `target`, with `target` as its
    /// target, and reports what the listeners threw. Returns whether a
    /// listener prevented the default action and whether the walk entered the
    /// engine. `cut` lists the nodes whose listeners stay silent.
    pub(super) fn walk(
        &self,
        target: EventTargetId,
        event: &str,
        payload: &EventPayload,
        cut: &[NodeId],
    ) -> (bool, bool) {
        let outcome = event::walk(
            &self.engine,
            &self.host,
            &Walk {
                name: event,
                flags: flags_of(event, payload),
                target,
                cut,
                payload,
                trusted: true,
            },
        );
        // Reported after the walk: a reporter is free to do anything, and
        // re-entering the engine from inside `Engine::with` panics.
        self.report_pending();
        (outcome.canceled, outcome.entered)
    }

    /// `payload` as a listener sees it: a `related_target` that left the tree
    /// reads `None`, and the buttons, movement, screen and offset fields are
    /// set. `target` is the node the event fires at.
    fn prepare(
        &self,
        event: &str,
        target: NodeId,
        payload: &EventPayload,
        window: &Window,
    ) -> EventPayload {
        let payload = self.live_related(payload.clone());
        let payload = self.with_held_buttons(event, payload);
        let payload = self.with_movement(payload, event);
        let payload = with_screen(payload, window);
        self.with_offset(target, payload)
    }

    /// Fires `event` at `start` with `start` as the `target`. The capture
    /// listeners from `window` down to `start` run first, then the bubble
    /// listeners of `start` and, while the event bubbles, of each ancestor,
    /// until one calls `stopPropagation()`. Every listener receives the same
    /// event object, whose `eventId` is larger than the one of the event
    /// fired before. Returns whether a listener prevented the default
    /// action. `preventDefault()` calls from a passive listener are ignored.
    /// A pointer `click` or `dblclick` skips a disabled button or input and its
    /// ancestors, as `mousedown` and `mouseup` do. A `start` that left the
    /// tree fires nothing. When a listener ran, the job queue drains after
    /// the walk and the window refreshes. A call made while the window draws
    /// has no refresh to make, and the caller requests the next frame.
    ///
    /// While the listeners run, `eventPhase` is 1 on the ancestors of `target`
    /// during the capture pass, 2 where `currentTarget` is `target` and 3 on
    /// the ancestors during the bubble pass, and `composedPath()` lists the path,
    /// from `target` up through its element ancestors, document and window, as the
    /// objects that `__inca_targets__` gives, or as node ids and the tags
    /// `"$document"` and `"$window"` when no such global exists. Afterwards
    /// `currentTarget` is `null`, `eventPhase` is 0 and `composedPath()` is
    /// empty. A listener that throws is reported, and the remaining listeners
    /// run. A throw of `__inca_targets__` is reported and no listener runs.
    pub fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if self.host.borrow().tree.get(start).is_none() {
            return false;
        }
        self.fire_inner(start, event, payload, window, cx)
    }

    pub(super) fn fire_inner(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        self.host.borrow_mut().focus.observe(window, cx);
        let payload = &self.prepare(event, start, payload, window);
        let cut = self.cut_by_disabled_control(event, payload, cx);
        let (canceled, entered) = self.walk(EventTargetId::Node(start), event, payload, &cut);
        if entered {
            self.drain_observed(window);
        }
        canceled
    }

    /// `payload` with `offsetX`/`offsetY` measured from the padding edge of
    /// `target` in the last drawn frame, or equal to the client position when
    /// `target` has no recorded box.
    fn with_offset(&self, target: NodeId, mut payload: EventPayload) -> EventPayload {
        let host = self.host.borrow();
        let layout = host.layout.borrow().get(&target).copied();
        if let Some(mouse) = payload.mouse_mut() {
            let (left, top) = layout.map_or((0.0, 0.0), |l| {
                let border = f32::from(l.border);
                (
                    f32::from(l.bounds.origin.x) + border,
                    f32::from(l.bounds.origin.y) + border,
                )
            });
            mouse.offset_x = mouse.client_x - left;
            mouse.offset_y = mouse.client_y - top;
        }
        payload
    }

    /// `payload` with a `related_target` that left the tree set to `None`.
    fn live_related(&self, mut payload: EventPayload) -> EventPayload {
        let gone = |id: &NodeId| self.host.borrow().tree.get(*id).is_none();
        match &mut payload {
            EventPayload::Focus { related_target } => {
                if related_target.as_ref().is_some_and(gone) {
                    *related_target = None;
                }
            }
            other => {
                if let Some(mouse) = other.mouse_mut()
                    && mouse.related_target.as_ref().is_some_and(gone)
                {
                    mouse.related_target = None;
                }
            }
        }
        payload
    }

    /// `found` when that node is still in the tree, else `node_id`.
    pub(super) fn resolve(&self, found: Option<NodeId>, node_id: NodeId) -> NodeId {
        let host = self.host.borrow();
        found
            .filter(|id| host.tree.get(*id).is_some())
            .unwrap_or(node_id)
    }

    /// `target` and its ancestors up to the host root, nearest first.
    pub(super) fn path_from(&self, target: NodeId) -> Vec<NodeId> {
        self.host.borrow().tree.ancestors(target).collect()
    }

    /// The nearest node that is `a`, `b` or an ancestor of both.
    pub(super) fn common_ancestor(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        let above_b = self.path_from(b);
        self.path_from(a).into_iter().find(|i| above_b.contains(i))
    }

    /// The nodes that receive no pointer `mousedown`, `mouseup`, `click` or
    /// `dblclick`: a disabled button or input on the path from the mouse target
    /// or the pressed target of the event, and the ancestors of that control.
    /// Listeners below the control run. A keyboard click always runs.
    fn cut_by_disabled_control(
        &self,
        event: &str,
        payload: &EventPayload,
        cx: &App,
    ) -> Vec<NodeId> {
        let keyboard = matches!(
            payload,
            EventPayload::Mouse(MousePayload {
                pointer: Some(PointerSource::Keyboard),
                ..
            })
        );
        if keyboard || !matches!(event, "mousedown" | "mouseup" | "click" | "dblclick") {
            return Vec::new();
        }
        let host = self.host.borrow();
        let disabled = |id: &NodeId| host.tree.get(*id).is_some_and(is_disabled_control);
        let button = match payload {
            EventPayload::Mouse(mouse) => Some(mouse.button),
            _ => None,
        };
        let pressed = button.and_then(|button| inca_gpui::pressed_target(cx, button));
        let mut cut = Vec::new();
        for start in [inca_gpui::mouse_target(cx), pressed].into_iter().flatten() {
            let path = self.path_from(start);
            if let Some(button) = path.iter().position(disabled) {
                cut.extend_from_slice(&path[button..]);
            }
        }
        cut
    }

    /// The default action of a `mousedown` that reached `target`: the nearest
    /// node on its path that can take focus takes it, else the window blurs.
    /// A prevented `mousedown` leaves the focus alone. Every press records
    /// `target` as where Tab starts while no node is focused.
    pub(super) fn focus_on_press(
        &self,
        target: NodeId,
        prevented: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let path = self.path_from(target);
        let handle = {
            let mut host = self.host.borrow_mut();
            let focusing = if prevented {
                None
            } else {
                path.iter()
                    .copied()
                    .find(|&id| host.focus.handle(id).is_some())
            };
            let focused = host.focus.focused_node(window, cx);
            let root = host.root();
            host.focus
                .press_start(target, root, focusing.filter(|&node| Some(node) != focused));
            focusing.and_then(|node| host.focus.handle(node))
        };
        if prevented {
            return;
        }
        match handle {
            Some(handle) => handle.focus(window, cx),
            None => window.blur(cx),
        }
    }

    pub(super) fn key_target(&self, window: &mut Window, cx: &mut App) -> NodeId {
        let (root, focused) = {
            let host = self.host.borrow();
            (host.root(), host.focus.focused_node(window, cx))
        };
        self.resolve(focused, root)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::EventDispatcher;
    use crate::tests::{
        dispatcher_with_engine, fire_and_read, listen, listen_with, mouse_at, tree_of_three,
        wheel_at,
    };
    use gpui::TestAppContext;
    use inca_gpui::{EventPayload, EventSink};

    #[gpui::test]
    fn only_a_listener_the_event_reaches_enters_the_engine(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        let cx = cx.add_empty_window();
        let mut drained = |dispatcher: &EventDispatcher| {
            dispatcher
                .engine
                .eval::<()>(
                    "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
                )
                .unwrap();
            cx.update(|window, cx| {
                dispatcher.fire(child, "focus", &EventPayload::None, window, cx);
            });
            dispatcher.engine.eval::<bool>("globalThis.ran").unwrap()
        };

        // A bubble listener on an ancestor of a `focus` event stays out of reach.
        listen(&dispatcher, parent, "focus", "() => {}");
        assert!(!drained(&dispatcher), "bubble listener on an ancestor");

        listen_with(&dispatcher, parent, "focus", (true, false), "() => {}");
        assert!(drained(&dispatcher), "capture listener on an ancestor");
    }

    #[gpui::test]
    fn tracks_pointer_only_with_a_listener_or_a_focusable_node(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        let root = host.borrow().root();
        host.borrow_mut().tree.append_child(root, node).unwrap();
        assert!(!dispatcher.tracks_pointer());

        listen(&dispatcher, node, "keydown", "() => {}");
        assert!(dispatcher.tracks_pointer());
        host.borrow_mut()
            .listeners
            .remove(crate::EventTargetId::Node(node), "keydown", 1);
        assert!(!dispatcher.tracks_pointer());

        host.borrow_mut()
            .tree
            .set_attribute(node, "tabindex", "0")
            .unwrap();
        host.borrow_mut().focus.mark_tab_dirty(node);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let host = &mut *host.borrow_mut();
            let _ = host.focus.apply_pending(&host.tree, window, cx);
        });
        assert!(dispatcher.tracks_pointer());
    }

    #[gpui::test]
    fn an_event_nobody_keeps_is_collected_with_its_state(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            "(e) => { globalThis.weak = new WeakRef(e); e.stopPropagation(); }",
        );
        fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

        dispatcher.engine.with(|ctx| ctx.run_gc());

        let gone = dispatcher
            .engine
            .eval::<bool>("weak.deref() === undefined")
            .unwrap();
        assert!(gone);
    }

    #[gpui::test]
    fn an_unlistened_pointer_or_key_input_leaves_the_engine_untouched(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        listen(&dispatcher, node, "click", "() => {}");
        dispatcher
            .engine
            .eval::<()>(
                "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
            )
            .unwrap();
        let cx = cx.add_empty_window();
        let key = gpui::Keystroke::parse("a").unwrap();
        let ran = |d: &EventDispatcher| d.engine.eval::<bool>("globalThis.ran").unwrap();

        cx.update(|window, cx| {
            let sink: &dyn EventSink = &dispatcher;
            sink.pointer_input("mousemove", node, &mouse_at(1.0, 1.0), window, cx);
            sink.pointer_input("wheel", node, &wheel_at(1.0, 1.0), window, cx);
            sink.pointer_input("mousedown", node, &mouse_at(1.0, 1.0), window, cx);
            sink.pointer_input("mouseup", node, &mouse_at(1.0, 1.0), window, cx);
            sink.key_input(&key, false, &EventPayload::None, window, cx);
        });
        assert!(!ran(&dispatcher), "no listener, no drain");

        for event in ["mousemove", "wheel", "keydown"] {
            // A key goes to the root when nothing is focused.
            let listener = if event == "keydown" {
                host.borrow().root()
            } else {
                node
            };
            listen(&dispatcher, listener, event, "() => {}");
            dispatcher
                .engine
                .eval::<()>(
                    "globalThis.ran = false; Promise.resolve().then(() => globalThis.ran = true);",
                )
                .unwrap();
            cx.update(|window, cx| {
                let sink: &dyn EventSink = &dispatcher;
                match event {
                    "keydown" => sink.key_input(&key, false, &EventPayload::None, window, cx),
                    "wheel" => sink.pointer_input(event, node, &wheel_at(1.0, 1.0), window, cx),
                    _ => sink.pointer_input(event, node, &mouse_at(1.0, 1.0), window, cx),
                }
            });
            assert!(ran(&dispatcher), "a heard {event} drains");
        }
    }

    #[gpui::test]
    fn cancel_bubble_reads_a_stop_requested_after_the_walk(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        let record = "(e) => { out.push(e.type); globalThis.kept = e; }";
        listen(&dispatcher, child, "click", record);
        fire_and_read(cx, &dispatcher, child, "click", &EventPayload::None);

        let state = dispatcher
            .engine
            .eval::<String>(
                r"
                kept.stopPropagation();
                String(kept.cancelBubble)
                ",
            )
            .unwrap();

        assert_eq!(state, "true");
    }

    #[gpui::test]
    fn a_script_that_patches_the_builtins_cannot_reach_or_break_event_state(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = host.borrow_mut().tree.create_node("div").unwrap();
        dispatcher
            .engine
            .eval::<()>(
                r"
                Object.defineProperty(Object.prototype, 'isTrusted', {
                    get() {
                        return 'inherited';
                    },
                    set(value) {
                        globalThis.caught = value;
                    },
                    configurable: true,
                });
                Object.defineProperty = () => {
                    throw new Error('defineProperty');
                };
                Reflect.apply = () => {
                    throw new Error('apply');
                };
                ",
            )
            .unwrap();
        listen(
            &dispatcher,
            node,
            "click",
            r"
            (e) => {
                e.preventDefault();
                out.push(e.defaultPrevented, e.isTrusted, globalThis.caught === undefined);
            }
            ",
        );

        let out = fire_and_read(cx, &dispatcher, node, "click", &EventPayload::None);

        assert_eq!(out, serde_json::json!([true, true, true]));
    }

    #[inca_test_macros::gpui_cases]
    #[gpui::test]
    #[case::hover_rule(".a:hover { width: 1px }", true)]
    #[case::focus_rule(".a:focus { width: 1px }", false)]
    fn tracks_pointer_when_a_rule_names_hover_or_active(
        _cx: &mut TestAppContext,
        #[case] css: &str,
        #[case] expected: bool,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        crate::tests::add_sheet(&host, css);

        assert_eq!(dispatcher.tracks_pointer(), expected);
    }
}
