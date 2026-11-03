// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reports the hovered container and fires the hover events.

use gpui::{App, Window};
use inca_gpui::event_sink::hover_path_difference;
use inca_gpui::{EventPayload, MousePayload, NO_BUTTON, NodeId};

use super::{EventDispatcher, pointer};

impl EventDispatcher {
    /// The deepest container `gpui` reports as hovered. Containers that left
    /// the tree drop out of the set.
    pub(super) fn deepest_hovered(&self) -> Option<NodeId> {
        let mut set = self.state.hovered.borrow_mut();
        set.retain(|id| self.host.borrow().tree.get(*id).is_some());
        set.iter()
            .copied()
            .max_by_key(|id| (self.path_from(*id).len(), *id))
    }

    /// Queues the hover report for the end of the update, once. It reports the
    /// hovered set unless the update already reported its deepest container.
    pub(super) fn schedule_hover_report(&self, window: &mut Window, cx: &mut App) {
        if self.state.hover_scheduled.replace(true) {
            return;
        }
        let this = self.clone();
        window.defer(cx, move |window, cx| {
            this.state.hover_scheduled.set(false);
            if !this.state.hover_settled.replace(false) {
                this.report_hover(this.deepest_hovered(), window, cx);
            }
        });
    }

    /// [`Self::report_hover_with`] for a move, which is about no button.
    pub(crate) fn report_hover(&self, next: Option<NodeId>, window: &mut Window, cx: &mut App) {
        self.report_hover_with(next, NO_BUTTON, window, cx);
    }

    /// Fires the hover events for the pointer moving from the deepest
    /// container reported last time to the one under it now, or to the capturing
    /// node while a capture is active. The pointer events come first, then the
    /// mouse ones. Each group is `out` at the old container, `leave` at each
    /// container left (innermost first), `over` at the new one and `enter` at
    /// each container entered (outermost first). The pointer events are about
    /// `button`.
    pub(super) fn report_hover_with(
        &self,
        physical: Option<NodeId>,
        button: u8,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mouse = MousePayload::at(window.mouse_position(), window.modifiers());
        self.process_capture(&EventPayload::Mouse(mouse), button, window, cx);
        let next = self.hover_target(physical);
        let next_path = next.map(|id| self.path_from(id)).unwrap_or_default();
        // A removed container is replaced by its nearest live ancestor.
        let old_path = self.state.hover_prev.replace(next_path.clone());
        // A container is live while it still hangs below the old path's top.
        let top = old_path.last().copied();
        let live = |id: &NodeId| self.path_from(*id).last().copied() == top;
        let removed = old_path.first().is_some_and(|id| !live(id));
        let old_path: Vec<_> = old_path.into_iter().skip_while(|id| !live(id)).collect();
        let prev = old_path.first().copied();
        if prev == next {
            return;
        }
        let (leave, enter) = hover_path_difference(&old_path, &next_path);
        let with = |related_target| {
            EventPayload::Mouse(MousePayload {
                related_target,
                ..mouse
            })
        };
        let pointer = |related| pointer::pointer_payload(&with(related), button);
        // A removed container receives no `out`.
        let old = prev.filter(|_| !removed);
        if let Some(old) = old {
            self.fire(old, "pointerout", &pointer(next), window, cx);
        }
        for &id in &leave {
            self.fire(id, "pointerleave", &pointer(next), window, cx);
        }
        if let Some(new) = next {
            self.fire(new, "pointerover", &pointer(prev), window, cx);
        }
        for &id in &enter {
            self.fire(id, "pointerenter", &pointer(prev), window, cx);
        }
        if let Some(old) = old {
            self.fire(old, "mouseout", &with(next), window, cx);
        }
        for id in leave {
            self.fire(id, "mouseleave", &with(next), window, cx);
        }
        if let Some(new) = next {
            self.fire(new, "mouseover", &with(prev), window, cx);
        }
        for id in enter {
            self.fire(id, "mouseenter", &with(prev), window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{
        RECORDER, at, dispatcher_with_engine, fire_and_read, listen, listen_global,
        node_under_root, read_out, seen, seen_rows, tree_of_three, wheel_at,
    };
    use crate::{EventDispatcher, Host};
    use gpui::{TestAppContext, point, px};
    use inca_gpui::{EventPayload, EventSink as _, MousePayload, NodeId};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// `a` holds `b`, which holds `c` and `d`. Every hover event is recorded as
    /// `type@currentTarget/target/relatedTarget` in `globalThis.seen`, with `x` for a missing
    /// related target.
    fn hover_tree() -> (EventDispatcher, [NodeId; 4]) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [a, b, c, d] = {
            let mut h = host.borrow_mut();
            let ids = [(); 4].map(|()| h.tree.create_node("div").unwrap());
            h.tree.append_child(ids[0], ids[1]).unwrap();
            h.tree.append_child(ids[1], ids[2]).unwrap();
            h.tree.append_child(ids[1], ids[3]).unwrap();
            ids
        };
        dispatcher
            .engine
            .eval::<()>("globalThis.seen = [];")
            .unwrap();
        for node in [a, b, c, d] {
            for event in ["mouseover", "mouseout", "mouseenter", "mouseleave"] {
                listen(
                    &dispatcher,
                    node,
                    event,
                    "(e) => seen.push(`${e.type}@${e.currentTarget}/${e.target}/${e.relatedTarget ?? 'x'}`)",
                );
            }
        }
        (dispatcher, [a, b, c, d])
    }

    fn hover(
        cx: &mut TestAppContext,
        dispatcher: &EventDispatcher,
        next: Option<NodeId>,
    ) -> Vec<String> {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| dispatcher.report_hover(next, window, cx));
        serde_json::from_value(seen(dispatcher)).unwrap()
    }

    #[gpui::test]
    fn entering_a_nested_node_fires_over_then_enter_on_each_ancestor(cx: &mut TestAppContext) {
        let (dispatcher, [a, b, c, _]) = hover_tree();

        let got = hover(cx, &dispatcher, Some(c));

        assert_eq!(
            got,
            [
                format!("mouseover@{c}/{c}/x"),
                format!("mouseover@{b}/{c}/x"),
                format!("mouseover@{a}/{c}/x"),
                format!("mouseenter@{a}/{a}/x"),
                format!("mouseenter@{b}/{b}/x"),
                format!("mouseenter@{c}/{c}/x"),
            ]
        );
    }

    #[gpui::test]
    fn moving_to_a_sibling_fires_out_leave_over_enter_with_related_targets(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, [a, b, c, d]) = hover_tree();
        hover(cx, &dispatcher, Some(c));

        let got = hover(cx, &dispatcher, Some(d));

        assert_eq!(
            got,
            [
                format!("mouseout@{c}/{c}/{d}"),
                format!("mouseout@{b}/{c}/{d}"),
                format!("mouseout@{a}/{c}/{d}"),
                format!("mouseleave@{c}/{c}/{d}"),
                format!("mouseover@{d}/{d}/{c}"),
                format!("mouseover@{b}/{d}/{c}"),
                format!("mouseover@{a}/{d}/{c}"),
                format!("mouseenter@{d}/{d}/{c}"),
            ]
        );
    }

