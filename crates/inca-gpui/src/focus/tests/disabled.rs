// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[gpui::test]
fn focus_on_a_disabled_button_is_ignored(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (tree, id) = rooted_button(true);
    let mut registry = FocusRegistry::default();
    registry.mark_tab_dirty(id);
    registry.request_focus(id, false);

    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert!(transitions.is_empty());
    assert_eq!(
        cx.update(|window, cx| registry.focused_node(window, cx)),
        None
    );
}

#[gpui::test]
fn disabling_a_focused_button_reports_its_blur(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, id) = rooted_button(false);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &[id], id);

    tree.set_attribute(id, "disabled", true).unwrap();
    registry.mark_tab_dirty(id);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].blurred, Some(id));
    assert_eq!(transitions[0].focused, None);
}

#[gpui::test]
fn an_enabled_button_takes_focus_again(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, id) = rooted_button(false);
    let mut registry = FocusRegistry::default();
    focus_all(cx, &mut registry, &tree, &[id], id);
    tree.set_attribute(id, "disabled", true).unwrap();
    registry.mark_tab_dirty(id);
    cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    tree.remove_attribute(id, "disabled").unwrap();
    registry.mark_tab_dirty(id);
    registry.request_focus(id, false);
    let transitions = cx.update(|window, cx| registry.apply_pending(&tree, window, cx));

    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].blurred, None);
    assert_eq!(transitions[0].focused, Some(id));
}
