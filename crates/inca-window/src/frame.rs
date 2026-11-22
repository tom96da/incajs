// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The work a window owes before a frame and when it gains or loses focus.

use gpui::{App, Window};
use inca_document::EventPayload;
use inca_gpui::event_sink::with_screen;

use crate::dispatch::{EventDispatcher, observe_focus};
use crate::focus::dispatch_transition;
use inca_document::EventTargetId;

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
            let document = &mut *self.cells.document.borrow_mut();
            let transitions = self.cells.window.borrow_mut().focus.apply_pending(
                &mut document.focus,
                &document.tree,
                &document.style,
                window,
                cx,
            );
            document.sync_focus_style();
            transitions
        };
        for transition in &transitions {
            self.input_focus_moved(transition, window, cx);
            dispatch_transition(transition, self, window, cx);
        }
        self.release_orphans(window, cx);
        self.cells.reveal_focus(window);
        self.fire_scrolls(window, cx);
        // A refresh requested while drawing has no effect, so a listener's own focus changes need
        // the next frame.
        if self.cells.document.borrow().focus.has_queued_work() {
            window.request_animation_frame();
        }
    }

    /// Prints each warning of the sheets parsed since the last frame, prefixed with `<style>: `.
    fn warn_style_sheets(&self) {
        let document = self.cells.document.borrow();
        for line in document.style.take_style_warnings(&document.tree) {
            document.warnings.emit(&format!("<style>: {line}"));
        }
    }

    /// Frees the orphans of the tree and tells the script with
    /// [`Script::release`]. What the script threw is reported and the frame
    /// goes on. Nodes the release leaves orphaned wait for the next frame,
    /// which this requests.
    fn release_orphans(&self, window: &mut Window, cx: &mut App) {
        let swept = self.cells.sweep_orphans();
        let Some((nodes, callback_ids)) = swept else {
            return;
        };
        observe_focus(&self.cells, window, cx);
        let entered = self.script.release(&nodes, &callback_ids);
        if entered {
            self.drain_observed(window);
        }
        // The requests end when a frame leaves no orphan.
        if self.cells.document.borrow().tree.has_orphans() {
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
        observe_focus(&self.cells, window, cx);
        let payload = self.with_held_buttons(event, payload.clone());
        let payload = with_screen(payload, window);
        let (_, entered) = self.walk(target, event, &payload, &[]);
        if entered {
            self.drain_observed(window);
        }
    }

    /// Fires `scroll` at each container whose offset changed since the last
    /// call and that a `scroll` listener reaches. A script write and a wheel
    /// scroll each fire once. Only the containers a listener reaches run the script.
    pub fn fire_scrolls(&self, window: &mut Window, cx: &mut App) {
        let moved = self.cells.scrolled_nodes();
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
    use crate::Cells;
    use crate::tests::{
        FakeScript, Frame, draw, fake_dispatcher, fake_focus_scene, fake_focusable_node, is_focused,
    };
    use gpui::{AppContext as _, TestAppContext};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[gpui::test]
    fn a_frame_prints_the_warnings_of_a_style_sheet_once(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake) = fake_dispatcher();
        let printed = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&printed);
        cells
            .document
            .borrow_mut()
            .warnings
            .set_sink(Rc::new(move |line| sink.borrow_mut().push(line.to_owned())));
        {
            let mut document = cells.document.borrow_mut();
            let root = document.root();
            let style = document.tree.create_node("style").unwrap();
            let text = document.tree.create_node("#text").unwrap();
            document.tree.set_data(text, ".a { bogus: 1 }").unwrap();
            document.tree.append_child(style, text).unwrap();
            document.tree.append_child(root, style).unwrap();
        }
        let window = cx.add_window(|_, _| Frame(Rc::new(dispatcher)));

        draw(cx, window);
        draw(cx, window);

        assert_eq!(
            *printed.borrow(),
            ["<style>: ignoring unknown style key `bogus`"]
        );
    }

    #[gpui::test]
    fn a_frame_writes_the_focus_a_request_asked_for(cx: &mut TestAppContext) {
        let (_dispatcher, cells, _fake, window, node) = fake_focus_scene(cx);
        cells.document.borrow_mut().focus.request_focus(node, true);

        draw(cx, window);

        assert!(is_focused(&cells.document.borrow(), node));
    }

    #[gpui::test]
    fn a_frame_clears_the_focus_of_a_node_that_stops_being_focusable(cx: &mut TestAppContext) {
        let (_dispatcher, cells, _fake, window, node) = fake_focus_scene(cx);
        cells.document.borrow_mut().focus.request_focus(node, true);
        draw(cx, window);
        {
            let document = &mut *cells.document.borrow_mut();
            document.tree.remove_attribute(node, "tabindex").unwrap();
            document.focus.mark_tab_dirty(node);
        }

        draw(cx, window);

        assert!(!is_focused(&cells.document.borrow(), node));
    }

    /// A window over `count` focusable nodes on a [`FakeScript`], settled by one draw.
    struct Settled {
        cells: Rc<Cells>,
        fake: Rc<FakeScript>,
        window: gpui::WindowHandle<Frame>,
        nodes: Vec<u32>,
    }

    fn settled(cx: &mut TestAppContext, count: usize) -> Settled {
        let (_dispatcher, cells, fake, window, first) = fake_focus_scene(cx);
        let nodes: Vec<u32> = std::iter::once(first)
            .chain((1..count).map(|_| fake_focusable_node(&cells)))
            .collect();
        draw(cx, window);
        requested_frames(cx, window);
        Settled {
            cells,
            fake,
            window,
            nodes,
        }
    }

    /// Frame callbacks the last draw asked for.
    fn requested_frames(cx: &mut TestAppContext, window: gpui::WindowHandle<Frame>) -> usize {
        cx.update_window(window.into(), |_, window, cx| {
            window.simulate_next_frame(cx)
        })
        .unwrap()
    }

    impl Settled {
        fn request_focus(&self, node: u32) {
            self.cells
                .document
                .borrow_mut()
                .focus
                .request_focus(node, false);
        }

        fn focused(&self, node: u32) -> bool {
            is_focused(&self.cells.document.borrow(), node)
        }

        /// Makes the `focus` walk at `node` run `act` on the document.
        fn on_focus(&self, node: u32, act: impl Fn(&mut inca_document::DocumentState) + 'static) {
            let cells = Rc::clone(&self.cells);
            self.fake.on_walk(move |walked| {
                if walked.name == "focus" && walked.target_text() == node.to_string() {
                    act(&mut cells.document.borrow_mut());
                }
                false
            });
        }
    }

    #[gpui::test]
    fn a_focus_requested_in_a_focus_listener_applies_on_the_next_frame(cx: &mut TestAppContext) {
        let scene = settled(cx, 2);
        let (first, second) = (scene.nodes[0], scene.nodes[1]);
        scene.on_focus(first, move |cells| cells.focus.request_focus(second, false));
        scene.request_focus(first);

        draw(cx, scene.window);
        assert!(!scene.focused(second));
        assert_eq!(requested_frames(cx, scene.window), 1);
        draw(cx, scene.window);

        assert!(scene.focused(second));
    }

    #[gpui::test]
    fn listeners_focusing_each_other_move_the_focus_once_per_frame(cx: &mut TestAppContext) {
        let scene = settled(cx, 2);
        let (first, second) = (scene.nodes[0], scene.nodes[1]);
        scene.fake.on_walk({
            let cells = Rc::clone(&scene.cells);
            move |walked| {
                let target = walked.target_text();
                if walked.name == "focus" && target == first.to_string() {
                    cells
                        .document
                        .borrow_mut()
                        .focus
                        .request_focus(second, false);
                } else if walked.name == "focus" && target == second.to_string() {
                    cells
                        .document
                        .borrow_mut()
                        .focus
                        .request_focus(first, false);
                }
                false
            }
        });
        scene.request_focus(first);

        draw(cx, scene.window);
        assert!(scene.focused(first));
        assert!(!scene.focused(second));
        // Running a requested frame draws it, and each draw requests one frame in turn.
        assert_eq!(requested_frames(cx, scene.window), 1);
        assert!(scene.focused(second));
        assert_eq!(requested_frames(cx, scene.window), 1);

        assert!(scene.focused(first));
    }

    #[gpui::test]
    fn a_settled_frame_with_no_owed_work_requests_no_frame(cx: &mut TestAppContext) {
        let scene = settled(cx, 1);
        scene.request_focus(scene.nodes[0]);

        draw(cx, scene.window);
        requested_frames(cx, scene.window);
        draw(cx, scene.window);

        assert_eq!(requested_frames(cx, scene.window), 0);
    }

    #[gpui::test]
    fn a_tabindex_removed_in_a_focus_listener_blurs_the_node_on_the_next_frame(
        cx: &mut TestAppContext,
    ) {
        let scene = settled(cx, 1);
        let node = scene.nodes[0];
        scene.on_focus(node, move |cells| {
            cells.tree.remove_attribute(node, "tabindex").unwrap();
            cells.focus.mark_tab_dirty(node);
        });
        scene.request_focus(node);

        draw(cx, scene.window);
        assert!(scene.focused(node));
        assert_eq!(requested_frames(cx, scene.window), 1);
        draw(cx, scene.window);

        assert!(!scene.focused(node));
    }
}