    #[gpui::test]
    fn leaving_everything_fires_out_then_leave_from_the_inside_out(cx: &mut TestAppContext) {
        let (dispatcher, [a, b, _, d]) = hover_tree();
        hover(cx, &dispatcher, Some(d));

        let got = hover(cx, &dispatcher, None);

        assert_eq!(
            got,
            [
                format!("mouseout@{d}/{d}/x"),
                format!("mouseout@{b}/{d}/x"),
                format!("mouseout@{a}/{d}/x"),
                format!("mouseleave@{d}/{d}/x"),
                format!("mouseleave@{b}/{b}/x"),
                format!("mouseleave@{a}/{a}/x"),
            ]
        );
    }

    #[gpui::test]
    fn reporting_the_same_hover_again_fires_nothing(cx: &mut TestAppContext) {
        let (dispatcher, [_, _, c, _]) = hover_tree();
        hover(cx, &dispatcher, Some(c));

        let got = hover(cx, &dispatcher, Some(c));

        assert_eq!(got, Vec::<String>::new());
    }

    #[gpui::test]
    fn entering_a_node_under_the_host_root_fires_mouseenter_at_the_root_and_not_at_document(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = node_under_root(&host);
        let root = host.borrow().root();
        dispatcher
            .engine
            .eval::<()>("globalThis.seen = [];")
            .unwrap();
        let record = "(e) => seen.push(`${e.type}@${e.currentTarget}/${e.target}`)";
        listen(&dispatcher, root, "mouseenter", record);
        listen_global(&dispatcher, "$document", "mouseenter", false, record);

        let got = hover(cx, &dispatcher, Some(node));

        assert_eq!(got, [format!("mouseenter@{root}/{root}")]);
    }

