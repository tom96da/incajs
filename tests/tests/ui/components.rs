// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Props, slots and emits across a parent and a child component.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, near, text, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn the_child_renders_the_prop_and_the_slotted_content(cx: &mut TestAppContext) {
    let snap = load(cx, "compose").snapshot();

    assert_eq!(text(by_id(&snap, "title")), "First");
    assert_eq!(text(by_id(&snap, "slotted")), "Slotted");
}

#[gpui::test]
fn the_slot_box_sits_below_the_title_and_holds_the_slotted_content(cx: &mut TestAppContext) {
    let snap = load(cx, "compose").snapshot();
    let title = by_id(&snap, "title").bounds.unwrap();
    let slot = by_id(&snap, "slot").bounds.unwrap();
    let slotted = by_id(&snap, "slotted").bounds.unwrap();

    near(slot.y, title.y + 20.0);
    near(slotted.x, slot.x);
    near(slotted.y, slot.y);
    near(slotted.width, 60.0);
}

#[gpui::test]
fn a_changed_parent_value_reaches_the_child_prop(cx: &mut TestAppContext) {
    let mut h = load(cx, "compose");

    click(&mut h, "rename");

    assert_eq!(text_of(&mut h, "title"), "Second");
}

#[gpui::test]
fn an_emit_after_a_click_updates_the_parent_state(cx: &mut TestAppContext) {
    let mut h = load(cx, "compose");
    assert_eq!(text_of(&mut h, "total"), "0");

    click(&mut h, "bump");
    click(&mut h, "bump");

    assert_eq!(text_of(&mut h, "total"), "10");
}
