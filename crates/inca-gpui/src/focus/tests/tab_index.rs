// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn tab_index_parsing_follows_the_leading_integer() {
    let cases: [(&str, Option<i32>); 14] = [
        (" 1.5", Some(1)),
        ("+1", Some(1)),
        ("-0", Some(0)),
        ("-1", Some(-1)),
        ("\t\n 7x", Some(7)),
        ("1e3", Some(1)),
        ("abc", None),
        ("", None),
        ("+", None),
        ("-", None),
        (".5", None),
        ("2147483648", None),
        ("-2147483649", None),
        ("2147483647", Some(i32::MAX)),
    ];
    let mismatches: Vec<_> = cases
        .iter()
        .filter(|(text, want)| parse_tab_index(text) != *want)
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

#[test]
fn tab_index_of_reads_strings_numbers_and_booleans() {
    let cases = [
        (AttributeValue::from("3"), Some(3)),
        (AttributeValue::Number(0.0), Some(0)),
        (AttributeValue::Number(-1.0), Some(-1)),
        (AttributeValue::Number(1.5), Some(1)),
        (AttributeValue::Number(1e21), Some(1)),
        (AttributeValue::Number(2.5e-7), Some(2)),
        (AttributeValue::Number(f64::NAN), None),
        (AttributeValue::Bool(true), None),
        (AttributeValue::Bool(false), None),
    ];
    for (value, want) in cases {
        let (tree, id) = tree_with("div", Some(value.clone()));
        assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{value:?}");
    }
    let (tree, id) = tree_with("div", None);
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    assert_eq!(FocusRegistry::tab_index_of(&tree, 999), None);
}

#[test]
fn a_text_node_and_a_display_none_node_have_no_tab_index() {
    let (tree, id) = tree_with("text", Some(AttributeValue::from("0")));
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    tree.set_style(id, "display", "none").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    tree.set_style(id, "display", "flex").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
}

#[rstest]
#[case::display("display")]
#[case::hidden("hidden")]
#[case::inert("inert")]
fn a_node_under_display_none_hidden_or_inert_has_no_tab_index(#[case] hide: &str) {
    let mut mismatches = Vec::new();
    let (mut tree, child) = tree_with("div", Some(AttributeValue::from("0")));
    let parent = tree.create_node("div").unwrap();
    tree.append_child(parent, child).unwrap();
    for (target, name) in [(child, "self"), (parent, "ancestor")] {
        if hide == "display" {
            tree.set_style(target, "display", "none").unwrap();
        } else {
            tree.set_attribute(target, hide, true).unwrap();
        }
        if FocusRegistry::tab_index_of(&tree, child).is_some() {
            mismatches.push(format!("{hide} on {name} keeps the tab index"));
        }
        if hide == "display" {
            tree.remove_style(target, "display").unwrap();
        } else {
            tree.remove_attribute(target, hide).unwrap();
        }
        if FocusRegistry::tab_index_of(&tree, child) != Some(0) {
            mismatches.push(format!("removing {hide} on {name} loses the tab index"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn hidden_with_an_own_display_or_until_found_keeps_the_tab_index() {
    let (mut tree, id) = tree_with("div", Some(AttributeValue::from("0")));
    tree.set_attribute(id, "hidden", "").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), None);
    tree.set_style(id, "display", "flex").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
    tree.remove_style(id, "display").unwrap();
    tree.set_attribute(id, "hidden", "until-found").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
}

#[gpui::test]
fn mark_subtree_dirty_queues_nodes_that_can_hold_focus(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    let (mut tree, focusable) = tree_with("div", Some(AttributeValue::from("0")));
    let parent = tree.create_node("div").unwrap();
    let plain = tree.create_node("div").unwrap();
    let button = tree.create_node("button").unwrap();
    let input = tree.create_node("input").unwrap();
    let held = tree.create_node("div").unwrap();
    for child in [focusable, plain, button, input, held] {
        tree.append_child(parent, child).unwrap();
    }
    let mut registry = FocusRegistry::default();
    cx.update(|_, cx| registry.get_or_create(held, cx));
    registry.mark_subtree_dirty(&tree, parent);
    assert_eq!(
        registry.tab_dirty,
        HashSet::from([focusable, button, input, held])
    );
}

#[rstest]
#[case::absent(None, Some(0))]
#[case::positive(Some(AttributeValue::from("3")), Some(3))]
#[case::negative(Some(AttributeValue::from("-1")), Some(-1))]
#[case::not_a_number(Some(AttributeValue::from("abc")), Some(0))]
#[case::boolean(Some(AttributeValue::Bool(true)), Some(0))]
fn a_button_has_tab_index_0_unless_told_otherwise(
    #[case] value: Option<AttributeValue>,
    #[case] want: Option<i32>,
) {
    let (tree, id) = tree_with("button", value.clone());
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{value:?}");
}

#[test]
fn an_input_has_tab_index_0_without_a_tabindex() {
    let (tree, id) = tree_with("input", None);
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(0));
}

#[rstest]
#[case::button("button", None)]
#[case::input("input", None)]
#[case::div("div", Some(2))]
fn a_disabled_attribute_drops_the_tab_index_of_a_control_only(
    #[case] tag: &str,
    #[case] want: Option<i32>,
) {
    let (mut tree, id) = tree_with(tag, Some(AttributeValue::from("2")));
    tree.set_attribute(id, "disabled", true).unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), want, "{tag}");
    tree.remove_attribute(id, "disabled").unwrap();
    assert_eq!(FocusRegistry::tab_index_of(&tree, id), Some(2), "{tag}");
}