    #[gpui::test]
    fn leaving_a_node_under_the_host_root_gives_the_root_as_the_related_target(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let node = node_under_root(&host);
        let root = host.borrow().root();
        dispatcher
            .engine
            .eval::<()>("globalThis.seen = [];")
            .unwrap();
        let record =
            "(e) => seen.push(`${e.type}@${e.currentTarget}/${e.target}/${e.relatedTarget}`)";
        for target in [node, root] {
            for event in ["mouseout", "mouseleave"] {
                listen(&dispatcher, target, event, record);
            }
        }
        hover(cx, &dispatcher, Some(node));
        dispatcher
            .engine
            .eval::<()>("globalThis.seen = [];")
            .unwrap();

        let got = hover(cx, &dispatcher, Some(root));

        assert_eq!(
            got,
            [
                format!("mouseout@{node}/{node}/{root}"),
                format!("mouseout@{root}/{node}/{root}"),
                format!("mouseleave@{node}/{node}/{root}"),
            ]
        );
    }

    #[test]
    fn a_click_targets_the_nearest_common_ancestor_of_press_and_release() {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        let other = {
            let mut h = host.borrow_mut();
            let other = h.tree.create_node("div").unwrap();
            h.tree.append_child(parent, other).unwrap();
            other
        };
        assert_eq!(dispatcher.common_ancestor(child, other), Some(parent));
        assert_eq!(dispatcher.common_ancestor(child, parent), Some(parent));
        assert_eq!(dispatcher.common_ancestor(child, child), Some(child));
        assert_eq!(dispatcher.common_ancestor(child, outer), Some(outer));
    }

