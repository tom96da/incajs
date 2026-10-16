// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `id` of a template ref reflects the `id` attribute.

use super::common::{click, has_id, load, text_of};
use gpui::TestAppContext;

#[gpui::test]
fn id_reads_each_bound_value_as_a_string(cx: &mut TestAppContext) {
    let mut h = load(cx, "id-attribute");
    click(&mut h, "read");
    assert_eq!(text_of(&mut h, "log"), r#""" "5" "named""#);
}

#[gpui::test]
fn id_reads_empty_once_the_binding_removes_the_attribute(cx: &mut TestAppContext) {
    let mut h = load(cx, "id-attribute");
    click(&mut h, "drop");
    click(&mut h, "read");
    assert_eq!(text_of(&mut h, "log"), r#""" "5" """#);
}

#[gpui::test]
fn assigning_id_writes_the_attribute_as_a_string(cx: &mut TestAppContext) {
    let mut h = load(cx, "id-attribute");
    click(&mut h, "assign");
    assert_eq!(text_of(&mut h, "log"), r#""7" "null" "named""#);
    let snap = h.snapshot();
    assert!(has_id(&snap, "7") && has_id(&snap, "null"));
}
