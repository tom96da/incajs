// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use rstest::rstest;

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
fn append_child_keeps_the_call_order() {
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
fn insert_before_places_a_node_at_the_start_middle_and_end() {
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
        "a stale id never names another node"
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
fn set_attribute_replaces_an_existing_value() {
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

#[rstest]
#[case::camel("flexDirection")]
#[case::kebab("flex-direction")]
#[case::snake("flex_direction")]
fn a_style_key_reads_back_in_snake_case_whatever_its_spelling(#[case] key: &str) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, key, "column").unwrap();
    let props = tree.get(id).unwrap().style_props();
    assert_eq!(props.len(), 1, "{key}");
    assert_eq!(props.get("flex_direction"), Some(&"column".into()), "{key}");
}

#[test]
fn a_style_key_in_another_spelling_overwrites_the_value() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "flexDirection", "row").unwrap();
    tree.set_style(id, "flex-direction", "column").unwrap();
    let props = tree.get(id).unwrap().style_props();
    assert_eq!(props.len(), 1);
    assert_eq!(props.get("flex_direction"), Some(&"column".into()));
}

#[test]
fn a_style_key_in_another_spelling_removes_the_value() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "flex-direction", "column").unwrap();

    tree.remove_style(id, "flexDirection").unwrap();
    assert!(tree.get(id).unwrap().style_props().is_empty());
}

#[test]
fn style_key_borrows_a_snake_case_key() {
    assert!(matches!(style_key("flex_direction"), Cow::Borrowed(_)));
}

#[rstest]
#[case::camel("flexDirection", "flex_direction")]
#[case::kebab("flex-direction", "flex_direction")]
#[case::leading_capital("FlexDirection", "_flex_direction")]
#[case::double_dash("--x", "__x")]
#[case::dash_before_capital("flex-Direction", "flex__direction")]
#[case::empty("", "")]
#[case::multibyte_before_capital("é-Z", "é__z")]
#[case::multibyte_only("幅", "幅")]
fn a_style_key_normalizes_to_snake_case_in_every_edge_case(#[case] key: &str, #[case] want: &str) {
    assert_eq!(style_key(key), want, "{key}");
}

#[test]
fn ancestors_lists_the_node_then_its_parents_up_to_the_root() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    let mid = tree.create_node("div").unwrap();
    let leaf = tree.create_node("div").unwrap();
    tree.append_child(root, mid).unwrap();
    tree.append_child(mid, leaf).unwrap();

    assert_eq!(tree.ancestors(leaf).collect::<Vec<_>>(), [leaf, mid, root]);
    assert_eq!(tree.ancestors(root).collect::<Vec<_>>(), [root]);
}

#[test]
fn ancestors_of_an_unknown_id_yields_only_that_id() {
    let tree = VirtualTree::new();
    assert_eq!(tree.ancestors(7).collect::<Vec<_>>(), [7]);
}

#[rstest]
#[case::button("button", &["disabled"], true)]
#[case::input("input", &["disabled"], true)]
#[case::div("div", &["disabled"], false)]
#[case::enabled_button("button", &[], false)]
fn is_disabled_control_needs_a_control_tag_and_the_disabled_attribute(
    #[case] tag: &str,
    #[case] attributes: &[&str],
    #[case] want: bool,
) {
    let mut tree = VirtualTree::new();
    let node = tree.create_node(tag).unwrap();
    for attribute in attributes {
        tree.set_attribute(node, *attribute, true).unwrap();
    }
    assert_eq!(is_disabled_control(tree.get(node).unwrap()), want, "{tag}");
}

#[test]
fn a_tree_without_a_root_connects_every_id() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    assert!(tree.is_connected(id));
    assert!(tree.is_connected(999));
}

#[test]
fn a_chain_attached_to_the_root_is_connected() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    let mid = tree.create_node("div").unwrap();
    let leaf = tree.create_node("div").unwrap();
    tree.set_root(root);
    tree.append_child(root, mid).unwrap();
    tree.append_child(mid, leaf).unwrap();
    assert!(tree.is_connected(root));
    assert!(tree.is_connected(mid));
    assert!(tree.is_connected(leaf));
}

#[test]
fn a_detached_subtree_is_not_connected_until_it_is_attached_again() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    let mid = tree.create_node("div").unwrap();
    let leaf = tree.create_node("div").unwrap();
    tree.set_root(root);
    tree.append_child(root, mid).unwrap();
    tree.append_child(mid, leaf).unwrap();

    tree.remove_child(root, mid).unwrap();
    assert!(!tree.is_connected(mid));
    assert!(!tree.is_connected(leaf));

    tree.append_child(root, mid).unwrap();
    assert!(tree.is_connected(leaf));
}

#[test]
fn a_destroyed_node_is_not_connected_under_a_root() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    let child = tree.create_node("div").unwrap();
    tree.set_root(root);
    tree.append_child(root, child).unwrap();
    tree.destroy_node(child);
    assert!(!tree.is_connected(child));
}

#[test]
fn an_unknown_id_is_not_connected_under_a_root() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
    assert!(!tree.is_connected(999));
}

