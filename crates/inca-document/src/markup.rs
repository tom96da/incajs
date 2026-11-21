// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The algorithms of the markup setters `innerHTML`, `outerHTML` and `insertAdjacentHTML`. A
//! setter parses the markup in the context of a node, then puts the parsed nodes in place and
//! reports each node that leaves the tree.

use inca_node::{NodeId, ParsedHtml, Removal, TreeError, VirtualNode, VirtualTree};

/// What the algorithms need to reach the tree they change and the steps that follow a tree call.
pub trait MarkupContext {
    /// The tree the setters change.
    fn tree(&mut self) -> &mut VirtualTree;

    /// Runs the steps that follow a tree call which moved or removed `removals`.
    fn settle(&mut self, removals: &[Removal]);
}

/// Where `insertAdjacentHTML` puts the nodes, relative to the node it is called on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    BeforeBegin,
    AfterBegin,
    BeforeEnd,
    AfterEnd,
}

impl Position {
    /// Reads `text` ignoring ASCII case. Any other string gives `None`.
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "beforebegin" => Some(Self::BeforeBegin),
            "afterbegin" => Some(Self::AfterBegin),
            "beforeend" => Some(Self::BeforeEnd),
            "afterend" => Some(Self::AfterEnd),
            _ => None,
        }
    }
}

/// The children of `id`, or none when the node is gone.
#[must_use]
pub fn children_of(tree: &VirtualTree, id: NodeId) -> Vec<NodeId> {
    tree.get(id)
        .map_or_else(Vec::new, |node| node.children().to_vec())
}

/// Parses `markup` for `innerHTML` on `node_id`. Returns the node that takes the children, which
/// is the content of a template, and the parse result.
///
/// # Errors
///
/// `NodeNotFound` when `node_id` is gone, or the error of the parse.
pub fn parse_inner(
    tree: &mut VirtualTree,
    node_id: NodeId,
    markup: &str,
) -> Result<(NodeId, ParsedHtml), TreeError> {
    tree.get(node_id).ok_or(TreeError::NodeNotFound(node_id))?;
    let container = tree.template_content(node_id).unwrap_or(node_id);
    let parsed = tree.parse_html(Some(node_id), markup)?;
    Ok((container, parsed))
}

/// Parses `markup` for `outerHTML` on `node_id` in the context of its parent. A node without a
/// parent gives `None`.
///
/// # Errors
///
/// `NodeNotFound` when `node_id` is gone, `DocumentParent` when its parent is a document, or the
/// error of the parse.
pub fn parse_outer(
    tree: &mut VirtualTree,
    node_id: NodeId,
    markup: &str,
) -> Result<Option<ParsedHtml>, TreeError> {
    let node = tree.get(node_id).ok_or(TreeError::NodeNotFound(node_id))?;
    let Some(parent) = node.parent() else {
        return Ok(None);
    };
    if tree.get(parent).is_some_and(VirtualNode::is_document) {
        return Err(TreeError::DocumentParent(node_id));
    }
    tree.parse_html(Some(parent), markup).map(Some)
}

/// Parses `markup` for `insertAdjacentHTML` at `position` of `node_id`. Returns the node that
/// takes the nodes, the node they go before (the last place without one) and the parse result.
///
/// # Errors
///
/// `NodeNotFound` when `node_id` is gone, `NoParent` or `DocumentParent` for a sibling position
/// without an element parent, or the error of the parse.
pub fn parse_adjacent(
    tree: &mut VirtualTree,
    node_id: NodeId,
    position: Position,
    markup: &str,
) -> Result<(NodeId, Option<NodeId>, ParsedHtml), TreeError> {
    let node = tree.get(node_id).ok_or(TreeError::NodeNotFound(node_id))?;
    let sibling = matches!(position, Position::BeforeBegin | Position::AfterEnd);
    let container = if sibling {
        node.parent().ok_or(TreeError::NoParent(node_id))?
    } else {
        node_id
    };
    if sibling && tree.get(container).is_some_and(VirtualNode::is_document) {
        return Err(TreeError::DocumentParent(node_id));
    }
    let context = match tree.get(container) {
        Some(node) if node.is_html("html") => None,
        _ => Some(container),
    };
    let parsed = tree.parse_html(context, markup)?;
    let reference = match position {
        Position::BeforeBegin => Some(node_id),
        Position::AfterBegin => tree.first_child(node_id),
        Position::BeforeEnd => None,
        Position::AfterEnd => tree.next_sibling(node_id),
    };
    Ok((container, reference, parsed))
}

/// Moves the nodes of the parse result into `container` before `reference`, or last without one.
///
/// # Errors
///
/// The error of the insertion.
pub fn insert_parsed(
    context: &mut impl MarkupContext,
    parsed: &ParsedHtml,
    container: NodeId,
    reference: Option<NodeId>,
) -> Result<(), TreeError> {
    let tree = context.tree();
    let left = tree.removals(&tree.nodes_to_move(parsed.fragment));
    tree.insert_before(container, parsed.fragment, reference)?;
    context.settle(&left);
    Ok(())
}

/// Removes every child of `container` and puts the parse result in. Returns the old children.
///
/// # Errors
///
/// The error of a removal or of the insertion.
pub fn replace_children(
    context: &mut impl MarkupContext,
    container: NodeId,
    parsed: &ParsedHtml,
) -> Result<Vec<NodeId>, TreeError> {
    let old = children_of(context.tree(), container);
    for &child in &old {
        let tree = context.tree();
        let left = tree.removals(&[child]);
        tree.remove_child(container, child)?;
        context.settle(&left);
    }
    insert_parsed(context, parsed, container, None)?;
    Ok(old)
}

