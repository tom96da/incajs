// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Searching the descendants of a node for a selector.

use super::matching::{Node, matching_context};
use super::{
    Element, NodeId, SelectorCaches, SelectorError, VirtualTree, matches_selector_list, parse,
};

/// The descendants of `start` that match `selector`, in document order. The walk stops at the
/// first match when `first_only` is set.
fn search(
    tree: &VirtualTree,
    start: NodeId,
    selector: &str,
    first_only: bool,
) -> Result<Vec<NodeId>, SelectorError> {
    let list = parse(selector)?;
    let mut found = Vec::new();
    let Some(root) = Node::of(tree, start) else {
        return Ok(found);
    };
    let mut caches = SelectorCaches::default();
    let mut context = matching_context(&mut caches, root.opaque());
    let mut pending: Vec<NodeId> = root.node.children().iter().rev().copied().collect();
    while let Some(id) = pending.pop() {
        let Some(element) = Node::of(tree, id) else {
            continue;
        };
        if matches_selector_list(&list, &element, &mut context) {
            found.push(id);
            if first_only {
                break;
            }
        }
        pending.extend(element.node.children().iter().rev().copied());
    }
    Ok(found)
}

/// The first descendant of `node` that matches `selector` in document order. `:scope` and a
/// top-level `&` are `node`. A node that is not an element, such as a text leaf or a node tagged
/// `#comment`, and an unknown id find nothing.
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn query_selector(
    tree: &VirtualTree,
    node: NodeId,
    selector: &str,
) -> Result<Option<NodeId>, SelectorError> {
    Ok(search(tree, node, selector, true)?.first().copied())
}

/// Every descendant of `node` that matches `selector`, each once, in document order. The other
/// rules are those of [`query_selector`].
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn query_selector_all(
    tree: &VirtualTree,
    node: NodeId,
    selector: &str,
) -> Result<Vec<NodeId>, SelectorError> {
    search(tree, node, selector, false)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use rstest::rstest;

    /// `root > (a > (a1, a2), b)`, created in the order a, a1, b, a2, so that the creation order,
    /// the level order and the document order differ.
    fn forest() -> (Fixture, [NodeId; 4]) {
        let mut f = Fixture::new();
        let a = f.add(f.root, "div", "a");
        let a1 = f.add(a, "div", "a1");
        let b = f.add(f.root, "div", "b");
        let a2 = f.add(a, "div", "a2");
        (f, [a, a1, a2, b])
    }

    #[test]
    fn query_selector_all_lists_matches_in_document_order() {
        let (f, [a, a1, a2, b]) = forest();

        assert_eq!(
            query_selector_all(&f.tree, f.root, "*").unwrap(),
            [a, a1, a2, b]
        );
    }

    #[test]
    fn a_selector_list_yields_one_ordered_sequence_without_duplicates() {
        let p = page();

        assert_eq!(
            query_selector_all(&p.f.tree, p.f.root, "span, p, #p1").unwrap(),
            [p.p1, p.p2, p.p3]
        );
    }

    #[test]
    fn query_selector_returns_the_first_match_in_document_order() {
        let p = page();

        assert_eq!(
            query_selector(&p.f.tree, p.f.root, "p").unwrap(),
            Some(p.p1)
        );
    }

    #[test]
    fn query_selector_returns_none_without_a_match() {
        let p = page();

        assert_eq!(
            query_selector(&p.f.tree, p.f.root, "article").unwrap(),
            None
        );
    }

    #[test]
    fn a_matching_start_node_is_not_in_its_own_result() {
        let p = page();

        assert_eq!(
            query_selector_all(&p.f.tree, p.main, "section, p").unwrap(),
            [p.p1, p.p3]
        );
    }

    #[rstest]
    #[case::scope_child(":scope > p", &["p1", "p3"])]
    #[case::ampersand_child("& > p", &["p1", "p3"])]
    #[case::scope_alone(":scope", &[])]
    #[case::ampersand_alone("&", &[])]
    #[case::negated_scope(":not(:scope)", &["p1", "p2", "p3"])]
    fn scope_selects_the_start_node(#[case] selector: &str, #[case] expected: &[&str]) {
        let p = page();

        assert_eq!(
            query_selector_all(&p.f.tree, p.main, selector).unwrap(),
            expected.iter().map(|name| p.node(name)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn ancestors_outside_the_start_node_take_part_in_the_match() {
        let p = page();

        assert_eq!(
            query_selector_all(&p.f.tree, p.main, "div p").unwrap(),
            [p.p1, p.p3]
        );
    }

    #[test]
    fn text_leaves_and_comment_stand_ins_are_not_results() {
        let m = mixed();

        assert_eq!(
            query_selector_all(&m.f.tree, m.parent, "*").unwrap(),
            [m.a, m.b]
        );
    }

    #[rstest]
    #[case::unknown_id(9999)]
    #[case::text_leaf(mixed().text_node)]
    #[case::comment_stand_in(mixed().comment)]
    fn an_unknown_text_or_comment_start_gives_an_empty_result(#[case] start: NodeId) {
        let m = mixed();

        assert!(
            query_selector_all(&m.f.tree, start, "*")
                .unwrap()
                .is_empty()
        );
        assert_eq!(query_selector(&m.f.tree, start, "*").unwrap(), None);
    }

    #[test]
    fn a_parentless_node_searches_its_own_subtree() {
        let mut tree = VirtualTree::new();
        let top = tree.create_node("div").unwrap();
        let child = tree.create_node("p").unwrap();
        tree.append_child(top, child).unwrap();

        assert_eq!(query_selector_all(&tree, top, "p").unwrap(), [child]);
    }

    #[test]
    fn query_selector_all_walks_a_deep_chain_without_overflow() {
        let mut tree = VirtualTree::new();
        let mut top = tree.create_node("div").unwrap();
        for _ in 1..100_000 {
            let parent = tree.create_node("div").unwrap();
            tree.append_child(parent, top).unwrap();
            top = parent;
        }

        assert_eq!(query_selector_all(&tree, top, "*").unwrap().len(), 99_999);
    }
}
