// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `target`, `currentTarget` and `composedPath()` are elements, `document` and `window`.

use super::common::{click, load, text_of};
use gpui::TestAppContext;

const CLICK: &str =
    "click:inner>outer[inner,outer,app,root,document,window] this:true ref:true reactive:true";

// The path holds the root container `root` and no `body` or `html` element.
#[gpui::test]
fn a_script_click_carries_the_ref_objects_and_this(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-target");
    click(&mut h, "fire");
    assert_eq!(text_of(&mut h, "log"), CLICK);
}

#[gpui::test]
fn after_dispatch_target_stays_and_current_target_and_path_clear(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-target");
    click(&mut h, "fire");
    click(&mut h, "after");
    assert_eq!(
        text_of(&mut h, "log"),
        format!("{CLICK} after:inner>null[0]")
    );
}

#[gpui::test]
fn a_dispatched_event_targets_its_element(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-target");
    click(&mut h, "ping");
    assert_eq!(
        text_of(&mut h, "log"),
        "ping:inner>inner[inner,outer,app,root,document,window]"
    );
}

#[gpui::test]
fn a_key_with_nothing_focused_targets_the_root_container(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-target");
    h.keystrokes("x");
    assert_eq!(
        text_of(&mut h, "log"),
        "key:root>window[root,document,window]"
    );
}

#[gpui::test]
fn a_key_on_a_focused_node_reaches_window_with_that_target(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-target");
    click(&mut h, "focus");
    h.keystrokes("x");
    assert_eq!(
        text_of(&mut h, "log"),
        "key:inner>window[inner,outer,app,root,document,window]"
    );
}

#[gpui::test]
fn target_id_and_current_target_id_are_their_id_attributes(cx: &mut TestAppContext) {
    let mut h = load(cx, "event-target");
    click(&mut h, "leaf");
    assert_eq!(text_of(&mut h, "log"), r#"ids:"leaf">"""#);
}
