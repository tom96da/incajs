// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `style` member of a template ref on the real host.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, near};
use gpui::TestAppContext;

#[gpui::test]
fn an_inline_width_set_through_the_style_member_is_drawn(cx: &mut TestAppContext) {
    let mut h = load(cx, "style-member");
    click(&mut h, "probe");

    let snap = h.snapshot();
    near(by_id(&snap, "box").bounds.unwrap().width, 40.0);
}
