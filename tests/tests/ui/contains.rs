// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `contains()` agrees with the path the host walks.

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn every_path_ancestor_contains_the_target_after_moves_and_removals(cx: &mut TestAppContext) {
    let mut h = load(cx, "contains");
    for id in [
        "leaf", "reverse", "a", "b", "hide", "after", "switch", "switch", "tally",
    ] {
        click(&mut h, id);
    }
    assert_eq!(
        text_of(&mut h, "log"),
        "leaf:true reverse:true a:true b:true hide:true after:true switch:true switch:true tally:true"
    );
}
