// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The clicks that Space, Enter and a mouse release make on a button.

use gpui::{App, MouseButton, MouseUpEvent, Window};
use inca_gpui::{EventPayload, NodeId};

use crate::dispatch::EventDispatcher;

impl EventDispatcher {
    pub(super) fn focused_enabled_button(&self, window: &Window, cx: &App) -> Option<NodeId> {
        let host = self.host.borrow();
        host.focus.focused_node(window, cx).filter(|id| {
            host.tree.get(*id).is_some_and(|node| {
                node.tag_name() == "button" && !inca_gpui::is_disabled_control(node)
            })
        })
    }

    /// Records the focused enabled button a mouse press landed on.
    pub(crate) fn hold_button(&self, target: NodeId, window: &Window, cx: &App) {
        let window_id = window.window_handle().window_id();
        let held = self
            .focused_enabled_button(window, cx)
            .filter(|button| self.path_from(target).contains(button));
        self.state
            .mouse_held
            .set(held.map(|button| (window_id, button)));
    }

    /// Fires the `click`, `auxclick` and `dblclick` the release of `event` completes, at the
    /// nearest common ancestor of `pressed` and `released`, or at `captured` when a capture ended.
    pub(crate) fn fire_clicks(
        &self,
        event: &MouseUpEvent,
        pressed: Option<NodeId>,
        released: NodeId,
        captured: Option<NodeId>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(pressed) = pressed else {
            return;
        };
        let Some(start) = captured.or_else(|| self.common_ancestor(pressed, released)) else {
            return;
        };
        let mut payload = EventPayload::click(event);
        if event.button != MouseButton::Left {
            self.fire(start, "auxclick", &payload, window, cx);
            return;
        }
        self.fire(start, "click", &payload, window, cx);
        if event.click_count == 2 {
            // `dblclick` carries the mouse fields only.
            payload
                .mouse_mut()
                .into_iter()
                .for_each(|mouse| mouse.pointer = None);
            self.fire(start, "dblclick", &payload, window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::keys::{Scene, scene};
    use crate::tests::{at, mouse_at};
    use gpui::{TestAppContext, WindowId};
    use inca_gpui::{EventPayload, EventSink, NodeId};
    use inca_test_macros::gpui_cases;

    impl Scene {
        fn cancel_at_parent(&self, on: bool) {
            self.engine
                .eval::<()>(&format!("globalThis.cancelParent = {on};"))
                .unwrap();
        }

        fn mouse_up(&self, cx: &mut TestAppContext, node: NodeId) {
            let payload = mouse_at(1.0, 1.0);
            self.in_window(cx, |scene, window, cx| {
                scene
                    .dispatcher
                    .pointer_input("mouseup", node, &payload, window, cx);
            });
        }

        fn set_pending(&self, pending: Option<(WindowId, NodeId)>) {
            self.dispatcher.state.space_down.set(pending);
        }

        fn disable(&self, node: NodeId) {
            self.host
                .borrow_mut()
                .tree
                .set_attribute(node, "disabled", true)
                .unwrap();
        }

        fn enable(&self, node: NodeId) {
            self.host
                .borrow_mut()
                .tree
                .remove_attribute(node, "disabled")
                .unwrap();
        }
    }

    #[gpui::test]
    fn space_up_after_space_down_clicks_the_button(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn space_up_without_space_down_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn space_up_with_a_modifier_clicks_the_button(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "ctrl-space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::down(false)]
    #[case::up(true)]
    fn another_key_leaves_the_pending_press(cx: &mut TestAppContext, up: bool) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "a", up, false);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn space_up_clicks_the_held_button_after_a_space_down_on_another(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.mouse_down(cx, s.second);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.second));
    }

    #[gpui::test]
    fn space_up_with_a_pending_press_of_another_window_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        let other = scene(cx);
        let other_id = other.window_id(cx);
        s.focus(cx, s.first);
        s.set_pending(Some((other_id, s.first)));
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn space_up_after_space_down_while_disabled_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.disable(s.first);
        s.key(cx, "space", false, false);
        s.enable(s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn space_up_after_the_button_became_disabled_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.disable(s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::enter("enter")]
    #[case::space("space")]
    fn a_focused_non_button_does_not_click(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        s.focus(cx, s.field);
        s.key(cx, key, false, false);
        s.key(cx, key, true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn enter_down_on_a_focused_button_clicks_it(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "enter", false, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn enter_up_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "enter", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::control("ctrl-enter")]
    #[case::alt("alt-enter")]
    #[case::platform("cmd-enter")]
    fn enter_with_a_command_modifier_clicks_nothing(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, key, false, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn enter_with_shift_clicks_the_button(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "shift-enter", false, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn space_up_after_an_enter_click_clicks_nothing_more(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "enter", false, false);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn space_down_after_an_enter_click_still_clicks_on_release(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "enter", false, false);
        s.key(cx, "space", false, false);
        s.key(cx, "space", true, false);
        let click = format!("click:{}:true", s.first);
        assert_eq!(s.clicks(), format!(r#"["{click}","{click}"]"#));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::blur_of_another_node("blur", false)]
    #[case::focusout_of_the_node("focusout", true)]
    fn space_up_after_a_focus_event_that_is_no_blur_of_the_node_clicks_the_button(
        cx: &mut TestAppContext,
        event: &str,
        same_node: bool,
    ) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        let node = if same_node { s.first } else { s.second };
        s.fire(cx, node, event);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn space_up_after_a_blur_of_the_node_in_another_window_clicks_the_button(
        cx: &mut TestAppContext,
    ) {
        let s = scene(cx);
        let other = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        let payload = EventPayload::Focus {
            related_target: None,
        };
        other.in_window(cx, |_, window, cx| {
            EventSink::fire(&s.dispatcher, s.first, "blur", &payload, window, cx);
        });
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::control("ctrl-enter", false)]
    #[case::alt("alt-enter", false)]
    #[case::platform("cmd-enter", false)]
    #[case::enter_up("enter", true)]
    fn enter_without_a_click_leaves_the_pending_press(
        cx: &mut TestAppContext,
        key: &str,
        up: bool,
    ) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, key, up, false);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::enter("enter")]
    #[case::space("space")]
    fn a_key_with_nothing_focused_clicks_nothing(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.blur(cx, s.first);
        s.key(cx, key, false, false);
        s.key(cx, key, true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn space_up_with_a_pending_press_of_another_button_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        let window_id = s.window_id(cx);
        s.focus(cx, s.first);
        s.set_pending(Some((window_id, s.second)));
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::enter("enter")]
    #[case::space("space")]
    fn a_canceled_key_down_clicks_nothing(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, key, false, true);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn a_canceled_other_key_down_leaves_the_pending_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "a", false, true);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn space_up_after_a_canceled_space_down_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "space", false, true);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn a_canceled_space_up_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "space", true, true);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn a_second_space_up_after_a_canceled_one_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "space", true, true);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn a_canceled_enter_down_leaves_the_pending_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.key(cx, "enter", false, true);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::enter_click("enter", 0, "disabled", "true", None)]
    #[case::tab_move("tab", 1, "tabindex", "'-1'", Some(2))]
    fn a_job_queued_by_a_key_down_listener_lands_before_the_default_action(
        cx: &mut TestAppContext,
        key: &str,
        changed: usize,
        attribute: &str,
        value: &str,
        focused: Option<usize>,
    ) {
        let s = scene(cx);
        let nodes = [s.first, s.second, s.field];
        s.engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({}, 'keydown', () => {{
                    Promise.resolve().then(() => {{
                        __inca_native__.setAttribute({}, '{attribute}', {value});
                    }});
                }});
                ",
                s.first, nodes[changed]
            ))
            .unwrap();
        s.focus(cx, s.first);
        s.key(cx, key, false, false);
        assert_eq!(s.clicks(), "[]");
        assert_eq!(s.focused(cx), focused.map(|index| nodes[index]));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::enter("enter")]
    #[case::space("space")]
    fn an_ancestors_canceled_key_down_clicks_nothing(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.cancel_at_parent(true);
        s.key(cx, key, false, false);
        s.cancel_at_parent(false);
        s.key(cx, key, true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn a_mouse_press_on_the_button_leaves_the_pending_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.mouse_down(cx, s.first);
        s.mouse_up(cx, s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn space_up_clicks_the_focused_button_after_a_prevented_mouse_press_on_it(
        cx: &mut TestAppContext,
    ) {
        let s = scene(cx);
        s.engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({}, 'mousedown', (e) => {{
                    e.preventDefault();
                }});
                ",
                s.first
            ))
            .unwrap();
        s.focus(cx, s.first);
        s.mouse_down(cx, s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn a_held_mouse_press_takes_one_space_up_click(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.mouse_down(cx, s.first);
        s.key(cx, "space", true, false);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn an_enter_click_ends_a_held_mouse_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.mouse_down(cx, s.first);
        s.key(cx, "enter", false, false);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn losing_focus_ends_a_held_mouse_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.mouse_down(cx, s.first);
        s.focus(cx, s.second);
        s.focus(cx, s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn a_space_press_after_a_held_mouse_click_clicks_again(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.mouse_down(cx, s.first);
        s.key(cx, "space", true, false);
        s.key(cx, "space", false, false);
        s.key(cx, "space", true, false);
        let click = format!("click:{}:true", s.first);
        assert_eq!(s.clicks(), format!(r#"["{click}","{click}"]"#));
    }

    #[gpui::test]
    fn a_mouse_press_on_a_non_focusable_node_ends_the_pending_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.mouse_down(cx, s.plain);
        s.focus(cx, s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn space_up_clicks_the_button_under_a_held_mouse_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.mouse_down(cx, s.second);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.second));
    }

    #[gpui::test]
    fn space_up_after_the_mouse_release_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.mouse_down(cx, s.first);
        s.mouse_up(cx, s.second);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn space_up_after_a_prevented_mouse_press_elsewhere_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({}, 'mousedown', (e) => {{
                    e.preventDefault();
                }});
                ",
                s.second
            ))
            .unwrap();
        s.focus(cx, s.first);
        s.mouse_down(cx, s.second);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn enter_down_on_a_disabled_button_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.disable(s.first);
        s.key(cx, "enter", false, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn space_up_after_the_button_lost_focus_clicks_nothing(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.blur(cx, s.first);
        s.focus(cx, s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn enter_on_a_disabled_button_leaves_the_pending_press(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.first);
        s.key(cx, "space", false, false);
        s.disable(s.first);
        s.key(cx, "enter", false, false);
        s.enable(s.first);
        s.key(cx, "space", true, false);
        assert_eq!(s.clicks(), Scene::click_of(s.first));
    }

    #[gpui::test]
    fn a_key_click_reaches_the_ancestors_while_a_disabled_button_is_hovered(
        cx: &mut TestAppContext,
    ) {
        let s = scene(cx);
        {
            let mut host = s.host.borrow_mut();
            host.tree.set_style(s.first, "width", 100.0).unwrap();
            host.tree.set_style(s.first, "height", 50.0).unwrap();
        }
        s.disable(s.first);
        s.engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({}, 'click', () => {{
                    out.push('parent');
                }});
                ",
                s.parent
            ))
            .unwrap();
        s.focus(cx, s.second);
        s.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        let mut visual = gpui::VisualTestContext::from_window(s.window.into(), cx);
        visual.simulate_mouse_move(at(10.0, 10.0), None, gpui::Modifiers::none());
        s.key(cx, "enter", false, false);
        assert_eq!(
            s.clicks(),
            format!(r#"["click:{}:true","parent"]"#, s.second)
        );
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::enter("enter")]
    #[case::space("space")]
    fn a_focused_descendant_of_a_button_does_not_click_it(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        let inner = {
            let mut host = s.host.borrow_mut();
            let inner = host.tree.create_node("div").unwrap();
            host.tree.set_attribute(inner, "tabindex", "-1").unwrap();
            host.tree.append_child(s.first, inner).unwrap();
            host.focus.mark_tab_dirty(inner);
            inner
        };
        s.focus(cx, inner);
        assert_eq!(s.focused(cx), Some(inner));
        s.key(cx, key, false, false);
        s.key(cx, key, true, false);
        assert_eq!(s.clicks(), "[]");
    }

    #[gpui::test]
    fn a_press_on_a_disabled_button_focuses_its_focusable_ancestor(cx: &mut TestAppContext) {
        let s = scene(cx);
        {
            let mut host = s.host.borrow_mut();
            host.tree.set_attribute(s.parent, "tabindex", "0").unwrap();
            host.focus.mark_tab_dirty(s.parent);
        }
        s.disable(s.first);
        s.host.borrow_mut().focus.mark_tab_dirty(s.first);
        s.focus(cx, s.second);
        s.mouse_down(cx, s.first);
        assert_eq!(s.focused(cx), Some(s.parent));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::pointerdown("pointerdown")]
    #[case::mousedown("mousedown")]
    fn a_canceled_press_does_not_focus(cx: &mut TestAppContext, event: &str) {
        let s = scene(cx);
        s.engine
            .eval::<()>(&format!(
                "__inca_test_listen__({}, '{event}', (e) => e.preventDefault());",
                s.field
            ))
            .unwrap();
        s.mouse_down(cx, s.field);
        assert_eq!(s.focused(cx), None);
    }
}
