// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The interfaces that the nodes of a real app are instances of.

#![allow(clippy::unwrap_used)]

use super::common::{click, load, probe, text_of};
use gpui::TestAppContext;
use inca_test_macros::gpui_cases;
use serde_json::{Value, json};

#[gpui_cases]
#[gpui::test]
#[case::document_reads_as_an_html_document("documentTag", json!("[object HTMLDocument]"))]
#[case::a_ref_is_an_html_element("refIsHtmlElement", json!(true))]
#[case::a_text_child_is_a_text("textChildIsText", json!(true))]
#[case::the_last_child_of_the_box_is_a_comment("commentNodeType", json!(8))]
#[case::a_v_if_comment_is_character_data("commentIsCharacterData", json!(true))]
#[case::a_v_if_comment_is_no_element("commentIsElement", json!(false))]
#[case::an_svg_from_inner_html_is_an_element("svgIsElement", json!(true))]
#[case::an_svg_from_inner_html_is_no_html_element("svgIsHtmlElement", json!(false))]
#[case::a_fragment_is_a_document_fragment("fragmentIsDocumentFragment", json!(true))]
#[case::a_fragment_finds_a_descendant_with_both_queries("fragmentFindsDescendants", json!(["P", 2]))]
#[case::window_is_an_event_target("windowIsEventTarget", json!(true))]
#[case::a_text_child_reads_its_ref_as_parent_element("textParentIsTheRef", json!(true))]
#[case::a_deep_clone_reads_a_child_by_index("deepCloneChildTag", json!([1000, "LI"]))]
fn the_nodes_are_instances_of_their_interfaces_on_the_real_host(
    cx: &mut TestAppContext,
    #[case] key: &str,
    #[case] expected: Value,
) {
    assert_eq!(probe(cx, "interfaces")[key], expected);
}

#[gpui::test]
fn a_v_if_comment_is_absent_from_the_snapshot(cx: &mut TestAppContext) {
    let snap = load(cx, "interfaces").snapshot();

    assert!(snap.find(&|node| node.tag == "#comment").is_none());
}

#[gpui::test]
fn a_removed_node_that_a_script_holds_survives_a_frame(cx: &mut TestAppContext) {
    let mut h = load(cx, "interfaces");
    click(&mut h, "removeTop");

    click(&mut h, "reattach");

    assert_eq!(text_of(&mut h, "reattached"), "x");
}

#[gpui_cases]
#[gpui::test]
#[case::a_bound_aria_property_reads_back("boundLabel", json!("save"))]
#[case::a_bound_aria_property_sets_its_attribute("boundLabelAttribute", json!("save"))]
#[case::the_role_attribute_reads_through_role("role", json!("tab"))]
#[case::an_aria_attribute_reads_through_its_property("selected", json!("true"))]
#[case::the_labelled_by_property_resolves_the_id("labelledBy", json!(["name"]))]
fn the_aria_members_reflect_the_attributes_of_a_template_on_the_real_host(
    cx: &mut TestAppContext,
    #[case] key: &str,
    #[case] expected: Value,
) {
    let got = probe(cx, "interfaces-aria");

    assert_eq!(got[key], expected);
}
