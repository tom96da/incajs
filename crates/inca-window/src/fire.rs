// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Fires one event along the path of a node and resolves its targets.

use gpui::{App, Window};
use inca_document::flags_of;
use inca_document::{EventPayload, MousePayload, PointerSource};
use inca_gpui::event_sink::with_screen;
use inca_node::{NodeId, is_disabled_control};

use crate::dispatch::{EventDispatcher, observe_focus};
use inca_document::EventTargetId;
use inca_document::Walk;

impl EventDispatcher {
    /// Dispatches `event` along the path of `target`, with `target` as its
    /// target, and reports what the listeners threw. Returns whether a
    /// listener prevented the default action and whether the walk entered the
    /// script. `cut` lists the nodes whose listeners stay silent.
    pub(super) fn walk(
        &self,
        target: EventTargetId,
        event: &str,
        payload: &EventPayload,
        cut: &[NodeId],
    ) -> (bool, bool) {
        let outcome = self.script.walk(&Walk {
            name: event,
            flags: flags_of(
                event,
                match payload {
                    EventPayload::Input(input) => Some(input.input_type),
                    _ => None,
                },
            ),
            target,
            cut,
            payload,
            trusted: true,
        });
        // Reported after the walk: a reporter is free to do anything, and
        // re-entering the script from inside a walk can panic.
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
    /// The script runs the listeners through [`Script::walk`]. A listener that
    /// throws is reported, and the remaining listeners run.
    pub fn fire(
        &self,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if self.cells.document.borrow().tree.get(start).is_none() {
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
        observe_focus(&self.cells, window, cx);
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
        let layout = self.cells.window.borrow().layout_of(target);
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
        let gone = |id: &NodeId| self.cells.document.borrow().tree.get(*id).is_none();
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
        let document = self.cells.document.borrow();
        found
            .filter(|id| document.tree.get(*id).is_some())
            .unwrap_or(node_id)
    }

    /// `target` and its ancestors up to the document, nearest first.
    pub(super) fn path_from(&self, target: NodeId) -> Vec<NodeId> {
        self.cells
            .document
            .borrow()
            .tree
            .ancestors(target)
            .collect()
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
        let document = self.cells.document.borrow();
        let disabled = |id: &NodeId| document.tree.get(*id).is_some_and(is_disabled_control);
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
            let mut document = self.cells.document.borrow_mut();
            let window_state = self.cells.window.borrow();
            let handles = &window_state.focus;
            let focusing = if prevented {
                None
            } else {
                path.iter()
                    .copied()
                    .find(|&id| handles.handle(id).is_some())
            };
            let focused = handles.focused_node(&document.focus, window, cx);
            let top = document.top();
            document
                .focus
                .press_start(target, top, focusing.filter(|&node| Some(node) != focused));
            focusing.and_then(|node| handles.handle(node))
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
        let (top, focused) = {
            let document = self.cells.document.borrow();
            let focused =
                self.cells
                    .window
                    .borrow()
                    .focus
                    .focused_node(&document.focus, window, cx);
            (document.top(), focused)
        };
        self.resolve(focused, top)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::Cells;
    use crate::tests::{FakeScript, Walked, fake_dispatcher, tree_of_three, wheel_at};
    use gpui::{TestAppContext, point, px};
    use inca_document::{EventPayload, MousePayload};
    use inca_node::NodeId;

    use std::rc::Rc;

    #[gpui::test]
    fn tracks_pointer_only_with_a_listener_or_a_focusable_node(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake) = fake_dispatcher();
        let node = cells.document.borrow_mut().tree.create_node("div").unwrap();
        let root = cells.document.borrow().root();
        cells
            .document
            .borrow_mut()
            .tree
            .append_child(root, node)
            .unwrap();
        assert!(!dispatcher.tracks_pointer());

        cells.document.borrow_mut().listeners.insert(
            inca_document::EventTargetId::Node(node),
            "keydown",
            1,
            false,
            false,
        );
        assert!(dispatcher.tracks_pointer());
        cells.document.borrow_mut().listeners.remove(
            inca_document::EventTargetId::Node(node),
            "keydown",
            1,
        );
        assert!(!dispatcher.tracks_pointer());

        cells
            .document
            .borrow_mut()
            .tree
            .set_attribute(node, "tabindex", "0")
            .unwrap();
        cells.document.borrow_mut().focus.mark_tab_dirty(node);
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            let _ = crate::tests::apply_focus(&cells, window, cx);
        });
        assert!(dispatcher.tracks_pointer());
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
        let (dispatcher, cells, _fake) = fake_dispatcher();
        crate::tests::add_sheet(&cells, css);

        assert_eq!(dispatcher.tracks_pointer(), expected);
    }

