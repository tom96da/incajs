// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reports the hovered container and fires the hover events, and redraws the window when a
//! pointer or focus state change reaches a style rule.

use gpui::{App, Window};
use inca_document::{EventPayload, MousePayload, NO_BUTTON};
use inca_gpui::mouse_at;
use inca_node::NodeId;

use crate::dispatch::EventDispatcher;
use crate::pointer;

/// The containers a pointer leaves and enters when its deepest container
/// changes from the end of `old_path` to the end of `new_path`. Each path
/// lists a container and its ancestors, nearest first. The first list holds
/// the containers only `old_path` has, innermost first. The second holds the
/// ones only `new_path` has, outermost first.
#[must_use]
fn hover_path_difference(old_path: &[NodeId], new_path: &[NodeId]) -> (Vec<NodeId>, Vec<NodeId>) {
    let leave = old_path
        .iter()
        .copied()
        .filter(|id| !new_path.contains(id))
        .collect();
    let enter = new_path
        .iter()
        .rev()
        .copied()
        .filter(|id| !old_path.contains(id))
        .collect();
    (leave, enter)
}

impl EventDispatcher {
    /// The deepest container `gpui` reports as hovered. Containers that left
    /// the document drop out of the set.
    pub(super) fn deepest_hovered(&self) -> Option<NodeId> {
        let mut set = self.state.hovered.borrow_mut();
        set.retain(|id| self.cells.document.borrow().tree().is_connected(*id));
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
        let physical = self.state.bar_drag.get().or(physical);
        let path = physical.map(|id| self.path_from(id)).unwrap_or_default();
        let changed = self.cells.document.borrow_mut().set_hover_path(&path);
        self.restyle(changed, true, window);
        let mouse = mouse_at(window.mouse_position(), window.modifiers());
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
        // A pointer over a removed root leaves the document alone in the old path. It sends no
        // event.
        if next.is_none() && old_path == [self.cells.document.borrow().document()] {
            self.state.hover_prev.replace(old_path);
            return;
        }
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

impl EventDispatcher {
    /// Redraws the window when `changed` and a connected rule names the pointer state (`pointer`)
    /// or the focus state. Returns whether it requested the redraw.
    pub(crate) fn restyle(&self, changed: bool, pointer: bool, window: &mut Window) -> bool {
        let document = self.cells.document.borrow();
        let redraw = changed
            && if pointer {
                document.style.uses_pointer_state(document.tree())
            } else {
                document.style.uses_focus_state(document.tree())
            };
        if redraw {
            window.refresh();
        }
        redraw
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::rc::Rc;

    use super::hover_path_difference;
    use crate::EventDispatcher;
    use crate::tests::{
        FakeScript, Walked, add_sheet, fake_dispatcher, node_under_root, state, tree_of_three,
    };
    use gpui::TestAppContext;
    use inca_document::{EventPayload, MousePayload};
    use inca_gpui::EventSink as _;
    use inca_node::NodeId;
    use inca_test_macros::gpui_cases;

    /// A dispatcher on a fake script.
    struct Rig {
        dispatcher: EventDispatcher,
        fake: Rc<FakeScript>,
    }

    fn rig() -> Rig {
        let (dispatcher, _host, fake) = fake_dispatcher();
        Rig { dispatcher, fake }
    }

    /// `type@target/relatedTarget`, with `x` for a missing related target.
    fn label(walked: &Walked) -> String {
        let related = walked
            .related()
            .map_or_else(|| "x".to_owned(), |node| node.to_string());
        format!("{}@{}/{related}", walked.name, walked.target_text())
    }

    impl Rig {
        /// The walks of the mouse events since the last call.
        fn mouse_walks(&self) -> Vec<Walked> {
            let mut walks = self.fake.take();
            walks.retain(|walked| walked.name.starts_with("mouse"));
            walks
        }

        fn labels(&self) -> Vec<String> {
            self.mouse_walks().iter().map(label).collect()
        }

        fn hover(&self, cx: &mut TestAppContext, next: Option<NodeId>) -> Vec<String> {
            let cx = cx.add_empty_window();
            cx.update(|window, cx| self.dispatcher.report_hover(next, window, cx));
            self.labels()
        }
    }

    /// `a` holds `b`, which holds `c` and `d`.
    fn hover_tree() -> (Rig, [NodeId; 4]) {
        let rig = rig();
        let [a, b, c, d] = {
            let mut h = rig.dispatcher.cells.document.borrow_mut();
            let ids = [(); 4].map(|()| h.create_node("div").unwrap());
            h.append_child(&*rig.dispatcher.cells.window.borrow(), ids[0], ids[1])
                .unwrap();
            h.append_child(&*rig.dispatcher.cells.window.borrow(), ids[1], ids[2])
                .unwrap();
            h.append_child(&*rig.dispatcher.cells.window.borrow(), ids[1], ids[3])
                .unwrap();
            ids
        };
        (rig, [a, b, c, d])
    }

    #[gpui::test]
    fn entering_a_nested_node_fires_over_then_enter_on_each_ancestor(cx: &mut TestAppContext) {
        let (rig, [a, b, c, _]) = hover_tree();

        let got = rig.hover(cx, Some(c));

        assert_eq!(
            got,
            [
                format!("mouseover@{c}/x"),
                format!("mouseenter@{a}/x"),
                format!("mouseenter@{b}/x"),
                format!("mouseenter@{c}/x"),
            ]
        );
    }

    #[gpui::test]
    fn moving_to_a_sibling_fires_out_leave_over_enter_with_related_targets(
        cx: &mut TestAppContext,
    ) {
        let (rig, [_, _, c, d]) = hover_tree();
        rig.hover(cx, Some(c));

        let got = rig.hover(cx, Some(d));

        assert_eq!(
            got,
            [
                format!("mouseout@{c}/{d}"),
                format!("mouseleave@{c}/{d}"),
                format!("mouseover@{d}/{c}"),
                format!("mouseenter@{d}/{c}"),
            ]
        );
    }

    #[gpui::test]
    fn leaving_everything_fires_out_then_leave_from_the_inside_out(cx: &mut TestAppContext) {
        let (rig, [a, b, _, d]) = hover_tree();
        rig.hover(cx, Some(d));

        let got = rig.hover(cx, None);

        assert_eq!(
            got,
            [
                format!("mouseout@{d}/x"),
                format!("mouseleave@{d}/x"),
                format!("mouseleave@{b}/x"),
                format!("mouseleave@{a}/x"),
            ]
        );
    }

    #[gpui::test]
    fn reporting_the_same_hover_again_fires_nothing(cx: &mut TestAppContext) {
        let (rig, [_, _, c, _]) = hover_tree();
        rig.hover(cx, Some(c));

        let got = rig.hover(cx, Some(c));

        assert_eq!(got, Vec::<String>::new());
    }

    #[gpui::test]
    fn entering_the_root_fires_mouseenter_at_the_document_then_the_root(cx: &mut TestAppContext) {
        let rig = rig();
        let node = node_under_root(&rig.dispatcher.cells);
        let (root, document) = {
            let cells = rig.dispatcher.cells.document.borrow();
            (cells.root(), cells.document())
        };

        let got = rig.hover(cx, Some(node));

        assert_eq!(
            got,
            [
                format!("mouseover@{node}/x"),
                format!("mouseenter@{document}/x"),
                format!("mouseenter@{root}/x"),
                format!("mouseenter@{node}/x"),
            ]
        );
    }

    #[gpui::test]
    fn leaving_a_node_under_the_host_root_gives_the_root_as_the_related_target(
        cx: &mut TestAppContext,
    ) {
        let rig = rig();
        let node = node_under_root(&rig.dispatcher.cells);
        let root = rig.dispatcher.cells.document.borrow().root();
        rig.hover(cx, Some(node));

        let got = rig.hover(cx, Some(root));

        assert_eq!(
            got.iter()
                .filter(|row| row.starts_with("mouseout") || row.starts_with("mouseleave"))
                .collect::<Vec<_>>(),
            [
                &format!("mouseout@{node}/{root}"),
                &format!("mouseleave@{node}/{root}"),
            ]
        );
    }

    #[test]
    fn a_click_targets_the_nearest_common_ancestor_of_press_and_release() {
        let (dispatcher, cells, _fake) = fake_dispatcher();
        let [outer, parent, child] = tree_of_three(&cells);
        let other = {
            let mut h = cells.document.borrow_mut();
            let other = h.create_node("div").unwrap();
            h.append_child(&*cells.window.borrow(), parent, other)
                .unwrap();
            other
        };
        assert_eq!(dispatcher.common_ancestor(child, other), Some(parent));
        assert_eq!(dispatcher.common_ancestor(child, parent), Some(parent));
        assert_eq!(dispatcher.common_ancestor(child, child), Some(child));
        assert_eq!(dispatcher.common_ancestor(child, outer), Some(outer));
    }

    #[gpui::test]
    fn a_release_clicks_only_when_both_containers_share_an_ancestor(cx: &mut TestAppContext) {
        let (dispatcher, cells, fake) = fake_dispatcher();
        let [_, parent, a] = tree_of_three(&cells);
        let (b, apart) = {
            let mut h = cells.document.borrow_mut();
            let b = h.create_node("div").unwrap();
            h.append_child(&*cells.window.borrow(), parent, b).unwrap();
            (b, h.create_node("div").unwrap())
        };
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
            let walks = fake.take();
            walks
                .iter()
                .filter(|walked| walked.name == "click")
                .map(Walked::target_text)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            clicked(Some(a), b),
            [parent.to_string()],
            "siblings share the parent"
        );
        assert!(clicked(Some(a), apart).is_empty(), "no common ancestor");
        assert!(clicked(None, a).is_empty(), "no press container");

        drop(
            cells
                .document
                .borrow_mut()
                .destroy_node(&mut *cells.window.borrow_mut(), b),
        );
        assert!(clicked(Some(a), b).is_empty(), "the release node is gone");
        assert!(clicked(Some(b), a).is_empty(), "the press node is gone");
    }

    #[gpui::test]
    fn a_removed_node_is_no_related_target(cx: &mut TestAppContext) {
        let rig = rig();
        let [outer, parent, _] = tree_of_three(&rig.dispatcher.cells);
        {
            let mut cells = rig.dispatcher.cells.document.borrow_mut();
            let root = cells.root();
            cells
                .append_child(&*rig.dispatcher.cells.window.borrow(), root, outer)
                .unwrap();
        }
        let cx = cx.add_empty_window();
        let at = |related_target| {
            EventPayload::Mouse(MousePayload {
                related_target,
                ..MousePayload::at(0.0, 0.0, inca_document::Modifiers::none())
            })
        };
        let dispatcher = &rig.dispatcher;

        cx.update(|window, cx| {
            dispatcher.fire(outer, "mouseover", &at(Some(parent)), window, cx);
            dispatcher.fire(outer, "mouseover", &at(Some(99)), window, cx);
            for related_target in [Some(parent), Some(99), None] {
                let payload = EventPayload::Focus { related_target };
                dispatcher.fire(outer, "focus", &payload, window, cx);
            }
        });

        let got: Vec<_> = rig
            .fake
            .take()
            .iter()
            .map(|walked| (walked.name.clone(), walked.related()))
            .collect();
        assert_eq!(
            got,
            [
                ("mouseover".to_owned(), Some(parent)),
                ("mouseover".to_owned(), None),
                ("focus".to_owned(), Some(parent)),
                ("focus".to_owned(), None),
                ("focus".to_owned(), None),
            ]
        );
    }

    #[gpui::test]
    fn a_removed_node_is_no_previous_hover_container(cx: &mut TestAppContext) {
        let rig = rig();
        let cells = Rc::clone(&rig.dispatcher.cells);
        let [outer, parent, child] = tree_of_three(&cells);
        {
            let mut document = cells.document.borrow_mut();
            let root = document.root();
            document
                .append_child(&*cells.window.borrow(), root, outer)
                .unwrap();
        }
        let cx = cx.add_empty_window();
        let boundary = |rig: &Rig| -> Vec<String> {
            let mut walks = rig.fake.take();
            walks.retain(|walked| walked.name == "mouseover" || walked.name == "mouseout");
            walks.iter().map(label).collect()
        };
        cx.update(|window, cx| rig.dispatcher.report_hover(Some(child), window, cx));
        boundary(&rig);
        drop(
            cells
                .document
                .borrow_mut()
                .destroy_node(&mut *cells.window.borrow_mut(), child),
        );
        cx.update(|window, cx| rig.dispatcher.report_hover(Some(parent), window, cx));
        assert_eq!(boundary(&rig), Vec::<String>::new());
        cx.update(|window, cx| rig.dispatcher.report_hover(Some(outer), window, cx));
        assert_eq!(
            boundary(&rig),
            [
                format!("mouseout@{parent}/{outer}"),
                format!("mouseover@{outer}/{parent}")
            ]
        );

        // A detached deepest container gets no mouseout.
        cx.update(|window, cx| rig.dispatcher.report_hover(Some(parent), window, cx));
        boundary(&rig);
        cells
            .document
            .borrow_mut()
            .remove_child(&*cells.window.borrow(), outer, parent)
            .unwrap();
        cx.update(|window, cx| rig.dispatcher.report_hover(None, window, cx));
        assert_eq!(boundary(&rig), Vec::<String>::new());
    }

    /// [`hover_tree`] with `a` attached under the cells root.
    fn connected_hover_tree() -> (Rig, [NodeId; 4]) {
        let (rig, ids) = hover_tree();
        {
            let mut cells = rig.dispatcher.cells.document.borrow_mut();
            let root = cells.root();
            cells
                .append_child(&*rig.dispatcher.cells.window.borrow(), root, ids[0])
                .unwrap();
        }
        (rig, ids)
    }

    fn hovered(rig: &Rig, ids: [NodeId; 4]) -> [bool; 4] {
        let cells = rig.dispatcher.cells.document.borrow();
        ids.map(|id| state(&cells, id, ":hover"))
    }

    #[gpui::test]
    fn a_hover_report_marks_the_node_and_its_ancestors(cx: &mut TestAppContext) {
        let (rig, ids) = connected_hover_tree();
        rig.hover(cx, Some(ids[2]));

        assert_eq!(hovered(&rig, ids)[..3], [true, true, true]);
    }

    #[gpui::test]
    fn a_hover_report_leaves_a_sibling_unmarked(cx: &mut TestAppContext) {
        let (rig, ids) = connected_hover_tree();
        rig.hover(cx, Some(ids[2]));

        assert!(!hovered(&rig, ids)[3]);
    }

    #[gpui::test]
    fn a_hover_report_of_none_clears_the_hover_state(cx: &mut TestAppContext) {
        let (rig, ids) = connected_hover_tree();
        rig.hover(cx, Some(ids[2]));
        rig.hover(cx, None);

        assert_eq!(hovered(&rig, ids), [false; 4]);
    }

    #[gpui::test]
    fn a_hover_report_that_fires_nothing_still_rewrites_the_path(cx: &mut TestAppContext) {
        let (rig, [a, b, c, d]) = connected_hover_tree();
        rig.hover(cx, Some(c));
        rig.dispatcher
            .cells
            .document
            .borrow_mut()
            .remove_child(&*rig.dispatcher.cells.window.borrow(), b, c)
            .unwrap();
        rig.hover(cx, Some(b));
        rig.dispatcher
            .cells
            .document
            .borrow_mut()
            .insert_before(&*rig.dispatcher.cells.window.borrow(), b, c, None)
            .unwrap();

        assert_eq!(hovered(&rig, [a, b, c, d]), [true, true, false, false]);
    }

    #[gpui::test]
    fn the_mouseover_walk_sees_the_hover_state_as_true(cx: &mut TestAppContext) {
        let (rig, [_, _, c, _]) = connected_hover_tree();
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let (cells, log) = (Rc::clone(&rig.dispatcher.cells), Rc::clone(&seen));
        rig.fake.on_walk(move |walked| {
            if walked.name == "mouseover" {
                log.borrow_mut()
                    .push(state(&cells.document.borrow(), c, ":hover"));
            }
            false
        });

        rig.hover(cx, Some(c));

        assert_eq!(*seen.borrow(), [true]);
    }

    #[gpui::test]
    fn a_hovered_node_moved_inside_its_parent_keeps_the_hover_state(cx: &mut TestAppContext) {
        let (rig, [a, b, c, d]) = connected_hover_tree();
        rig.hover(cx, Some(c));
        {
            let cells = &rig.dispatcher.cells;
            let view = cells.window.borrow();
            let mut document = cells.document.borrow_mut();
            document.remove_child(&*view, b, c).unwrap();
            document.insert_before(&*view, b, c, Some(d)).unwrap();
        }

        assert_eq!(hovered(&rig, [a, b, c, d]), [true, true, true, false]);
    }

    #[gpui::test]
    fn a_hovered_node_moved_under_another_parent_keeps_the_hover_state(cx: &mut TestAppContext) {
        let (rig, [a, b, c, d]) = connected_hover_tree();
        rig.hover(cx, Some(c));
        {
            let cells = &rig.dispatcher.cells;
            let view = cells.window.borrow();
            let mut document = cells.document.borrow_mut();
            document.remove_child(&*view, b, c).unwrap();
            document.append_child(&*view, d, c).unwrap();
        }

        assert_eq!(hovered(&rig, [a, b, c, d]), [true, true, true, false]);
    }

    #[gpui::test]
    fn leaving_the_window_fires_mouseleave_at_the_root_then_the_document(cx: &mut TestAppContext) {
        let rig = rig();
        let node = node_under_root(&rig.dispatcher.cells);
        let (root, document) = {
            let cells = rig.dispatcher.cells.document.borrow();
            (cells.root(), cells.document())
        };
        rig.hover(cx, Some(node));

        let got = rig.hover(cx, None);

        assert_eq!(
            got,
            [
                format!("mouseout@{node}/x"),
                format!("mouseleave@{node}/x"),
                format!("mouseleave@{root}/x"),
                format!("mouseleave@{document}/x"),
            ]
        );
    }

    /// The pointer rests over `c` of the connected tree.
    fn resting_pointer() -> (Rig, [NodeId; 4]) {
        let (rig, ids) = connected_hover_tree();
        rig.dispatcher.state.hovered.borrow_mut().insert(ids[2]);
        (rig, ids)
    }

    /// The events of every kind that one hover report fires.
    fn report_deepest(cx: &mut TestAppContext, rig: &Rig) -> Vec<String> {
        let cx = cx.add_empty_window();
        let dispatcher = &rig.dispatcher;
        cx.update(|window, cx| dispatcher.report_hover(dispatcher.deepest_hovered(), window, cx));
        rig.fake.take().iter().map(label).collect()
    }

    fn remove_the_root(dispatcher: &EventDispatcher) {
        let cells = &mut *dispatcher.cells.document.borrow_mut();
        let (document, root) = (cells.document(), cells.root());
        cells
            .remove_child(&*dispatcher.cells.window.borrow(), document, root)
            .unwrap();
    }

    #[gpui::test]
    fn removing_the_root_under_the_pointer_fires_no_event(cx: &mut TestAppContext) {
        let (rig, _ids) = resting_pointer();
        report_deepest(cx, &rig);
        remove_the_root(&rig.dispatcher);

        let got = report_deepest(cx, &rig);

        assert_eq!(got, Vec::<String>::new());
    }

    #[gpui::test]
    fn appending_the_root_back_under_the_pointer_fires_no_enter_at_the_document(
        cx: &mut TestAppContext,
    ) {
        let (rig, [_, _, c, _]) = resting_pointer();
        report_deepest(cx, &rig);
        remove_the_root(&rig.dispatcher);
        report_deepest(cx, &rig);
        rig.dispatcher.state.hovered.borrow_mut().insert(c);
        {
            let cells = &mut *rig.dispatcher.cells.document.borrow_mut();
            let (document, root) = (cells.document(), cells.root());
            cells
                .append_child(&*rig.dispatcher.cells.window.borrow(), document, root)
                .unwrap();
        }
        let document = rig.dispatcher.cells.document.borrow().document();

        let got = report_deepest(cx, &rig);

        let at_document = format!("enter@{document}/");
        assert_eq!(
            got.iter()
                .filter(|row| row.contains(&at_document))
                .collect::<Vec<_>>(),
            Vec::<&String>::new()
        );
    }

    #[gpui::test]
    fn an_inner_node_removed_under_the_pointer_leaves_its_live_ancestors(cx: &mut TestAppContext) {
        let (rig, [a, b, c, _]) = resting_pointer();
        let (root, document) = {
            let cells = rig.dispatcher.cells.document.borrow();
            (cells.root(), cells.document())
        };
        report_deepest(cx, &rig);
        rig.dispatcher
            .cells
            .document
            .borrow_mut()
            .remove_child(&*rig.dispatcher.cells.window.borrow(), b, c)
            .unwrap();
        rig.dispatcher.state.hovered.borrow_mut().clear();

        let got = report_deepest(cx, &rig);

        let leave = |node| format!("mouseleave@{node}/x");
        assert_eq!(
            got.iter()
                .filter(|row| row.starts_with("mouseleave"))
                .collect::<Vec<_>>(),
            [leave(b), leave(a), leave(root), leave(document)]
                .iter()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_hover_path_difference_lists_the_containers_left_and_entered() {
        // Paths run nearest first: 3 sits in 2, which sits in 1, then 0.
        let nested = [3, 2, 1, 0];
        let cases: [[&[NodeId]; 4]; 5] = [
            // into a nested container
            [&[0], &nested, &[], &[1, 2, 3]],
            // to a sibling inside the same parent
            [&nested, &[4, 2, 1, 0], &[3], &[4]],
            // out of everything
            [&nested, &[], &[3, 2, 1, 0], &[]],
            // within the same container
            [&nested, &nested, &[], &[]],
            // the first move after entering the window
            [&[], &nested, &[], &[0, 1, 2, 3]],
        ];
        for [old, new, leave, enter] in cases {
            assert_eq!(
                hover_path_difference(old, new),
                (leave.to_vec(), enter.to_vec()),
                "{old:?} -> {new:?}"
            );
        }
    }

    const FOCUS_RULE: &str = "input:focus { width: 60px }";
    const HOVER_RULE: &str = "div:hover { width: 60px }";

    #[gpui_cases]
    #[gpui::test]
    #[case::focus_changed_with_a_focus_rule(true, false, FOCUS_RULE, true)]
    #[case::focus_changed_without_a_rule(true, false, "", false)]
    #[case::focus_unchanged_with_a_focus_rule(false, false, FOCUS_RULE, false)]
    #[case::focus_changed_with_only_a_hover_rule(true, false, HOVER_RULE, false)]
    #[case::pointer_changed_with_a_hover_rule(true, true, HOVER_RULE, true)]
    #[case::pointer_changed_with_only_a_focus_rule(true, true, FOCUS_RULE, false)]
    fn restyle_redraws_only_for_a_changed_value_a_rule_reads(
        cx: &mut TestAppContext,
        #[case] changed: bool,
        #[case] pointer: bool,
        #[case] css: &str,
        #[case] expected: bool,
    ) {
        let (dispatcher, cells, _fake) = fake_dispatcher();
        add_sheet(&cells, css);
        let cx = cx.add_empty_window();

        let redrawn = cx.update(|window, _| dispatcher.restyle(changed, pointer, window));

        assert_eq!(redrawn, expected);
    }
}
