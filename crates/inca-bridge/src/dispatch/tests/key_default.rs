// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::test_support::{TestRoot, focusable_tree, mount_active};
use gpui::{App, AppContext as _, Keystroke, Window, WindowHandle, WindowId};
use inca_gpui::KeyPayload;
use inca_test_macros::gpui_cases;

mod canceled;
mod click;
mod contextmenu;
mod modifiers;
mod mouse;
mod tab;

/// Two buttons, a focusable div and a div without a tab index under one parent. The buttons and
/// the focusable div have a `click` listener. All but the plain div have `keydown` and `keyup`
/// listeners that call `preventDefault` while `globalThis.cancel` is set. The parent has the same
/// listeners, controlled by `globalThis.cancelParent`.
struct Scene {
    window: WindowHandle<TestRoot>,
    dispatcher: EventDispatcher,
    host: Rc<RefCell<Host>>,
    engine: Rc<inca_jsenv::Engine>,
    parent: NodeId,
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
    for event in ["keydown", "keyup"] {
        engine
            .eval::<()>(&format!(
                r"
                __inca_test_listen__({parent}, '{event}', (e) => {{
                    if (globalThis.cancelParent) {{
                        e.preventDefault();
                    }}
                }});
                "
            ))
            .unwrap();
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
        parent,
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

    fn cancel_at_parent(&self, on: bool) {
        self.engine
            .eval::<()>(&format!("globalThis.cancelParent = {on};"))
            .unwrap();
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
