// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `contextmenu` that the Menu key and Shift+F10 open.

use gpui::{App, Window};
use inca_document::{EventPayload, MousePayload, NO_BUTTON, PointerSource};
use inca_gpui::modifiers_of;

use crate::dispatch::EventDispatcher;

impl EventDispatcher {
    /// Fires the `contextmenu` of the Menu key at the focused node, with the pointer at the center
    /// of its box. Without a focused node that has a box, it targets the container at (1, 1).
    pub(super) fn fire_key_context_menu(
        &self,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (target, x, y) = {
            let focused = self.cells.focused_node(window, cx);
            let boxed = focused.and_then(|node| {
                let center = self.cells.window.borrow().layout_of(node)?.bounds.center();
                Some((node, f32::from(center.x), f32::from(center.y)))
            });
            boxed.unwrap_or_else(|| (self.cells.container_at(1.0, 1.0), 1.0, 1.0))
        };
        let payload = EventPayload::Mouse(MousePayload {
            client_x: x,
            client_y: y,
            button: NO_BUTTON,
            pointer: Some(PointerSource::Mouse),
            ..MousePayload::keyboard_click(modifiers_of(modifiers))
        });
        self.fire(target, "contextmenu", &payload, window, cx);
    }
}

/// Whether Shift+F10 opens the context menu on the platform: everywhere but macOS.
pub(super) fn shift_f10_opens_context_menu(macos: bool) -> bool {
    !macos
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use crate::tests::Walked;
    use crate::tests::keys::{Scene, scene};
    use gpui::{Keystroke, TestAppContext};
    use inca_document::{EventPayload, EventTargetId, KeyPayload, PointerSource};
    use inca_gpui::EventSink as _;
    use inca_gpui::modifiers_of;
    use inca_node::NodeId;
    use inca_test_macros::gpui_cases;

    const EVENTS: &[&str] = &["keydown", "contextmenu"];

    /// The `keydown` and `contextmenu` walks at `node`, in order.
    fn menu_walks(s: &Scene, node: NodeId) -> Vec<Walked> {
        s.walks()
            .into_iter()
            .filter(|walked| {
                walked.target == EventTargetId::Node(node) && EVENTS.contains(&walked.name.as_str())
            })
            .collect()
    }

    fn center_of(s: &Scene, node: NodeId) -> (f32, f32) {
        let layout = s
            .cells
            .window
            .borrow()
            .layout
            .borrow()
            .get(&node)
            .copied()
            .unwrap();
        let center = layout.bounds.center();
        (f32::from(center.x), f32::from(center.y))
    }

    #[gpui::test]
    fn contextmenu_key_dispatches_to_focused_node(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.field);
        let (x, y) = center_of(&s, s.field);
        s.forget();
        s.key(cx, "menu", false, false);
        s.key(cx, "menu", true, false);
        let walks = menu_walks(&s, s.field);
        assert_eq!(walks.len(), 2);
        assert_eq!(walks[0].name, "keydown");
        assert_eq!(walks[1].name, "contextmenu");
        let menu = walks[1].mouse();
        assert_eq!((menu.client_x, menu.client_y), (x, y));
        assert_eq!(menu.pointer, Some(PointerSource::Mouse));
    }

    #[gpui::test]
    fn a_mac_menu_key_dispatches_contextmenu(cx: &mut TestAppContext) {
        let s = scene(cx);
        s.focus(cx, s.field);
        s.forget();
        let keystroke = Keystroke {
            key: "\u{10}".to_string(),
            key_char: None,
            modifiers: gpui::Modifiers::none(),
        };
        let payload = EventPayload::Key(KeyPayload {
            key: "ContextMenu".to_string(),
            repeat: false,
            modifiers: modifiers_of(keystroke.modifiers),
            is_composing: false,
        });
        s.in_window(cx, |scene, window, cx| {
            scene
                .dispatcher
                .key_input(&keystroke, false, &payload, window, cx);
        });
        let names: Vec<_> = menu_walks(&s, s.field)
            .into_iter()
            .map(|walked| walked.name)
            .collect();
        assert_eq!(names, EVENTS);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::shift_f10("shift-f10", false, usize::from(!cfg!(target_os = "macos")))]
    #[case::f10("f10", false, 0)]
    #[case::shift_menu("shift-menu", false, 0)]
    #[case::keyup("shift-f10", true, 0)]
    fn only_shift_f10_keydown_dispatches_contextmenu(
        cx: &mut TestAppContext,
        key: &str,
        up: bool,
        want: usize,
    ) {
        let s = scene(cx);
        s.focus(cx, s.field);
        s.key(cx, key, up, false);
        let menus = menu_walks(&s, s.field)
            .iter()
            .filter(|walked| walked.name == "contextmenu")
            .count();
        assert_eq!(menus, want);
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::menu("menu")]
    #[case::shift_f10("shift-f10")]
    fn a_canceled_contextmenu_keydown_suppresses_contextmenu(cx: &mut TestAppContext, key: &str) {
        let s = scene(cx);
        s.focus(cx, s.field);
        s.forget();
        s.key(cx, key, false, true);
        let names: Vec<_> = menu_walks(&s, s.field)
            .into_iter()
            .map(|walked| walked.name)
            .collect();
        assert_eq!(names, ["keydown"]);
    }

    /// The `contextmenu` walk of the scene.
    fn contextmenu(s: &Scene) -> Walked {
        let mut menus: Vec<_> = s
            .walks()
            .into_iter()
            .filter(|walked| walked.name == "contextmenu")
            .collect();
        assert_eq!(menus.len(), 1);
        menus.remove(0)
    }

    #[gpui::test]
    fn contextmenu_key_without_focus_targets_the_container_at_1_1(cx: &mut TestAppContext) {
        let s = scene(cx);
        for style in ["width", "height"] {
            s.cells
                .document
                .borrow_mut()
                .set_style_prop(s.first, style, 100.0)
                .unwrap();
        }
        s.in_window(cx, |_, window, cx| window.draw(cx).clear(cx));
        s.key(cx, "menu", false, false);
        let menu = contextmenu(&s);
        assert_eq!(menu.target, EventTargetId::Node(s.first));
        let mouse = menu.mouse();
        assert_eq!((mouse.client_x, mouse.client_y), (1.0, 1.0));
    }

    #[gpui::test]
    fn contextmenu_key_on_a_focused_node_without_a_box_targets_the_container_at_1_1(
        cx: &mut TestAppContext,
    ) {
        let s = scene(cx);
        s.cells
            .document
            .borrow_mut()
            .set_style_prop(s.first, "width", 100.0)
            .unwrap();
        s.cells
            .document
            .borrow_mut()
            .set_style_prop(s.first, "height", 100.0)
            .unwrap();
        s.focus(cx, s.field);
        s.cells.window.borrow().layout.borrow_mut().remove(&s.field);
        s.key(cx, "menu", false, false);
        let menu = contextmenu(&s);
        assert_eq!(menu.target, EventTargetId::Node(s.first));
        let mouse = menu.mouse();
        assert_eq!((mouse.client_x, mouse.client_y), (1.0, 1.0));
    }

    #[gpui::test]
    fn contextmenu_key_outside_every_box_targets_the_root(cx: &mut TestAppContext) {
        let s = scene(cx);
        let root = s.cells.document.borrow().root();
        s.key(cx, "menu", false, false);
        assert_eq!(contextmenu(&s).target, EventTargetId::Node(root));
    }

    #[gpui_cases]
    #[gpui::test]
    #[case::linux_and_windows(false, true)]
    #[case::macos(true, false)]
    fn shift_f10_opens_the_context_menu_except_on_macos(
        _cx: &mut TestAppContext,
        macos: bool,
        want: bool,
    ) {
        assert_eq!(
            crate::keys::context_menu::shift_f10_opens_context_menu(macos),
            want
        );
    }
}
