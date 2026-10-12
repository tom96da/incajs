// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;

#[test]
fn id_uniqueness() {
    let mut tree = VirtualTree::new();
    let ids: Vec<NodeId> = (0..100).map(|_| tree.create_node("div").unwrap()).collect();

    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ids.len(),
        "create_node must never reuse an id"
    );
}

#[test]
fn create_node_fails_once_the_id_space_is_used_up() {
    let mut tree = VirtualTree::new();
    tree.next_id = NodeId::MAX - 1;

    let last = tree.create_node("div").unwrap();
    assert_eq!(last, NodeId::MAX - 1);
    assert_eq!(tree.create_node("div"), Err(TreeError::IdsExhausted));
    assert_eq!(tree.create_node("div"), Err(TreeError::IdsExhausted));
    assert_eq!(tree.nodes.len(), 1);
    assert!(tree.get(last).is_some());
}

#[test]
fn ids_exhausted_has_a_message() {
    assert_eq!(
        TreeError::IdsExhausted.to_string(),
        "node id space exhausted"
    );
}

#[test]
fn append_order() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let a = tree.create_node("span").unwrap();
    let b = tree.create_node("span").unwrap();
    let c = tree.create_node("span").unwrap();

    tree.append_child(parent, a).unwrap();
    tree.append_child(parent, b).unwrap();
    tree.append_child(parent, c).unwrap();

    assert_eq!(tree.get(parent).unwrap().children(), &[a, b, c]);
}

#[test]
fn insert_before_at_start_middle_end() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let a = tree.create_node("span").unwrap();
    let b = tree.create_node("span").unwrap();
    let c = tree.create_node("span").unwrap();

    tree.insert_before(parent, b, None).unwrap(); // end: [b]
    tree.insert_before(parent, a, Some(b)).unwrap(); // start: [a, b]
    tree.insert_before(parent, c, None).unwrap(); // end: [a, b, c]

    assert_eq!(tree.get(parent).unwrap().children(), &[a, b, c]);

    let d = tree.create_node("span").unwrap();
    tree.insert_before(parent, d, Some(b)).unwrap(); // middle: [a, d, b, c]
    assert_eq!(tree.get(parent).unwrap().children(), &[a, d, b, c]);
}

#[test]
fn insert_before_itself_is_a_no_op() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let a = tree.create_node("span").unwrap();
    let b = tree.create_node("span").unwrap();
    tree.insert_before(parent, a, None).unwrap();
    tree.insert_before(parent, b, None).unwrap();

    tree.insert_before(parent, a, Some(a)).unwrap();

    assert_eq!(tree.get(parent).unwrap().children(), &[a, b]);
}

#[test]
fn insert_before_unknown_anchor_falls_back_to_append() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let a = tree.create_node("span").unwrap();
    let stray_anchor = tree.create_node("span").unwrap(); // real node, never attached here

    tree.insert_before(parent, a, Some(stray_anchor)).unwrap();

    assert_eq!(tree.get(parent).unwrap().children(), &[a]);
}

#[test]
fn insert_before_unknown_ids_error() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let child = tree.create_node("span").unwrap();

    assert_eq!(
        tree.insert_before(999, child, None).unwrap_err(),
        TreeError::NodeNotFound(999)
    );
    assert_eq!(
        tree.insert_before(parent, 999, None).unwrap_err(),
        TreeError::NodeNotFound(999)
    );
    assert_eq!(
        tree.insert_before(parent, child, Some(999)).unwrap_err(),
        TreeError::NodeNotFound(999)
    );
}

#[test]
fn a_node_has_one_parent_at_a_time() {
    let mut tree = VirtualTree::new();
    let first = tree.create_node("div").unwrap();
    let second = tree.create_node("div").unwrap();
    let child = tree.create_node("span").unwrap();

    tree.append_child(first, child).unwrap();
    tree.append_child(second, child).unwrap();

    assert!(
        tree.get(first).unwrap().children().is_empty(),
        "attaching elsewhere must detach from the previous parent"
    );
    assert_eq!(tree.get(second).unwrap().children(), &[child]);
    assert_eq!(tree.get(child).unwrap().parent(), Some(second));
}

#[test]
fn a_fresh_node_has_no_parent() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();

    assert_eq!(tree.get(node).unwrap().parent(), None);
}

#[test]
fn attaching_a_node_under_itself_is_refused() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();

    assert_eq!(
        tree.append_child(node, node).unwrap_err(),
        TreeError::WouldCycle(node)
    );
}