    fn record_box(cells: &Rc<Cells>, node: NodeId, left: f32, top: f32, border: f32) {
        cells.window.borrow().layout.borrow_mut().insert(
            node,
            inca_gpui::NodeLayout {
                bounds: gpui::Bounds::new(
                    point(px(left), px(top)),
                    gpui::size(px(100.0), px(50.0)),
                ),
                border: px(border),
                scroll: gpui::Point::default(),
                gutter: gpui::Size::default(),
            },
        );
    }

    /// Fires `event` at `start` on a fake script and returns the walks it recorded.
    fn fire_walked(
        cx: &mut TestAppContext,
        dispatcher: &crate::EventDispatcher,
        fake: &FakeScript,
        start: NodeId,
        event: &str,
        payload: &EventPayload,
    ) -> Vec<Walked> {
        let cx = cx.add_empty_window();
        cx.update(|window, cx| {
            dispatcher.fire(start, event, payload, window, cx);
        });
        fake.take()
    }

    fn offsets(walked: &Walked) -> (f32, f32, f32, f32) {
        let mouse = walked.mouse();
        (
            mouse.offset_x,
            mouse.offset_y,
            mouse.client_x,
            mouse.client_y,
        )
    }

    fn click_at_60_40() -> EventPayload {
        EventPayload::Mouse(MousePayload::at(
            60.0,
            40.0,
            inca_document::Modifiers::none(),
        ))
    }

    #[gpui::test]
    fn offset_falls_back_to_the_client_position_when_unrecorded(cx: &mut TestAppContext) {
        let (dispatcher, cells, fake) = fake_dispatcher();
        let [_, _, child] = tree_of_three(&cells);

        let walks = fire_walked(cx, &dispatcher, &fake, child, "click", &click_at_60_40());

        assert_eq!(walks.len(), 1);
        assert_eq!(offsets(&walks[0]), (60.0, 40.0, 60.0, 40.0));
    }

    #[gpui::test]
    fn the_offset_is_measured_from_the_target_not_from_an_ancestor(cx: &mut TestAppContext) {
        let (dispatcher, cells, fake) = fake_dispatcher();
        let [_, parent, child] = tree_of_three(&cells);
        record_box(&cells, parent, 500.0, 500.0, 9.0);
        record_box(&cells, child, 10.0, 20.0, 2.0);

        let walks = fire_walked(cx, &dispatcher, &fake, child, "click", &click_at_60_40());

        assert_eq!(walks.len(), 1);
        assert_eq!(walks[0].target_text(), child.to_string());
        assert_eq!(offsets(&walks[0]), (48.0, 18.0, 60.0, 40.0));
    }

    #[gpui::test]
    fn hover_events_measure_the_offset_from_the_node_they_fire_at(cx: &mut TestAppContext) {
        let (dispatcher, cells, fake) = fake_dispatcher();
        let [outer, parent, child] = tree_of_three(&cells);
        record_box(&cells, parent, 10.0, 20.0, 2.0);
        record_box(&cells, child, 30.0, 40.0, 0.0);

        // The pointer is over `child`: `mouseover` fires there and then `mouseenter` fires at
        // each node of the path. The pointer is at (0,0) in the test window.
        let cx = cx.add_empty_window();
        cx.update(|window, cx| dispatcher.report_hover(Some(child), window, cx));

        let rows: Vec<_> = fake
            .take()
            .iter()
            .filter(|walked| walked.name.starts_with("mouse"))
            .map(|walked| (walked.label(), offsets(walked)))
            .collect();
        assert_eq!(
            rows,
            [
                (format!("mouseover@{child}"), (-30.0, -40.0, 0.0, 0.0)),
                (format!("mouseenter@{outer}"), (0.0, 0.0, 0.0, 0.0)),
                (format!("mouseenter@{parent}"), (-12.0, -22.0, 0.0, 0.0)),
                (format!("mouseenter@{child}"), (-30.0, -40.0, 0.0, 0.0)),
            ]
        );
    }

    #[gpui::test]
    fn a_wheel_event_carries_the_offset(cx: &mut TestAppContext) {
        let (dispatcher, cells, fake) = fake_dispatcher();
        let [_, _, child] = tree_of_three(&cells);
        record_box(&cells, child, 10.0, 20.0, 0.0);

        let walks = fire_walked(
            cx,
            &dispatcher,
            &fake,
            child,
            "wheel",
            &wheel_at(60.0, 40.0),
        );

        assert_eq!(walks.len(), 1);
        let EventPayload::Wheel(wheel) = &walks[0].payload else {
            panic!("expected a wheel payload");
        };
        assert_eq!((wheel.mouse.offset_x, wheel.mouse.offset_y), (50.0, 20.0));
    }
}
