// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Toggles `v-if` and edits `v-for` lists in real `.vue` apps.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, child_ids, click, has_id, load, near, text};
use gpui::TestAppContext;
use inca_host::harness::Harness;

fn items(h: &mut Harness) -> Vec<String> {
    child_ids(by_id(&h.snapshot(), "list"))
}

#[gpui::test]
fn v_if_starts_on_its_else_branch(cx: &mut TestAppContext) {
    let snap = load(cx, "toggle").snapshot();

    assert!(!has_id(&snap, "panel"));
    assert!(has_id(&snap, "empty"));
}

#[gpui::test]
fn a_click_mounts_the_v_if_branch_and_unmounts_the_else_branch(cx: &mut TestAppContext) {
    let mut h = load(cx, "toggle");

    click(&mut h, "button");

    let snap = h.snapshot();
    assert_eq!(text(by_id(&snap, "panel")), "Panel");
    near(by_id(&snap, "panel").bounds.unwrap().height, 40.0);
    assert!(!has_id(&snap, "empty"));
}

#[gpui::test]
fn toggling_back_restores_the_else_branch(cx: &mut TestAppContext) {
    let mut h = load(cx, "toggle");

    click(&mut h, "button");
    click(&mut h, "button");

    let snap = h.snapshot();
    assert!(!has_id(&snap, "panel"));
    assert_eq!(text(by_id(&snap, "empty")), "Empty");
}

#[gpui::test]
fn v_for_renders_each_item_in_order(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");

    assert_eq!(items(&mut h), ["item-1", "item-2", "item-3"]);
    let snap = h.snapshot();
    assert_eq!(text(by_id(&snap, "item-2")), "Item 2");
}

#[gpui::test]
fn v_for_items_stack_in_order(cx: &mut TestAppContext) {
    let snap = load(cx, "list").snapshot();
    let first = by_id(&snap, "item-1").bounds.unwrap();
    let second = by_id(&snap, "item-2").bounds.unwrap();

    near(second.y, first.y + first.height);
}

#[gpui::test]
fn adding_appends_an_item(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");

    click(&mut h, "add");

    assert_eq!(items(&mut h), ["item-1", "item-2", "item-3", "item-4"]);
    assert_eq!(text(by_id(&h.snapshot(), "item-4")), "Item 4");
}

#[gpui::test]
fn removing_drops_the_first_item(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");

    click(&mut h, "remove");

    assert_eq!(items(&mut h), ["item-2", "item-3"]);
}

#[gpui::test]
fn removing_every_item_leaves_an_empty_list(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");

    for _ in 0..3 {
        click(&mut h, "remove");
    }
    click(&mut h, "remove");

    assert!(items(&mut h).is_empty());
}

#[gpui::test]
fn reversing_reorders_the_keyed_items(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");

    click(&mut h, "reverse");

    assert_eq!(items(&mut h), ["item-3", "item-2", "item-1"]);
}

#[gpui::test]
fn reversing_keeps_each_items_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");
    let before: Vec<_> = ["item-1", "item-2", "item-3"]
        .map(|id| by_id(&h.snapshot(), id).id)
        .into();

    click(&mut h, "reverse");

    let snap = h.snapshot();
    let after: Vec<_> = ["item-1", "item-2", "item-3"]
        .map(|id| by_id(&snap, id).id)
        .into();
    assert_eq!(before, after);
}

#[gpui::test]
fn reversing_twice_restores_the_order(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");

    click(&mut h, "reverse");
    click(&mut h, "reverse");

    assert_eq!(items(&mut h), ["item-1", "item-2", "item-3"]);
}

#[gpui::test]
fn adding_after_reversing_appends_at_the_end(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");

    click(&mut h, "reverse");
    click(&mut h, "add");

    assert_eq!(items(&mut h), ["item-3", "item-2", "item-1", "item-4"]);
}

#[gpui::test]
fn swapping_the_ends_moves_two_keyed_items(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");
    let before: Vec<_> = ["item-1", "item-2", "item-3"]
        .map(|id| by_id(&h.snapshot(), id).id)
        .into();

    click(&mut h, "swap");

    assert_eq!(items(&mut h), ["item-3", "item-2", "item-1"]);
    let snap = h.snapshot();
    let after: Vec<_> = ["item-1", "item-2", "item-3"]
        .map(|id| by_id(&snap, id).id)
        .into();
    assert_eq!(before, after);
}

#[gpui::test]
fn swapping_after_an_add_moves_the_new_item_to_the_front(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");
    click(&mut h, "add");

    click(&mut h, "swap");

    assert_eq!(items(&mut h), ["item-4", "item-2", "item-3", "item-1"]);
}

#[gpui::test]
fn inserting_in_the_middle_keeps_the_neighbours(cx: &mut TestAppContext) {
    let mut h = load(cx, "list");
    let first = by_id(&h.snapshot(), "item-1").id;
    let last = by_id(&h.snapshot(), "item-3").id;

    click(&mut h, "insert");

    assert_eq!(items(&mut h), ["item-1", "item-4", "item-2", "item-3"]);
    let snap = h.snapshot();
    assert_eq!(by_id(&snap, "item-1").id, first);
    assert_eq!(by_id(&snap, "item-3").id, last);
    assert_eq!(text(by_id(&snap, "item-4")), "Item 4");
}