fn orphan_pair(tree: &mut VirtualTree) -> (NodeId, NodeId) {
    let parent = tree.create_node("div").unwrap();
    let child = tree.create_node("span").unwrap();
    tree.append_child(parent, child).unwrap();
    (parent, child)
}

#[test]
fn sweep_frees_an_orphan_and_its_subtree() {
    let mut tree = VirtualTree::new();
    let (parent, child) = orphan_pair(&mut tree);

    assert_eq!(tree.sweep(), [parent, child]);
    assert!(tree.get(parent).is_none());
    assert!(tree.get(child).is_none());
    assert!(!tree.has_orphans());
}

#[test]
fn an_insert_before_the_sweep_saves_the_node() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
    let (parent, child) = orphan_pair(&mut tree);
    tree.append_child(root, parent).unwrap();

    assert!(tree.sweep().is_empty());
    assert!(tree.get(parent).is_some());
    assert!(tree.get(child).is_some());
}

#[test]
fn a_held_parentless_node_survives_until_it_is_released() {
    let mut tree = VirtualTree::new();
    let (parent, child) = orphan_pair(&mut tree);
    tree.hold(parent).unwrap();

    assert!(tree.sweep().is_empty());
    assert!(tree.get(child).is_some());

    tree.release(parent);
    assert!(tree.has_orphans());
    assert_eq!(tree.sweep(), [parent, child]);
}

#[test]
fn a_held_attached_node_stays_after_it_is_detached() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
    let node = tree.create_node("div").unwrap();
    tree.append_child(root, node).unwrap();
    tree.hold(node).unwrap();
    tree.remove_child(root, node).unwrap();

    assert!(tree.sweep().is_empty());
    assert!(tree.get(node).is_some());
}

#[test]
fn destroy_node_clears_the_hold_and_the_orphan_entry() {
    let mut tree = VirtualTree::new();
    let held = tree.create_node("div").unwrap();
    let loose = tree.create_node("div").unwrap();
    tree.hold(held).unwrap();

    assert_eq!(tree.destroy_node(held), [held]);
    assert_eq!(tree.destroy_node(loose), [loose]);

    assert!(!tree.has_orphans());
    assert!(tree.sweep().is_empty());
    assert!(!tree.held.contains(&held));
}

#[test]
fn an_empty_orphan_set_sweeps_nothing() {
    let mut tree = VirtualTree::new();
    assert!(!tree.has_orphans());
    assert!(tree.sweep().is_empty());
}

#[test]
fn a_node_created_and_never_inserted_is_an_orphan() {
    let mut tree = VirtualTree::new();
    tree.create_node("div").unwrap();
    assert!(tree.has_orphans());
}

#[test]
fn release_of_an_attached_node_makes_no_orphan() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
    let node = tree.create_node("div").unwrap();
    tree.append_child(root, node).unwrap();
    tree.hold(node).unwrap();

    tree.release(node);

    assert!(!tree.has_orphans());
}

#[test]
fn the_root_is_never_an_orphan() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
    tree.hold(root).unwrap();
    tree.release(root);

    assert!(!tree.has_orphans());
    assert!(tree.sweep().is_empty());
    assert_eq!(tree.root(), Some(root));
}

#[test]
fn hold_of_an_unknown_id_errors() {
    let mut tree = VirtualTree::new();
    assert_eq!(tree.hold(7), Err(TreeError::NodeNotFound(7)));
}

#[test]
fn a_detached_node_is_an_orphan_until_it_is_inserted() {
    let mut tree = VirtualTree::new();
    let root = tree.create_node("div").unwrap();
    tree.set_root(root);
    let node = tree.create_node("div").unwrap();
    tree.append_child(root, node).unwrap();
    assert!(!tree.has_orphans());

    tree.remove_child(root, node).unwrap();

    assert!(tree.has_orphans());
    assert_eq!(tree.sweep(), [node]);
}

#[test]
fn a_held_node_under_a_swept_orphan_is_freed_with_it() {
    let mut tree = VirtualTree::new();
    let (parent, child) = orphan_pair(&mut tree);
    tree.hold(child).unwrap();

    assert_eq!(tree.sweep(), [parent, child]);
    assert!(tree.held.is_empty());

    tree.release(child);
    assert!(!tree.has_orphans());
}

#[test]
fn preceding_is_none_without_a_parent() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();

    assert_eq!(tree.preceding(id), None);
}

#[test]
fn preceding_of_a_first_child_is_its_parent() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let child = tree.create_node("div").unwrap();
    tree.append_child(parent, child).unwrap();

    assert_eq!(tree.preceding(child), Some(parent));
}

#[test]
fn preceding_is_the_last_node_before_the_subtree() {
    let mut tree = VirtualTree::new();
    let parent = tree.create_node("div").unwrap();
    let first = tree.create_node("div").unwrap();
    let inner = tree.create_node("div").unwrap();
    let deepest = tree.create_node("div").unwrap();
    let second = tree.create_node("div").unwrap();
    tree.append_child(parent, first).unwrap();
    tree.append_child(first, inner).unwrap();
    tree.append_child(inner, deepest).unwrap();
    tree.append_child(parent, second).unwrap();

    assert_eq!(tree.preceding(second), Some(deepest));
}
