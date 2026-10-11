// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A template ref kept after its element is removed.

use super::common::{click, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn a_removed_ref_ignores_listener_and_tabindex_calls(cx: &mut TestAppContext) {
    let mut h = load(cx, "removed");
    click(&mut h, "remove");

    click(&mut h, "poke");

    assert_eq!(text_of(&mut h, "result"), "ok:0:true");
}