#[test]
fn attaching_an_ancestor_under_its_descendant_is_refused() {
    let mut tree = VirtualTree::new();
    let grandparent = tree.create_node("div").unwrap();
    let parent = tree.create_node("div").unwrap();
    let child = tree.create_node("div").unwrap();
    tree.append_child(grandparent, parent).unwrap();
    tree.append_child(parent, child).unwrap();

    assert_eq!(
        tree.append_child(child, grandparent).unwrap_err(),
        TreeError::WouldCycle(grandparent),
        "a cycle here would leave the render walk with no end"
    );
    assert_eq!(tree.get(grandparent).unwrap().children(), &[parent]);
}

#[test]
fn moving_a_child_within_one_parent_reorders_it() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let a = tree.create_node("span").unwrap();
    let b = tree.create_node("span").unwrap();
    tree.append_child(parent, a).unwrap();
    tree.append_child(parent, b).unwrap();

    tree.insert_before(parent, b, Some(a)).unwrap();

    assert_eq!(
        tree.get(parent).unwrap().children(),
        &[b, a],
        "a move must not leave the node listed twice"
    );
}

#[test]
fn destroy_frees_the_whole_subtree() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    let branch = tree.create_node("div").unwrap();
    let leaf = tree.create_node("text").unwrap();
    tree.append_child(root, branch).unwrap();
    tree.append_child(branch, leaf).unwrap();

    let mut freed = tree.destroy_node(branch);
    freed.sort_unstable();

    assert_eq!(
        freed,
        vec![branch, leaf],
        "a descendant is unreachable once its root is gone, so it must go too"
    );
    assert!(tree.get(branch).is_none());
    assert!(tree.get(leaf).is_none());
    assert!(tree.get(root).unwrap().children().is_empty());
}

#[test]
fn destroy_is_a_no_op_the_second_time_and_for_an_unknown_id() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();

    assert_eq!(tree.destroy_node(node), vec![node]);
    assert!(tree.destroy_node(node).is_empty());
    assert!(tree.destroy_node(9999).is_empty());
}

#[test]
fn destroyed_ids_are_not_handed_out_again() {
    let mut tree = VirtualTree::new();
    let first = tree.create_node("div").unwrap();
    tree.destroy_node(first);

    assert_ne!(
        tree.create_node("div").unwrap(),
        first,
        "a stale id must name nothing rather than something else"
    );
}

#[test]
fn remove_child_clears_the_parent_link() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let child = tree.create_node("span").unwrap();
    tree.append_child(parent, child).unwrap();

    tree.remove_child(parent, child).unwrap();

    assert_eq!(tree.get(child).unwrap().parent(), None);
}

#[test]
fn remove_child_leaves_a_link_to_a_different_parent_alone() {
    let mut tree = VirtualTree::new();
    let real_parent = tree.create_node("div").unwrap();
    let stranger = tree.create_node("div").unwrap();
    let child = tree.create_node("span").unwrap();
    tree.append_child(real_parent, child).unwrap();

    tree.remove_child(stranger, child).unwrap();

    assert_eq!(tree.get(child).unwrap().parent(), Some(real_parent));
}

#[test]
fn detach_keeps_node_alive() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let child = tree.create_node("span").unwrap();
    tree.append_child(parent, child).unwrap();

    tree.remove_child(parent, child).unwrap();

    assert!(tree.get(parent).unwrap().children().is_empty());
    assert!(
        tree.get(child).is_some(),
        "detaching a child must not deallocate it"
    );
}

#[test]
fn remove_of_absent_child_is_a_no_op() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let never_appended = tree.create_node("span").unwrap();

    // Absent child that exists elsewhere in the tree.
    assert!(tree.remove_child(parent, never_appended).is_ok());
    assert!(tree.get(parent).unwrap().children().is_empty());

    // Absent child id that was never allocated at all.
    assert!(tree.remove_child(parent, 9999).is_ok());
}

#[test]
fn set_attribute_overwrites() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("input").unwrap();

    tree.set_attribute(node, "value", "first").unwrap();
    tree.set_attribute(node, "value", "second").unwrap();

    assert_eq!(
        tree.get(node).unwrap().attributes().get("value"),
        Some(&AttributeValue::String("second".into()))
    );
}

#[test]
fn unknown_id_lookup_returns_none() {
    let tree = VirtualTree::new();
    assert!(tree.get(42).is_none());
}