/// Replaces `node_id` with the parse result. Returns `false` and changes nothing when the node has
/// no parent.
///
/// # Errors
///
/// The error of the replacement.
pub fn replace_with(
    context: &mut impl MarkupContext,
    node_id: NodeId,
    parsed: &ParsedHtml,
) -> Result<bool, TreeError> {
    let tree = context.tree();
    let Some(parent) = tree.get(node_id).and_then(VirtualNode::parent) else {
        return Ok(false);
    };
    let mut left = tree.removals(&[node_id]);
    left.extend(tree.removals(&tree.nodes_to_move(parsed.fragment)));
    tree.replace_child(parent, parsed.fragment, node_id)?;
    context.settle(&left);
    Ok(true)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::rooted_tree;

    /// A context that records the ids each settle call reports.
    struct Recorder {
        tree: VirtualTree,
        settled: Vec<Vec<NodeId>>,
    }

    impl MarkupContext for Recorder {
        fn tree(&mut self) -> &mut VirtualTree {
            &mut self.tree
        }

        fn settle(&mut self, removals: &[Removal]) {
            self.settled.push(removals.iter().map(|r| r.id).collect());
        }
    }

    fn recorder(count: usize) -> (Recorder, Vec<NodeId>) {
        let (tree, ids) = rooted_tree(count);
        (
            Recorder {
                tree,
                settled: Vec::new(),
            },
            ids,
        )
    }

    #[rstest]
    #[case::lower("beforebegin", Some(Position::BeforeBegin))]
    #[case::mixed("AfterBegin", Some(Position::AfterBegin))]
    #[case::upper("BEFOREEND", Some(Position::BeforeEnd))]
    #[case::after_end("afterend", Some(Position::AfterEnd))]
    #[case::unknown("inside", None)]
    fn a_position_reads_ignoring_case(#[case] text: &str, #[case] expected: Option<Position>) {
        assert_eq!(Position::from_name(text), expected);
    }

    #[test]
    fn replace_children_settles_each_old_child_on_its_own() {
        let (mut context, ids) = recorder(2);
        let (container, parsed) = parse_inner(&mut context.tree, 0, "<i></i>").unwrap();

        let old = replace_children(&mut context, container, &parsed).unwrap();

        assert_eq!(old, ids);
        assert_eq!(context.settled.len(), 3);
        assert_eq!(context.settled[..2], [vec![ids[0]], vec![ids[1]]]);
    }

    #[test]
    fn replace_with_settles_the_node_and_the_moved_nodes_together() {
        let (mut context, ids) = recorder(1);
        let parsed = parse_outer(&mut context.tree, ids[0], "<i></i>")
            .unwrap()
            .unwrap();

        assert!(replace_with(&mut context, ids[0], &parsed).unwrap());

        assert_eq!(context.settled.len(), 1);
        assert_eq!(context.settled[0][0], ids[0]);
        assert!(context.settled[0].len() > 1);
    }

    #[test]
    fn replace_with_a_parentless_node_changes_nothing() {
        let (mut context, _ids) = recorder(1);
        let loose = context.tree.create_node("p").unwrap();
        let parsed = context.tree.parse_html(None, "<i></i>").unwrap();

        assert!(!replace_with(&mut context, loose, &parsed).unwrap());

        assert!(context.settled.is_empty());
    }

    #[test]
    fn parse_outer_on_a_parentless_node_gives_none() {
        let (mut tree, _ids) = rooted_tree(0);
        let loose = tree.create_node("p").unwrap();

        assert_eq!(parse_outer(&mut tree, loose, "x").unwrap(), None);
    }

    #[test]
    fn parse_outer_under_a_document_is_refused() {
        let (mut tree, _ids) = rooted_tree(0);

        assert_eq!(
            parse_outer(&mut tree, 0, "x"),
            Err(TreeError::DocumentParent(0))
        );
    }

    /// `container` and `reference` index `[root, first, second]`, the nodes of `rooted_tree(2)`.
    #[rstest]
    #[case::before_begin(Position::BeforeBegin, 0, Some(1))]
    #[case::after_begin(Position::AfterBegin, 1, None)]
    #[case::before_end(Position::BeforeEnd, 1, None)]
    #[case::after_end(Position::AfterEnd, 0, Some(2))]
    fn parse_adjacent_gives_the_insertion_point_of_each_position(
        #[case] position: Position,
        #[case] container: usize,
        #[case] reference: Option<usize>,
    ) {
        let (mut tree, ids) = rooted_tree(2);
        let nodes = [0, ids[0], ids[1]];

        let (got_container, got_reference, _parsed) =
            parse_adjacent(&mut tree, ids[0], position, "x").unwrap();

        assert_eq!(
            (got_container, got_reference),
            (nodes[container], reference.map(|i| nodes[i]))
        );
    }

    #[rstest]
    #[case::before_begin(Position::BeforeBegin)]
    #[case::after_end(Position::AfterEnd)]
    fn a_sibling_position_without_a_parent_is_refused(#[case] position: Position) {
        let (mut tree, _ids) = rooted_tree(0);
        let loose = tree.create_node("p").unwrap();

        assert_eq!(
            parse_adjacent(&mut tree, loose, position, "x").map(|_| ()),
            Err(TreeError::NoParent(loose))
        );
    }
}
