// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn debug_selector_names_the_node_id() {
    assert_eq!(debug_selector(7), "node-7");
}

#[test]
fn unknown_root_returns_none() {
    let tree = VirtualTree::new();
    assert!(build_spec(&tree, 42).is_none());
}

#[test]
fn non_text_tag_is_a_container() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(spec.tag, ElementTag::Container);
}

#[test]
fn the_hidden_attribute_makes_the_spec_display_none_unless_display_is_set() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    let display = |tree: &VirtualTree| build_spec(tree, id).unwrap().style.display;
    assert_eq!(display(&tree), None);
    for (value, want) in [
        ("", Some(DisplaySpec::None)),
        ("hidden", Some(DisplaySpec::None)),
        ("until-found", None),
        ("UNTIL-FOUND", None),
    ] {
        tree.set_attribute(id, "hidden", value).unwrap();
        assert_eq!(display(&tree), want, "{value:?}");
    }
    tree.set_attribute(id, "hidden", true).unwrap();
    assert_eq!(display(&tree), Some(DisplaySpec::None));
    tree.set_style(id, "display", "flex").unwrap();
    assert_eq!(display(&tree), Some(DisplaySpec::Flex));
    tree.remove_style(id, "display").unwrap();
    tree.remove_attribute(id, "hidden").unwrap();
    assert_eq!(display(&tree), None);
}

#[test]
fn an_inert_node_and_its_subtree_are_inert() {
    let mut tree = VirtualTree::new();
    let [outer, inert, child, sibling] = ["div"; 4].map(|tag| tree.create_node(tag).unwrap());
    tree.append_child(outer, inert).unwrap();
    tree.append_child(inert, child).unwrap();
    tree.append_child(outer, sibling).unwrap();
    tree.set_attribute(inert, "inert", true).unwrap();
    let inerts = |root| {
        let spec = build_spec(&tree, root).unwrap();
        let mut seen = vec![(spec.id, spec.inert)];
        let mut stack = spec.children;
        while let Some(spec) = stack.pop() {
            seen.push((spec.id, spec.inert));
            stack.extend(spec.children);
        }
        seen.sort_unstable();
        seen
    };
    assert_eq!(
        inerts(outer),
        [
            (outer, false),
            (inert, true),
            (child, true),
            (sibling, false)
        ]
    );
    // A subtree built from the child alone still sees the inert ancestor.
    assert_eq!(inerts(child), [(child, true)]);
}

#[test]
fn text_tag_uses_its_value_attribute() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("text").unwrap();
    tree.set_attribute(id, "value", "hello").unwrap();

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(spec.tag, ElementTag::Text("hello".into()));
}

#[test]
fn text_tag_uses_its_child_text() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("text").unwrap();
    let child = tree.create_node("text").unwrap();
    tree.set_attribute(child, "value", "Hello").unwrap();
    tree.append_child(id, child).unwrap();

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(spec.tag, ElementTag::Text("Hello".into()));
    assert!(spec.children.is_empty());
}

#[test]
fn text_tag_puts_its_value_before_its_children_in_order() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("text").unwrap();
    tree.set_attribute(id, "value", "a").unwrap();
    for part in ["b", "c"] {
        let child = tree.create_node("text").unwrap();
        tree.set_attribute(child, "value", part).unwrap();
        tree.append_child(id, child).unwrap();
    }

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(spec.tag, ElementTag::Text("abc".into()));
}

#[test]
fn text_tag_includes_text_nested_in_any_tag() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("text").unwrap();
    let wrapper = tree.create_node("div").unwrap();
    let leaf = tree.create_node("text").unwrap();
    tree.set_attribute(leaf, "value", "nested").unwrap();
    tree.append_child(wrapper, leaf).unwrap();
    tree.append_child(id, wrapper).unwrap();

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(spec.tag, ElementTag::Text("nested".into()));
}

#[test]
fn text_tag_counts_a_non_string_value_as_empty() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("text").unwrap();
    tree.set_attribute(id, "value", 1.0).unwrap();
    let child = tree.create_node("text").unwrap();
    tree.set_attribute(child, "value", "kept").unwrap();
    tree.append_child(id, child).unwrap();

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(spec.tag, ElementTag::Text("kept".into()));
}

#[test]
fn text_tag_missing_value_is_empty_content() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("text").unwrap();

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(spec.tag, ElementTag::Text(String::new()));
}

#[test]
fn children_are_built_in_append_order() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let a = tree.create_node("div").unwrap();
    let b = tree.create_node("div").unwrap();
    tree.append_child(parent, a).unwrap();
    tree.append_child(parent, b).unwrap();

    let spec = build_spec(&tree, parent).unwrap();
    let child_ids: Vec<NodeId> = spec.children.iter().map(|c| c.id).collect();
    assert_eq!(child_ids, &[a, b]);
}
