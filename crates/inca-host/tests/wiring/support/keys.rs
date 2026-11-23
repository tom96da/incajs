// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The key scene: two buttons, a field and a plain node under one parent in an active window.

#![allow(clippy::unwrap_used, dead_code, unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Keystroke, MouseButton, MouseUpEvent, TestAppContext, VisualTestContext,
    Window, WindowHandle, WindowId,
};
use inca_document::{EVENTS, PointerSource, WheelPayload};
use inca_document::{EventPayload, EventTargetId, KeyPayload, MousePayload};
use inca_gpui::{EventSink, InputLayout, InputRequest};
use inca_node::NodeId;
use inca_style::set_style;
use inca_window::{Cells, EventDispatcher};

use super::rig::{self as wiring, *};

/// The HTML namespace.
pub const HTML: &str = "http://www.w3.org/1999/xhtml";

/// The SVG namespace.
pub const SVG: &str = "http://www.w3.org/2000/svg";

/// Two buttons, a focusable div or an `input` (`field`) and a div without a tab index (`plain`)
/// under one parent, in an active window. The buttons and the field have a `click` listener that
/// pushes `click:currentTarget:isTrusted` into `out` and `keydown` and `keyup` listeners that read
/// `globalThis.cancel`.
pub struct KeyScene {
    pub window: WindowHandle<TestRoot>,
    pub dispatcher: Wired,
    pub cells: Rc<Cells>,
    pub parent: NodeId,
    pub first: NodeId,
    pub second: NodeId,
    pub field: NodeId,
    pub plain: NodeId,
}

fn key_tree(cells: &Cells, value: Option<&str>) -> [NodeId; 5] {
    let mut h = cells.document.borrow_mut();
    let root = h.root();
    let parent = h.tree.create_node("div").unwrap();
    h.tree.append_child(root, parent).unwrap();
    let tags = [
        "button",
        "button",
        if value.is_some() { "input" } else { "div" },
        "div",
    ];
    let [first, second, field, plain] = tags.map(|tag| {
        let node = h.tree.create_node(tag).unwrap();
        h.tree.append_child(parent, node).unwrap();
        node
    });
    match value {
        Some(value) => {
            set_style(&mut h.tree, field, "width", 100.0).unwrap();
            set_style(&mut h.tree, field, "text_size", 10.0).unwrap();
            h.inputs
                .get_mut()
                .entry(field)
                .or_default()
                .set_value(value);
        }
        None => h.tree.set_attribute(field, "tabindex", "0").unwrap(),
    }
    for node in [first, second, field] {
        h.mark_tab_dirty(node);
    }
    [parent, first, second, field, plain]
}

pub type Scene = KeyScene;

pub fn js_scene(cx: &mut TestAppContext) -> KeyScene {
    js_scene_of(cx, None)
}

/// [`js_scene`] with an `input` holding `value` as the field.
pub fn js_key_input_scene(cx: &mut TestAppContext, value: &str) -> KeyScene {
    js_scene_of(cx, Some(value))
}

fn js_scene_of(cx: &mut TestAppContext, value: Option<&str>) -> KeyScene {
    let rig = Rig::default();
    let [parent, first, second, field, plain] = key_tree(&rig.cells, value);
    let engine = wiring::engine_for(&rig);
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
    let (report, _reported) = capture_reporter();
    let dispatcher = rig.dispatcher(&engine, report);
    let window = inca_window::test_support::mount_active(cx, dispatcher.dispatcher(), parent);
    KeyScene {
        window,
        dispatcher,
        cells: Rc::clone(&rig.cells),
        parent,
        first,
        second,
        field,
        plain,
    }
}

impl KeyScene {
    pub fn focus(&self, cx: &mut TestAppContext, node: NodeId) {
        self.cells.document.borrow_mut().request_focus(node, false);
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    pub fn focused(&self, cx: &mut TestAppContext) -> Option<NodeId> {
        self.in_window(cx, |scene, window, cx| scene.cells.focused_node(window, cx))
    }

    pub fn key(&self, cx: &mut TestAppContext, key: &str, up: bool, canceled: bool) {
        let keystroke = Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os)).unwrap();
        let payload = EventPayload::Key(KeyPayload {
            key: keystroke.key.clone(),
            repeat: false,
            modifiers: inca_gpui::modifiers_of(keystroke.modifiers),
            is_composing: false,
        });
        self.dispatcher
            .engine
            .eval::<()>(&format!("globalThis.cancel = {canceled};"))
            .unwrap();
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .dispatcher()
                .key_input(&keystroke, up, &payload, window, cx);
        });
        cx.run_until_parked();
    }

    /// Sends the space-separated keystrokes through the window.
    pub fn press(&self, cx: &mut TestAppContext, keys: &str) {
        for key in keys.split(' ') {
            let keystroke =
                Keystroke::parse_with_os(key, cx.read(|cx| cx.platform_info().os)).unwrap();
            self.in_window(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx));
        }
        cx.run_until_parked();
        self.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
    }

    pub fn request(&self, cx: &mut TestAppContext, request: InputRequest) {
        self.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .dispatcher()
                .input_request(scene.field, request, window, cx);
        });
    }

    pub fn state(&self) -> inca_document::InputState {
        self.cells.document.borrow().inputs.borrow()[&self.field].clone()
    }

    pub fn in_window<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&Self, &mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.window.into(), |_, window, cx| f(self, window, cx))
            .unwrap()
    }
}

impl KeyScene {
    /// Moves `plain` between `first` and `second`.
    pub fn plain_between_the_buttons(&self) {
        self.cells
            .document
            .borrow_mut()
            .tree
            .insert_before(self.parent, self.plain, Some(self.second))
            .unwrap();
    }

    pub fn mouse_down(&self, cx: &mut TestAppContext, node: NodeId) {
        let payload = mouse_at(1.0, 1.0);
        self.in_window(cx, |scene, window, cx| {
            let sink = scene.dispatcher.dispatcher();
            sink.pointer_input("mousedown", node, &payload, window, cx);
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    /// A node of `namespace` named `tag` with `attributes`, before the field.
    pub fn add_link(&self, namespace: &str, tag: &str, attributes: &[(&str, &str)]) -> NodeId {
        let mut document = self.cells.document.borrow_mut();
        let node = document
            .tree
            .create_element_ns(Some(namespace), None, tag)
            .unwrap();
        for &(key, value) in attributes {
            document.tree.set_attribute(node, key, value).unwrap();
        }
        document
            .tree
            .insert_before(self.parent, node, Some(self.field))
            .unwrap();
        document.mark_tab_dirty(node);
        node
    }
}
