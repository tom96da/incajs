// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window gaining or losing focus: the focus events, the modifier keys and the pointer state
//! it resets.

use gpui::{App, Window};
use inca_gpui::{EventPayload, MousePayload, NO_BUTTON};

use super::EventDispatcher;
use crate::EventTargetId;
use crate::focus::FocusTransition;

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
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::{
        Frame, dispatcher_with_engine, draw, focus_scene, focusable_node, is_focused, listen,
        listen_global, seen_text, set_active,
    };
    use crate::{EventDispatcher, Host};
    use gpui::{AppContext as _, TestAppContext};
    use inca_gpui::NodeId;
    use std::cell::RefCell;
    use std::rc::Rc;

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