    #[gpui::test]
    fn a_release_clicks_only_when_both_containers_share_an_ancestor(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, a] = tree_of_three(&host);
        let (b, apart) = {
            let mut h = host.borrow_mut();
            let b = h.tree.create_node("div").unwrap();
            h.tree.append_child(parent, b).unwrap();
            (b, h.tree.create_node("div").unwrap())
        };
        dispatcher.engine.eval::<()>(RECORDER).unwrap();
        for node in [parent, a, b, apart] {
            listen(&dispatcher, node, "click", "record");
        }
        let release = gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        };
        let cx = cx.add_empty_window();
        let mut clicked = |pressed, released| {
            cx.update(|window, cx| {
                dispatcher.pointer_released(&release, pressed, released, window, cx);
            });
            seen_rows(&dispatcher).len()
        };

        assert_eq!(clicked(Some(a), b), 1, "siblings share the parent");
        assert_eq!(clicked(Some(a), apart), 0, "no common ancestor");
        assert_eq!(clicked(None, a), 0, "no press container");

        host.borrow_mut().tree.destroy_node(b);
        assert_eq!(clicked(Some(a), b), 0, "the release node is gone");
        assert_eq!(clicked(Some(b), a), 0, "the press node is gone");
    }

    #[gpui::test]
    fn a_removed_node_is_no_related_target(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, _] = tree_of_three(&host);
        {
            let mut host = host.borrow_mut();
            let root = host.root();
            host.tree.append_child(root, outer).unwrap();
        }
        dispatcher
            .engine
            .eval::<()>(
                r"
                globalThis.seen = [];
                globalThis.related = (e) => {
                    globalThis.seen.push([e.type, e.relatedTarget]);
                };
                ",
            )
            .unwrap();
        for event in ["mouseover", "focus"] {
            listen(&dispatcher, outer, event, "related");
        }
        let cx = cx.add_empty_window();
        let at = |related_target| {
            EventPayload::Mouse(MousePayload {
                related_target,
                ..MousePayload::at(gpui::Point::default(), gpui::Modifiers::none())
            })
        };

        cx.update(|window, cx| {
            dispatcher.fire(outer, "mouseover", &at(Some(parent)), window, cx);
            dispatcher.fire(outer, "mouseover", &at(Some(99)), window, cx);
            for related_target in [Some(parent), Some(99), None] {
                let payload = EventPayload::Focus { related_target };
                dispatcher.fire(outer, "focus", &payload, window, cx);
            }
        });
        assert_eq!(
            seen(&dispatcher),
            serde_json::json!([
                ["mouseover", parent],
                ["mouseover", null],
                ["focus", parent],
                ["focus", null],
                ["focus", null]
            ])
        );
    }

    #[gpui::test]
    fn a_removed_node_is_no_previous_hover_container(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [outer, parent, child] = tree_of_three(&host);
        {
            let mut host = host.borrow_mut();
            let root = host.root();
            host.tree.append_child(root, outer).unwrap();
        }
        dispatcher
            .engine
            .eval::<()>(
                r"
                globalThis.seen = [];
                globalThis.related = (e) => {
                    globalThis.seen.push([e.type, e.relatedTarget]);
                };
                ",
            )
            .unwrap();
        for event in ["mouseover", "mouseout"] {
            listen(&dispatcher, outer, event, "related");
        }
        let cx = cx.add_empty_window();
        cx.update(|window, cx| dispatcher.report_hover(Some(child), window, cx));
        seen(&dispatcher);
        host.borrow_mut().tree.destroy_node(child);
        cx.update(|window, cx| dispatcher.report_hover(Some(parent), window, cx));
        assert_eq!(seen(&dispatcher), serde_json::json!([]));
        cx.update(|window, cx| dispatcher.report_hover(Some(outer), window, cx));
        assert_eq!(
            seen(&dispatcher),
            serde_json::json!([["mouseout", outer], ["mouseover", parent]])
        );

        // A detached deepest container gets no mouseout.
        cx.update(|window, cx| dispatcher.report_hover(Some(parent), window, cx));
        seen(&dispatcher);
        host.borrow_mut().tree.remove_child(outer, parent).unwrap();
        cx.update(|window, cx| dispatcher.report_hover(None, window, cx));
        assert_eq!(seen(&dispatcher), serde_json::json!([]));
    }

    fn record_box(host: &Rc<RefCell<Host>>, node: NodeId, left: f32, top: f32, border: f32) {
        host.borrow().layout.borrow_mut().insert(
            node,
            inca_gpui::NodeLayout {
                bounds: gpui::Bounds::new(
                    point(px(left), px(top)),
                    gpui::size(px(100.0), px(50.0)),
                ),
                border: px(border),
                scroll: gpui::Point::default(),
            },
        );
    }

    #[gpui::test]
    fn offset_falls_back_to_the_client_position_when_unrecorded(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            parent,
            "click",
            "(e) => out.push([e.target, e.offsetX, e.offsetY, e.clientX, e.clientY])",
        );
        let click = EventPayload::Mouse(MousePayload::at(at(60.0, 40.0), gpui::Modifiers::none()));

        let out = fire_and_read(cx, &dispatcher, child, "click", &click);
        assert_eq!(out, serde_json::json!([[child, 60, 40, 60, 40]]));
    }

    #[gpui::test]
    fn a_parent_listener_reads_the_targets_offset(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            parent,
            "click",
            "(e) => out.push([e.target, e.offsetX, e.offsetY, e.clientX, e.clientY])",
        );
        let click = EventPayload::Mouse(MousePayload::at(at(60.0, 40.0), gpui::Modifiers::none()));
        record_box(&host, parent, 500.0, 500.0, 9.0);
        record_box(&host, child, 10.0, 20.0, 2.0);
        let out = fire_and_read(cx, &dispatcher, child, "click", &click);
        assert_eq!(out, serde_json::json!([[child, 48, 18, 60, 40]]));
    }

    #[gpui::test]
    fn hover_events_measure_the_offset_from_the_node_they_fire_at(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, parent, child] = tree_of_three(&host);
        for event in ["mouseover", "mouseenter"] {
            listen(
                &dispatcher,
                parent,
                event,
                "(e) => out.push([e.type, e.target, e.offsetX, e.offsetY, e.clientX, e.clientY])",
            );
        }
        record_box(&host, parent, 10.0, 20.0, 2.0);
        record_box(&host, child, 30.0, 40.0, 0.0);

        // The pointer is over `child`: `mouseover` fires there and bubbles to
        // `parent`, then `mouseenter` fires at `parent`.
        let cx = cx.add_empty_window();
        cx.update(|window, cx| dispatcher.report_hover(Some(child), window, cx));

        let out = read_out(&dispatcher);
        // The pointer is at (0,0) in the test window. Each event reads the box
        // of its own target.
        assert_eq!(
            out,
            serde_json::json!([
                ["mouseover", child, -30, -40, 0, 0],
                ["mouseenter", parent, -12, -22, 0, 0]
            ])
        );
    }

    #[gpui::test]
    fn a_wheel_event_carries_the_offset(cx: &mut TestAppContext) {
        let (dispatcher, host, _reported) = dispatcher_with_engine();
        let [_, _, child] = tree_of_three(&host);
        listen(
            &dispatcher,
            child,
            "wheel",
            "(e) => out.push([e.offsetX, e.offsetY])",
        );
        record_box(&host, child, 10.0, 20.0, 0.0);

        let out = fire_and_read(cx, &dispatcher, child, "wheel", &wheel_at(60.0, 40.0));

        assert_eq!(out, serde_json::json!([[50, 20]]));
    }
}
