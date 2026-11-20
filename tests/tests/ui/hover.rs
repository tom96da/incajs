// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The boundary events of a pointer that moves over nested nodes.

use super::common::{appended, click, hover, load, settled, text_of};
use gpui::TestAppContext;
use inca_host::harness::Harness;

/// Hovers `ids` in order and returns what the last move added to `#log`.
fn last_move(h: &mut Harness, ids: &[&str]) -> String {
    let mut before = String::new();
    for id in ids {
        before = text_of(h, "log");
        hover(h, id);
    }
    appended(&before, &text_of(h, "log")).to_owned()
}

#[gpui::test]
fn entering_a_nested_node_fires_over_up_the_path_then_enter_down_it(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");

    assert_eq!(
        last_move(&mut h, &["home", "c"]),
        "mouseout@home(home)<c mouseleave@home(home)<c mouseover@c(c)<home \
         mouseover@b(c)<home mouseover@a(c)<home mouseenter@a(a)<home \
         mouseenter@b(b)<home mouseenter@c(c)<home"
    );
}

#[gpui::test]
fn moving_to_a_sibling_fires_out_and_leave_before_over_and_enter(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");

    assert_eq!(
        last_move(&mut h, &["home", "c", "d"]),
        "mouseout@c(c)<d mouseout@b(c)<d mouseout@a(c)<d mouseleave@c(c)<d \
         mouseover@d(d)<c mouseover@b(d)<c mouseover@a(d)<c mouseenter@d(d)<c"
    );
}

#[gpui::test]
fn leaving_a_subtree_fires_leave_from_the_innermost_node_outward(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");

    assert_eq!(
        last_move(&mut h, &["home", "d", "e"]),
        "mouseout@d(d)<e mouseout@b(d)<e mouseout@a(d)<e mouseleave@d(d)<e \
         mouseleave@b(b)<e mouseleave@a(a)<e mouseover@e(e)<d mouseenter@e(e)<d"
    );
}

#[gpui::test]
fn entering_a_child_from_its_parent_fires_enter_on_the_child_only(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");

    assert_eq!(
        last_move(&mut h, &["home", "b", "c"]),
        "mouseout@b(b)<c mouseout@a(b)<c mouseover@c(c)<b mouseover@b(c)<b \
         mouseover@a(c)<b mouseenter@c(c)<b"
    );
}

#[gpui::test]
fn leaving_a_child_for_its_parent_fires_leave_on_the_child_only(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");

    assert_eq!(
        last_move(&mut h, &["home", "c", "b"]),
        "mouseout@c(c)<b mouseout@b(c)<b mouseout@a(c)<b mouseleave@c(c)<b \
         mouseover@b(b)<c mouseover@a(b)<c"
    );
}

#[gpui::test]
fn hovering_the_same_node_again_fires_no_boundary_event(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");

    assert_eq!(last_move(&mut h, &["home", "c", "c"]), "");
}

#[gpui::test]
fn a_second_move_inside_one_node_fires_only_the_move_events(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover-order");

    assert_eq!(
        last_move(&mut h, &["a", "a"]),
        "pointermove@a<null mousemove@a<null"
    );
}

#[gpui::test]
fn a_mouse_move_reports_the_pointer_position_in_client_coordinates(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover-order");

    hover(&mut h, "a");

    assert_eq!(text_of(&mut h, "pos"), "mousemove:50:20");
}

#[gpui::test]
fn a_pointer_move_fires_pointer_events_before_mouse_events_and_moves_last(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover-order");

    assert_eq!(
        last_move(&mut h, &["a", "b"]),
        "pointerout@a<b pointerleave@a<b pointerover@b<a pointerenter@b<a mouseout@a<b \
         mouseleave@a<b mouseover@b<a mouseenter@b<a pointermove@b<null mousemove@b<null"
    );
}

#[gpui::test]
fn leaving_every_node_fires_out_and_leave_from_the_innermost_node_outward(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");
    hover(&mut h, "home");
    hover(&mut h, "c");
    let before = text_of(&mut h, "log");

    h.unhover();

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "mouseout@c(c)<null mouseout@b(c)<null mouseout@a(c)<null mouseleave@c(c)<null \
         mouseleave@b(b)<null mouseleave@a(a)<null"
    );
}

#[gpui::test]
fn hovering_a_node_again_after_unhover_fires_the_enter_events_again(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");
    hover(&mut h, "home");
    hover(&mut h, "c");
    h.unhover();
    let before = text_of(&mut h, "log");

    hover(&mut h, "c");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "mouseover@c(c)<null mouseover@b(c)<null mouseover@a(c)<null \
         mouseenter@a(a)<null mouseenter@b(b)<null mouseenter@c(c)<null"
    );
}

#[gpui::test]
fn a_node_shifted_under_a_still_pointer_gets_over_and_enter(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");
    hover(&mut h, "home");
    hover(&mut h, "c");
    let before = text_of(&mut h, "log");

    click(&mut h, "c");
    settled(&mut h, 2);

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "mouseover@d(d)<b mouseover@b(d)<b mouseover@a(d)<b mouseenter@d(d)<b"
    );
}

#[gpui::test]
fn a_move_after_the_shift_leaves_from_the_node_that_took_the_removed_place(
    cx: &mut TestAppContext,
) {
    let mut h = load(cx, "hover");
    hover(&mut h, "home");
    hover(&mut h, "c");
    click(&mut h, "c");
    settled(&mut h, 2);
    let before = text_of(&mut h, "log");

    hover(&mut h, "e");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "mouseout@d(d)<e mouseout@b(d)<e mouseout@a(d)<e mouseleave@d(d)<e \
         mouseleave@b(b)<e mouseleave@a(a)<e mouseover@e(e)<d mouseenter@e(e)<d"
    );
}

#[gpui::test]
fn a_removed_node_with_nothing_in_its_place_gets_no_out_event(cx: &mut TestAppContext) {
    let mut h = load(cx, "hover");
    hover(&mut h, "home");
    hover(&mut h, "d");
    click(&mut h, "d");
    let before = text_of(&mut h, "log");

    hover(&mut h, "e");

    assert_eq!(
        appended(&before, &text_of(&mut h, "log")),
        "mouseleave@b(b)<e mouseleave@a(a)<e mouseover@e(e)<b mouseenter@e(e)<b"
    );
}