#[test]
fn operations_on_unknown_ids_error_instead_of_panicking() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();

    assert_eq!(
        tree.append_child(node, 999).unwrap_err(),
        TreeError::NodeNotFound(999)
    );
    assert_eq!(
        tree.append_child(999, node).unwrap_err(),
        TreeError::NodeNotFound(999)
    );
    assert_eq!(
        tree.remove_child(999, node).unwrap_err(),
        TreeError::NodeNotFound(999)
    );
    assert_eq!(
        tree.set_attribute(999, "k", "v").unwrap_err(),
        TreeError::NodeNotFound(999)
    );
    assert_eq!(
        tree.set_style(999, "k", "v").unwrap_err(),
        TreeError::NodeNotFound(999)
    );
    assert_eq!(
        tree.remove_style(999, "k").unwrap_err(),
        TreeError::NodeNotFound(999)
    );
}

#[test]
fn remove_attribute_unsets_only_that_key() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();
    tree.set_attribute(node, "label", "a").unwrap();
    tree.set_attribute(node, "count", 3.0).unwrap();

    tree.remove_attribute(node, "label").unwrap();

    let attrs = tree.get(node).unwrap().attributes();
    assert!(attrs.get("label").is_none());
    assert_eq!(attrs.get("count"), Some(&AttributeValue::Number(3.0)));
}

#[test]
fn remove_attribute_of_an_unset_key_succeeds() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();
    tree.set_attribute(node, "label", "a").unwrap();

    tree.remove_attribute(node, "never_set").unwrap();
    tree.remove_attribute(node, "label").unwrap();
    tree.remove_attribute(node, "label").unwrap();

    assert!(tree.get(node).unwrap().attributes().is_empty());
}

#[test]
fn remove_attribute_on_an_unknown_node_errors() {
    let mut tree = VirtualTree::new();
    assert_eq!(
        tree.remove_attribute(999, "k").unwrap_err(),
        TreeError::NodeNotFound(999)
    );
}

#[test]
fn remove_attribute_leaves_the_rest_of_the_node_unchanged() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let node = tree.create_node("span").unwrap();
    tree.append_child(parent, node).unwrap();
    tree.set_attribute(node, "label", "a").unwrap();
    tree.set_style(node, "label", "b").unwrap();

    tree.remove_attribute(node, "label").unwrap();

    let got = tree.get(node).unwrap();
    assert_eq!(got.tag_name(), "span");
    assert_eq!(got.parent(), Some(parent));
    assert_eq!(
        got.style_props().get("label"),
        Some(&AttributeValue::String("b".into()))
    );
}

#[test]
fn remove_style_unsets_only_that_key() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();
    tree.set_style(node, "gap", 8.0).unwrap();
    tree.set_style(node, "width", 10.0).unwrap();

    tree.remove_style(node, "gap").unwrap();
    tree.remove_style(node, "never_set").unwrap();

    let styles = tree.get(node).unwrap().style_props();
    assert!(styles.get("gap").is_none());
    assert!(styles.get("width").is_some());
}

#[test]
fn set_style_is_independent_of_attributes() {
    let mut tree = VirtualTree::new();
    let node = tree.create_node("div").unwrap();

    tree.set_style(node, "color", "red").unwrap();

    let got = tree.get(node).unwrap();
    assert_eq!(
        got.style_props().get("color"),
        Some(&AttributeValue::String("red".into()))
    );
    assert!(got.attributes().get("color").is_none());
}

#[test]
fn a_style_key_reads_back_in_snake_case_whatever_its_spelling() {
    for key in ["flexDirection", "flex-direction", "flex_direction"] {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, key, "column").unwrap();
        let props = tree.get(id).unwrap().style_props();
        assert_eq!(props.len(), 1, "{key}");
        assert_eq!(props.get("flex_direction"), Some(&"column".into()), "{key}");
    }
}

#[test]
fn a_style_key_in_another_spelling_overwrites_and_removes() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "flexDirection", "row").unwrap();
    tree.set_style(id, "flex-direction", "column").unwrap();
    let props = tree.get(id).unwrap().style_props();
    assert_eq!(props.len(), 1);
    assert_eq!(props.get("flex_direction"), Some(&"column".into()));

    tree.remove_style(id, "flexDirection").unwrap();
    assert!(tree.get(id).unwrap().style_props().is_empty());
}

#[test]
fn style_key_borrows_a_snake_case_key() {
    assert!(matches!(style_key("flex_direction"), Cow::Borrowed(_)));
}

#[test]
fn style_key_edge_cases() {
    for (key, want) in [
        ("flexDirection", "flex_direction"),
        ("flex-direction", "flex_direction"),
        ("FlexDirection", "_flex_direction"),
        ("--x", "__x"),
        ("flex-Direction", "flex__direction"),
        ("", ""),
        ("é-Z", "é__z"),
        ("幅", "幅"),
    ] {
        assert_eq!(style_key(key), want, "{key}");
    }
}
