// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The element members of a template ref on the real host.

#![allow(clippy::unwrap_used)]

use super::common::{by_id, click, load, text_of};
use gpui::TestAppContext;
use inca_test_macros::gpui_cases;
use serde_json::{Value, json};

/// Loads the fixture, runs its probe and returns the object it wrote.
fn probe(cx: &mut TestAppContext) -> Value {
    let mut h = load(cx, "members");
    click(&mut h, "probe");
    serde_json::from_str(&text_of(&mut h, "result")).unwrap()
}

#[gpui_cases]
#[gpui::test]
#[case::tag_name("tagName", json!("UL"))]
#[case::attribute_names("attributeNames", json!(["class", "data-kind"]))]
#[case::attribute("attribute", json!("people"))]
#[case::first_element_child("firstElementChild", json!(true))]
#[case::child_element_count("childElementCount", json!(2))]
#[case::next_element_sibling("nextElementSibling", json!(true))]
#[case::parent_element("parentElement", json!(true))]
#[case::id_set_in_upper_case("id", json!("renamed"))]
#[case::disabled_attribute_present("hasDisabled", json!(true))]
#[case::disabled_attribute_matches("matchesDisabled", json!(true))]
#[case::closest("closest", json!(true))]
#[case::matches("matches", json!(true))]
#[case::hover_never_matches("hover", json!(false))]
#[case::invalid_selector("syntaxError", json!("SyntaxError"))]
#[case::query_selector_returns_the_ref("querySelector", json!(true))]
#[case::query_selector_all_keeps_document_order("querySelectorAllIds", json!(["first", "second"]))]
#[case::query_selector_skips_the_start_node("querySelectorSelf", json!(null))]
#[case::query_selector_invalid_selector("querySelectorSyntaxError", json!("SyntaxError"))]
#[case::children_skip_text_nodes("childrenLength", json!(0))]
#[case::named_item_value("namedItemValue", json!("card"))]
fn the_element_members_answer_on_the_real_host(
    cx: &mut TestAppContext,
    #[case] key: &str,
    #[case] expected: Value,
) {
    assert_eq!(probe(cx)[key], expected);
}

#[gpui::test]
fn attributes_set_from_a_ref_reach_the_host_node(cx: &mut TestAppContext) {
    let mut h = load(cx, "members");
    click(&mut h, "probe");

    let snapshot = h.snapshot();
    let field = by_id(&snapshot, "renamed");

    assert!(field.attributes.contains_key("disabled"));
}
