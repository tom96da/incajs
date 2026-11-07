// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Classic node mutation and the node relations seen from a script.

use super::common::{by_id, child_ids, click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_text_node_replaced_by_text_content_keeps_working(cx: &mut TestAppContext) {
    let mut h = load(cx, "node-mutation");
    for id in ["replace", "reuse"] {
        click(&mut h, id);
    }
    assert_eq!(text_of(&mut h, "host"), "xz");
}

#[gpui::test]
fn the_children_of_an_appended_fragment_render_in_order(cx: &mut TestAppContext) {
    let mut h = load(cx, "node-mutation");
    click(&mut h, "fill");
    assert_eq!(child_ids(by_id(&h.snapshot(), "list")), ["c1", "c2", "c3"]);
}

#[gpui::test]
fn clone_node_gives_a_node_equal_to_the_original(cx: &mut TestAppContext) {
    let mut h = load(cx, "node-mutation");
    click(&mut h, "compare");
    assert_eq!(text_of(&mut h, "clone-log"), "true false true");
}

#[gpui::test]
fn contains_is_true_for_the_node_itself(cx: &mut TestAppContext) {
    let mut h = load(cx, "node-mutation");
    click(&mut h, "compare");
    let log = text_of(&mut h, "contains-log");
    assert_eq!(log.split(' ').nth(2), Some("true"));
}

#[gpui::test]
fn contains_follows_the_descendants_in_the_tree(cx: &mut TestAppContext) {
    let mut h = load(cx, "node-mutation");
    click(&mut h, "compare");
    let log = text_of(&mut h, "contains-log");
    let words: Vec<&str> = log.split(' ').collect();
    assert_eq!([words[0], words[1], words[3]], ["true", "false", "false"]);
}

#[gpui::test]
fn compare_document_position_names_the_order_and_nesting(cx: &mut TestAppContext) {
    let mut h = load(cx, "node-mutation");
    click(&mut h, "compare");
    assert_eq!(text_of(&mut h, "position-log"), "4 2 20 10");
}
