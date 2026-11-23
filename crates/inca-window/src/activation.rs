// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window gaining or losing focus: the focus events, the modifier keys and the pointer state
//! it resets.

use gpui::{App, Window};
use inca_document::{EventPayload, NO_BUTTON};
use inca_gpui::mouse_at;

use crate::dispatch::EventDispatcher;
use crate::focus::dispatch_transition;
use inca_document::EventTargetId;
use inca_document::FocusTransition;

/// The payload of the window's `focus` and `blur`.
const FOCUS_PAYLOAD: EventPayload = EventPayload::Focus {
    related_target: None,
};

impl EventDispatcher {
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
        self.cells.observe_focus(window, cx);
        let changed = self.cells.document.borrow_mut().set_window_blurred(!active);
        self.restyle(changed, false, window);
        let focused = self.cells.focused_node(window, cx);
        if active {
            self.fire_at_global(EventTargetId::Window, "focus", &FOCUS_PAYLOAD, window, cx);
        }
        if let Some(node) = focused {
            if !active {
                self.input_window_blur(node, window, cx);
            }
            dispatch_transition(
                &FocusTransition::window_activation(node, active),
                self,
                window,
                cx,
            );
        }
        if !active {
            self.fire_at_global(EventTargetId::Window, "blur", &FOCUS_PAYLOAD, window, cx);
        }
    }

    /// Ends the pointer capture with `lostpointercapture`, then forgets the held buttons, the
    /// press state and a canceled `pointerdown`. The release of a button the window never sees
    /// would leave them set.
    fn reset_pointer(&self, window: &mut Window, cx: &mut App) {
        self.cells.document.borrow_mut().cancel_capture_request();
        let payload = EventPayload::Mouse(mouse_at(gpui::Point::default(), window.modifiers()));
        self.process_capture(&payload, NO_BUTTON, window, cx);
        self.state.pointerdown_canceled.set(false);
        self.state.input_drag.set(None);
        {
            let mut window = self.cells.window.borrow_mut();
            window.held_buttons = 0;
            window.capture_active = None;
            self.cells.document.borrow_mut().cancel_capture_request();
        }
        self.clear_press(window);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{FakeScript, Frame, draw, fake_focus_scene, is_focused, set_active, state};
    use crate::{Cells, EventDispatcher};
    use gpui::{AppContext as _, TestAppContext};
    use inca_node::NodeId;

    use std::rc::Rc;

    type Handle = gpui::WindowHandle<Frame>;

    /// The focus events fired since the last call, as `type:target`.
    fn focus_events(fake: &FakeScript) -> Vec<String> {
        fake.take()
            .iter()
            .filter(|walked| {
                ["focus", "blur", "focusin", "focusout"].contains(&walked.name.as_str())
            })
            .map(|walked| format!("{}:{}", walked.name, walked.target_text()))
            .collect()
    }

    /// A window over one focusable node, with the window deactivated.
    fn inactive_scene(
        cx: &mut TestAppContext,
    ) -> (
        Rc<EventDispatcher>,
        Rc<Cells>,
        Rc<FakeScript>,
        Handle,
        NodeId,
    ) {
        let (dispatcher, cells, fake, window, node) = fake_focus_scene(cx);
        draw(cx, window);
        set_active(cx, window, &dispatcher, false);
        fake.take();
        (dispatcher, cells, fake, window, node)
    }

    #[gpui::test]
    fn focusing_in_an_inactive_window_fires_focus_when_it_activates(cx: &mut TestAppContext) {
        let (dispatcher, cells, fake, window, node) = inactive_scene(cx);

        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);
        let held = focus_events(&fake);
        set_active(cx, window, &dispatcher, true);

        assert!(held.is_empty());
        assert_eq!(
            focus_events(&fake),
            [
                "focus:$window".to_owned(),
                format!("focus:{node}"),
                format!("focusin:{node}")
            ]
        );
    }

    #[gpui::test]
    fn focusing_then_blurring_in_an_inactive_window_fires_nothing(cx: &mut TestAppContext) {
        let (dispatcher, cells, fake, window, node) = inactive_scene(cx);

        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);
        cells.document.borrow_mut().request_blur(node);
        draw(cx, window);
        set_active(cx, window, &dispatcher, true);

        assert_eq!(focus_events(&fake), ["focus:$window"]);
    }

    #[gpui::test]
    fn a_node_made_inert_while_the_window_is_inactive_fires_no_second_blur(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, cells, fake, window, node) = inactive_scene(cx);
        set_active(cx, window, &dispatcher, true);
        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);
        fake.take();

        set_active(cx, window, &dispatcher, false);
        {
            let document = &mut *cells.document.borrow_mut();
            document.set_attribute(node, "inert", true).unwrap();
            document.mark_focus_dirty(node);
        }
        draw(cx, window);
        set_active(cx, window, &dispatcher, true);

        let blurs: Vec<_> = focus_events(&fake)
            .into_iter()
            .filter(|event| event.starts_with("blur:"))
            .collect();
        assert_eq!(blurs, [format!("blur:{node}"), "blur:$window".to_owned()]);
    }

    #[gpui::test]
    fn window_activation_reports_the_four_focus_events_without_a_related_target(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, cells, fake, window, node) = inactive_scene(cx);
        set_active(cx, window, &dispatcher, true);
        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);
        fake.take();

        set_active(cx, window, &dispatcher, false);
        set_active(cx, window, &dispatcher, true);

        let rows: Vec<_> = fake
            .take()
            .iter()
            .filter(|walked| walked.target_text() == node.to_string())
            .map(|walked| format!("{}:{:?}", walked.name, walked.related()))
            .collect();
        assert_eq!(
            rows,
            ["blur:None", "focusout:None", "focus:None", "focusin:None"]
        );
    }

    #[gpui::test]
    fn activating_the_window_fires_focus_at_window_before_the_focused_node(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, cells, fake, window, node) = inactive_scene(cx);
        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);

        set_active(cx, window, &dispatcher, true);

        let events = focus_events(&fake);
        assert_eq!(
            events[..2],
            ["focus:$window".to_owned(), format!("focus:{node}")]
        );
    }

    #[gpui::test]
    fn deactivating_the_window_fires_blur_at_window_after_the_focused_node(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, cells, fake, window, node) = inactive_scene(cx);
        set_active(cx, window, &dispatcher, true);
        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);
        fake.take();

        set_active(cx, window, &dispatcher, false);

        let events = focus_events(&fake);
        let blur = |target: &str| events.iter().position(|e| *e == format!("blur:{target}"));
        assert!(blur(&node.to_string()).unwrap() < blur("$window").unwrap());
    }

    #[gpui::test]
    fn toggling_the_activation_with_nothing_focused_fires_focus_and_blur_at_window(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, _host, fake, window, _node) = inactive_scene(cx);

        set_active(cx, window, &dispatcher, true);
        set_active(cx, window, &dispatcher, false);

        assert_eq!(focus_events(&fake), ["focus:$window", "blur:$window"]);
    }

    /// Focuses a node, then drops its focus the way `removeChild` does, with no
    /// frame in between.
    fn focused_then_dropped(
        cx: &mut TestAppContext,
    ) -> (Rc<EventDispatcher>, Rc<FakeScript>, Handle) {
        let (dispatcher, cells, fake, window, node) = fake_focus_scene(cx);
        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);
        fake.take();
        {
            let document = &mut *cells.document.borrow_mut();
            document.drop_focus_in(node);
        }
        (dispatcher, fake, window)
    }

    #[gpui::test]
    fn a_window_that_deactivates_and_reactivates_after_a_drop_fires_no_node_event(
        cx: &mut TestAppContext,
    ) {
        let (dispatcher, fake, window) = focused_then_dropped(cx);

        for active in [false, true] {
            set_active(cx, window, &dispatcher, active);
        }

        assert_eq!(focus_events(&fake), ["blur:$window", "focus:$window"]);
    }

    /// Whether `node` is focused for style rules right after `window_activated(active)` returned.
    fn focused_after_activation(
        cx: &mut TestAppContext,
        (dispatcher, cells, window): (&crate::EventDispatcher, &crate::Cells, Handle),
        active: bool,
        node: u32,
    ) -> bool {
        cx.update_window(window.into(), |_, window, cx| {
            dispatcher.window_activated(active, window, cx);
            is_focused(&cells.document.borrow(), node)
        })
        .unwrap()
    }

    #[gpui::test]
    fn deactivating_the_window_clears_the_focus_state(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake, window, node) = fake_focus_scene(cx);
        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);

        assert!(!focused_after_activation(
            cx,
            (&dispatcher, &cells, window),
            false,
            node
        ));
    }

    #[gpui::test]
    fn activating_the_window_restores_the_focus_state(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake, window, node) = fake_focus_scene(cx);
        cells.document.borrow_mut().request_focus(node, true);
        draw(cx, window);
        set_active(cx, window, &dispatcher, false);

        assert!(focused_after_activation(
            cx,
            (&dispatcher, &cells, window),
            true,
            node
        ));
    }

    #[gpui::test]
    fn activating_the_window_writes_the_node_gpui_focused_meanwhile(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake, window, first) = fake_focus_scene(cx);
        let second = crate::tests::fake_focusable_node(&cells);
        cells.document.borrow_mut().request_focus(first, true);
        draw(cx, window);
        set_active(cx, window, &dispatcher, false);
        let handle = cells.window.borrow().focus.handle(second).unwrap();

        let focused = cx
            .update_window(window.into(), |_, window, cx| {
                handle.focus(window, cx);
                dispatcher.window_activated(true, window, cx);
                let document = cells.document.borrow();
                (is_focused(&document, second), is_focused(&document, first))
            })
            .unwrap();

        assert_eq!(focused, (true, false));
    }

    #[gpui::test]
    fn deactivating_the_window_clears_the_press_state(cx: &mut TestAppContext) {
        let (dispatcher, cells, _fake, window, node) = fake_focus_scene(cx);
        let path: Vec<_> = cells.document.borrow().tree().ancestors(node).collect();
        cells.document.borrow_mut().set_active_path(&path);
        set_active(cx, window, &dispatcher, false);

        assert!(!state(&cells.document.borrow(), node, ":active"));
    }
}
