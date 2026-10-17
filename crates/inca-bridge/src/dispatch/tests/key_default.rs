// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::test_support::{TestRoot, focusable_tree, mount_active};
use gpui::{App, AppContext as _, Keystroke, Window, WindowHandle, WindowId};
use inca_gpui::KeyPayload;
use inca_test_macros::gpui_cases;

/// Two buttons, a focusable div and a div without a tab index under one parent. The buttons and
/// the focusable div have a `click` listener. All but the plain div have `keydown` and `keyup`
/// listeners that call `preventDefault` while `globalThis.cancel` is set.
struct Scene {
    window: WindowHandle<TestRoot>,
    dispatcher: EventDispatcher,
    host: Rc<RefCell<Host>>,
    engine: Rc<inca_jsenv::Engine>,
    first: NodeId,
    second: NodeId,
    field: NodeId,
    plain: NodeId,
}

fn scene(cx: &mut TestAppContext) -> Scene {
    let (host, parent, children) = focusable_tree(1, false);
    let field = children[0];
    let (first, second, plain) = {
        let mut h = host.borrow_mut();
        let plain = h.tree.create_node("div").unwrap();
        h.tree.append_child(parent, plain).unwrap();
        let first = h.tree.create_node("button").unwrap();
        let second = h.tree.create_node("button").unwrap();
        h.tree.insert_before(parent, first, Some(field)).unwrap();
        h.tree.insert_before(parent, second, Some(field)).unwrap();
        h.focus.mark_tab_dirty(first);
        h.focus.mark_tab_dirty(second);
        (first, second, plain)
    };
    let engine = engine_for(&host);
    engine.eval::<()>("globalThis.out = [];").unwrap();
    for button in [first, second, field] {
        engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({button}, 'click', (e) => {{
                    out.push(`click:${{e.currentTarget}}:${{e.isTrusted}}`);
                }});
                "
            ))
            .unwrap();
    }
    for node in [first, second, field] {
        for event in ["keydown", "keyup"] {
            engine
                .eval::<()>(&format!(
                    r"
                    __inca_test_listen__({node}, '{event}', (e) => {{
                        if (globalThis.cancel) {{
                            e.preventDefault();
                        }}
                    }});
                    "
                ))
                .unwrap();
        }
    }
    let window = mount_active(cx, &host, &engine, parent);
    let dispatcher = window
        .read_with(cx, |root, _| root.dispatcher.clone())
        .unwrap();
    Scene {
        window,
        dispatcher,
        host,
        engine,
        first,
        second,
        field,
        plain,
    }
}

impl Scene {
    fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
        self.host.borrow_mut().focus.request_focus(node, false);
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    fn blur(&self, cx: &mut TestAppContext, node: NodeId) {
        self.host.borrow_mut().focus.request_blur(node);
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    fn focused(&self, cx: &mut TestAppContext) -> Option<NodeId> {
        self.in_window(cx, |scene, window, cx| {
            scene.host.borrow().focus.focused_node(window, cx)
        })
    }

    fn key(&self, cx: &mut TestAppContext, key: &str, up: bool, canceled: bool) {
        let keystroke = Keystroke::parse(key).unwrap();
        let payload = EventPayload::Key(KeyPayload {
            key: keystroke.key.clone(),
            repeat: false,
            modifiers: keystroke.modifiers,
        });
        self.engine
            .eval::<()>(&format!("globalThis.cancel = {canceled};"))
            .unwrap();
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .key_input(&keystroke, up, &payload, window, cx);
        });
        cx.run_until_parked();
    }

    fn mouse_down(&self, cx: &mut TestAppContext, node: NodeId) {
        let payload = mouse_at(1.0, 1.0);
        self.in_window(cx, |scene, window, cx| {
            let sink = &scene.dispatcher;
            sink.pointer_input("mousedown", node, &payload, window, cx);
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    fn mouse_up(&self, cx: &mut TestAppContext, node: NodeId) {
        let payload = mouse_at(1.0, 1.0);
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .pointer_input("mouseup", node, &payload, window, cx);
        });
    }

    fn fire(&self, cx: &mut TestAppContext, node: NodeId, event: &str) {
        self.in_window(cx, |scene, window, cx| {
            let payload = EventPayload::Focus {
                related_target: None,
            };
            EventSink::fire(&scene.dispatcher, node, event, &payload, window, cx);
        });
    }

    fn set_pending(&self, pending: Option<(WindowId, NodeId)>) {
        self.dispatcher.state.space_down.set(pending);
    }

    fn window_id(&self, cx: &mut TestAppContext) -> WindowId {
        self.in_window(cx, |_, window, _| window.window_handle().window_id())
    }

    fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&Self, &mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
            .unwrap()
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

    fn clicks(&self) -> String {
        self.engine.eval::<String>("JSON.stringify(out)").unwrap()
    }

    fn click_of(node: NodeId) -> String {
        format!(r#"["click:{node}:true"]"#)
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
fn space_down_with_modifiers_still_starts_a_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "ctrl-space", false, false);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
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
fn space_up_clicks_the_focused_button_after_a_prevented_mouse_press_on_it(cx: &mut TestAppContext) {
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
fn space_up_clicks_the_held_button_after_a_space_down_on_another(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
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
fn enter_down_on_a_disabled_button_clicks_nothing(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.disable(s.first);
    s.key(cx, "enter", false, false);
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
fn tab_moves_focus_to_the_next_stop(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "tab", false, false);
    assert_eq!(s.focused(cx), Some(s.second));
}

#[gpui::test]
fn shift_tab_moves_focus_to_the_previous_stop(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.second);
    s.key(cx, "shift-tab", false, false);
    assert_eq!(s.focused(cx), Some(s.first));
}

#[gpui::test]
fn tab_up_leaves_focus_where_it_is(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "tab", true, false);
    assert_eq!(s.focused(cx), Some(s.first));
}

#[gpui_cases]
#[gpui::test]
#[case::control("ctrl-tab")]
#[case::alt("alt-tab")]
#[case::platform("cmd-tab")]
fn tab_with_a_command_modifier_leaves_focus_where_it_is(cx: &mut TestAppContext, key: &str) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, key, false, false);
    assert_eq!(s.focused(cx), Some(s.first));
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
fn a_canceled_tab_down_leaves_focus_where_it_is(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "tab", false, true);
    assert_eq!(s.focused(cx), Some(s.first));
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

#[gpui::test]
fn a_canceled_enter_down_leaves_the_pending_press(cx: &mut TestAppContext) {
    let s = scene(cx);
    s.focus(cx, s.first);
    s.key(cx, "space", false, false);
    s.key(cx, "enter", false, true);
    s.key(cx, "space", true, false);
    assert_eq!(s.clicks(), Scene::click_of(s.first));
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
fn space_up_after_a_blur_of_the_node_in_another_window_clicks_the_button(cx: &mut TestAppContext) {
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

#[gpui_cases]
#[gpui::test]
#[case::control("ctrl-enter", false)]
#[case::alt("alt-enter", false)]
#[case::platform("cmd-enter", false)]
#[case::enter_up("enter", true)]
fn enter_without_a_click_leaves_the_pending_press(cx: &mut TestAppContext, key: &str, up: bool) {
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
