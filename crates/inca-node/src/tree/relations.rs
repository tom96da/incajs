// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Relations between nodes: the links to the neighbours, the nodes of a subtree, the top of a
//! tree, tree positions and equality.

use super::{NodeId, TreeError, VirtualNode, VirtualTree};

/// A neighbour of a node that [`VirtualTree::linked`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// The parent.
    Parent,
    /// The first child.
    First,
    /// The last child.
    Last,
    /// The sibling right before.
    Previous,
    /// The sibling right after.
    Next,
}

/// A node that is not in the tree.
fn missing(id: NodeId) -> TreeError {
    TreeError::NodeNotFound(id)
}

/// Whether two nodes carry the same attributes, compared by their text form.
fn same_attributes(a: &VirtualNode, b: &VirtualNode) -> bool {
    a.attributes.len() == b.attributes.len()
        && a.attributes.iter().all(|(key, value)| {
            b.attributes
                .get(key)
                .is_some_and(|other| value.text() == other.text())
        })
}

impl VirtualTree {
    /// The node `link` names from `id`. With `elements_only` the nearest element in that direction:
    /// the parent when it is an element, the first or last element child, the nearest element
    /// sibling. `None` when there is none and for an unknown id.
    #[must_use]
    pub fn linked(&self, id: NodeId, link: Link, elements_only: bool) -> Option<NodeId> {
        if !elements_only {
            return match link {
                Link::Parent => self.get(id)?.parent(),
                Link::First => self.first_child(id),
                Link::Last => self.last_child(id),
                Link::Previous => self.previous_sibling(id),
                Link::Next => self.next_sibling(id),
            };
        }
        let node = self.get(id)?;
        let is_element = |other: &NodeId| self.get(*other).is_some_and(VirtualNode::is_element);
        let beside = |before: bool| {
            let siblings = self.get(node.parent()?)?.children();
            let index = siblings.iter().position(|&child| child == id)?;
            if before {
                siblings[..index].iter().rev().copied().find(is_element)
            } else {
                siblings[index + 1..].iter().copied().find(is_element)
            }
        };
        match link {
            Link::Parent => node.parent().filter(is_element),
            Link::First => node.children().iter().copied().find(is_element),
            Link::Last => node.children().iter().rev().copied().find(is_element),
            Link::Previous => beside(true),
            Link::Next => beside(false),
        }
    }

    /// The children of `id` in order, only the elements with `elements_only`. Empty for an
    /// unknown id.
    #[must_use]
    pub fn child_ids(&self, id: NodeId, elements_only: bool) -> Vec<NodeId> {
        let Some(node) = self.get(id) else {
            return Vec::new();
        };
        node.children()
            .iter()
            .copied()
            .filter(|&child| !elements_only || self.get(child).is_some_and(VirtualNode::is_element))
            .collect()
    }

    /// `id` and every node below it in pre-order. The content of a template and its nodes follow
    /// the template, before its children. Empty for an unknown id.
    #[must_use]
    pub fn subtree(&self, id: NodeId) -> Vec<NodeId> {
        let mut ids = Vec::new();
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            let Some(node) = self.get(current) else {
                continue;
            };
            ids.push(current);
            pending.extend(node.children().iter().rev());
            pending.extend(node.content);
        }
        ids
    }

    /// The top of the parent chain of `id`. A connected node gives the document, or the root
    /// while no document is declared.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn root_of(&self, id: NodeId) -> Result<NodeId, TreeError> {
        self.get(id).ok_or(missing(id))?;
        self.ancestors(id).last().ok_or(missing(id))
    }

    /// Whether `id` is connected, or the top of its parent chain is a document node: the
    /// declared document or a copy of it. `is_connected` stays the measure of focus and hover.
    #[must_use]
    pub fn in_document(&self, id: NodeId) -> bool {
        self.is_connected(id)
            || self
                .root_of(id)
                .ok()
                .and_then(|top| self.get(top))
                .is_some_and(VirtualNode::is_document)
    }

    /// The position of `other` relative to `this` as a bit mask: `1` disconnected, `2` preceding,
    /// `4` following, `8` contains, `16` contained by and `32` implementation specific. Equal ids
    /// give `0`. Nodes of different trees give `1 | 32` plus `2` when the top of `other` has the
    /// smaller id and `4` otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `this` or `other` names no node.
    pub fn compare_position(&self, this: NodeId, other: NodeId) -> Result<u32, TreeError> {
        self.get(this).ok_or(missing(this))?;
        self.get(other).ok_or(missing(other))?;
        if this == other {
            return Ok(0);
        }
        let chain = |id| {
            let mut chain: Vec<NodeId> = self.ancestors(id).collect();
            chain.reverse();
            chain
        };
        let (mine, theirs) = (chain(this), chain(other));
        if mine[0] != theirs[0] {
            let direction = if theirs[0] < mine[0] { 2 } else { 4 };
            return Ok(1 | 32 | direction);
        }
        let shared = mine.iter().zip(&theirs).take_while(|(a, b)| a == b).count();
        if shared == theirs.len() {
            return Ok(8 | 2);
        }
        if shared == mine.len() {
            return Ok(16 | 4);
        }
        let siblings = &self.nodes[&mine[shared - 1]].children;
        let index = |id| siblings.iter().position(|&child| child == id);
        Ok(if index(theirs[shared]) < index(mine[shared]) {
            2
        } else {
            4
        })
    }

    /// Whether the subtrees of `a` and `b` are equal: the same kinds, tags, data, namespaces,
    /// prefixes, attributes (compared by text form, the derived `style` text included) and children,
    /// pairwise in order.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `a` or `b` names no node.
    pub fn is_equal(&self, a: NodeId, b: NodeId) -> Result<bool, TreeError> {
        let mut pending = vec![(a, b)];
        while let Some((x, y)) = pending.pop() {
            let nx = self.get(x).ok_or(missing(x))?;
            let ny = self.get(y).ok_or(missing(y))?;
            if nx.kind != ny.kind
                || nx.local_name != ny.local_name
                || nx.data != ny.data
                || nx.namespace != ny.namespace
                || nx.prefix != ny.prefix
                || !same_attributes(nx, ny)
                || nx.children.len() != ny.children.len()
            {
                return Ok(false);
            }
            pending.extend(nx.children.iter().copied().zip(ny.children.iter().copied()));
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{Fixture, HTML, SVG, family, node, sample};
    use crate::tree::{COMMENT_TAG, FRAGMENT_TAG, TEXT_TAG};

    /// `r > (a > (a1 > x, a2), b > b1)`, returned as `[r, a, a1, a2, b, b1, x]`.
    fn shape() -> (VirtualTree, [NodeId; 7]) {
        let mut tree = VirtualTree::new();
        let ids = ["div", "div", "div", "div", "div", "div", "div"].map(|tag| node(&mut tree, tag));
        let [r, a, a1, a2, b, b1, x] = ids;
        for (parent, child) in [(r, a), (a, a1), (a, a2), (r, b), (b, b1), (a1, x)] {
            tree.append_child(parent, child).unwrap();
        }
        (tree, ids)
    }

    /// `root > parent > [#text, #comment, a, #text, #comment, b, #comment, #text]` under a
    /// `#document`, a `#fragment` holding a `div`, a node with no parent and an id no node has.
    fn links() -> (VirtualTree, [NodeId; 14]) {
        let (f, document) = Fixture::with_document();
        let (mut tree, root) = (f.tree, f.root);
        let [
            parent,
            text_1,
            comment_1,
            a,
            text_2,
            comment_2,
            b,
            comment_3,
            text_3,
        ] = [
            "div",
            TEXT_TAG,
            COMMENT_TAG,
            "div",
            TEXT_TAG,
            COMMENT_TAG,
            "div",
            COMMENT_TAG,
            TEXT_TAG,
        ]
        .map(|tag| node(&mut tree, tag));
        tree.append_child(root, parent).unwrap();
        for child in [
            text_1, comment_1, a, text_2, comment_2, b, comment_3, text_3,
        ] {
            tree.append_child(parent, child).unwrap();
        }
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let in_fragment = node(&mut tree, "div");
        tree.append_child(fragment, in_fragment).unwrap();
        let loose = node(&mut tree, "div");
        let ids = [
            parent,
            text_1,
            comment_1,
            a,
            text_2,
            comment_2,
            b,
            comment_3,
            text_3,
            root,
            document,
            fragment,
            in_fragment,
            loose,
        ];
        (tree, ids)
    }

    const PARENT: usize = 0;
    const TEXT_1: usize = 1;
    const COMMENT_1: usize = 2;
    const A: usize = 3;
    const TEXT_2: usize = 4;
    const B: usize = 6;
    const TEXT_3: usize = 8;
    const ROOT: usize = 9;
    const IN_FRAGMENT: usize = 12;
    const LOOSE: usize = 13;
    const UNKNOWN_ID: NodeId = 999;

    #[rstest]
    #[case::parent(A, Link::Parent, Some(PARENT))]
    #[case::first(PARENT, Link::First, Some(TEXT_1))]
    #[case::last(PARENT, Link::Last, Some(TEXT_3))]
    #[case::previous(A, Link::Previous, Some(COMMENT_1))]
    #[case::next(A, Link::Next, Some(TEXT_2))]
    #[case::parentless(LOOSE, Link::Parent, None)]
    fn linked_reads_the_node_the_link_names(
        #[case] node: usize,
        #[case] link: Link,
        #[case] want: Option<usize>,
    ) {
        let (tree, ids) = links();

        assert_eq!(tree.linked(ids[node], link, false), want.map(|at| ids[at]));
    }

    #[rstest]
    #[case::parent(Link::Parent, false)]
    #[case::of_elements(Link::Next, true)]
    fn linked_of_an_unknown_id_is_none(#[case] link: Link, #[case] elements_only: bool) {
        let (tree, _) = links();

        assert_eq!(tree.linked(UNKNOWN_ID, link, elements_only), None);
    }

    #[rstest]
    #[case::skips_a_text_and_a_comment(A, Some(B))]
    #[case::trailing_non_elements(B, None)]
    fn linked_next_of_elements_skips_other_nodes(#[case] node: usize, #[case] want: Option<usize>) {
        let (tree, ids) = links();

        assert_eq!(
            tree.linked(ids[node], Link::Next, true),
            want.map(|at| ids[at])
        );
    }

    #[rstest]
    #[case::skips_a_text_and_a_comment(B, Some(A))]
    #[case::leading_non_elements(A, None)]
    fn linked_previous_of_elements_skips_other_nodes(
        #[case] node: usize,
        #[case] want: Option<usize>,
    ) {
        let (tree, ids) = links();

        assert_eq!(
            tree.linked(ids[node], Link::Previous, true),
            want.map(|at| ids[at])
        );
    }

    #[rstest]
    #[case::first(Link::First, Some(A))]
    #[case::last(Link::Last, Some(B))]
    fn linked_end_of_elements_skips_leading_and_trailing_non_elements(
        #[case] link: Link,
        #[case] want: Option<usize>,
    ) {
        let (tree, ids) = links();

        assert_eq!(tree.linked(ids[PARENT], link, true), want.map(|at| ids[at]));
    }

    #[rstest]
    #[case::element_parent(A, Some(PARENT))]
    #[case::document_parent(ROOT, None)]
    #[case::fragment_parent(IN_FRAGMENT, None)]
    #[case::no_parent(LOOSE, None)]
    fn linked_parent_of_elements_is_an_element(#[case] node: usize, #[case] want: Option<usize>) {
        let (tree, ids) = links();

        assert_eq!(
            tree.linked(ids[node], Link::Parent, true),
            want.map(|at| ids[at])
        );
    }

    #[rstest]
    #[case::all(false, &[1, 2, 3, 4, 5, 6, 7, 8])]
    #[case::elements(true, &[3, 6])]
    fn child_ids_lists_the_children_in_order(#[case] elements_only: bool, #[case] want: &[usize]) {
        let (tree, ids) = links();

        let want: Vec<NodeId> = want.iter().map(|&at| ids[at]).collect();
        assert_eq!(tree.child_ids(ids[PARENT], elements_only), want);
    }

    #[test]
    fn subtree_lists_a_template_content_before_its_children_in_pre_order() {
        let mut tree = VirtualTree::new();
        let template = node(&mut tree, "template");
        let content = tree.template_content(template).unwrap();
        let [in_content, first, nested, second] = [(); 4].map(|()| node(&mut tree, "div"));
        tree.append_child(content, in_content).unwrap();
        tree.append_child(template, first).unwrap();
        tree.append_child(first, nested).unwrap();
        tree.append_child(template, second).unwrap();

        assert_eq!(
            tree.subtree(template),
            [template, content, in_content, first, nested, second]
        );
    }

    #[test]
    fn subtree_of_an_unknown_id_is_empty() {
        let (tree, _) = links();

        assert_eq!(tree.subtree(UNKNOWN_ID), Vec::<NodeId>::new());
    }

    #[test]
    fn child_ids_of_an_unknown_id_is_empty() {
        let (tree, _) = links();

        assert_eq!(tree.child_ids(UNKNOWN_ID, false), Vec::<NodeId>::new());
    }

    #[rstest]
    #[case::equal(1, 1, 0)]
    #[case::ancestor(2, 1, 10)]
    #[case::descendant(1, 2, 20)]
    #[case::previous_sibling(3, 2, 2)]
    #[case::next_sibling(2, 3, 4)]
    #[case::cousin_before(5, 2, 2)]
    #[case::cousin_after(2, 5, 4)]
    #[case::deep_cousin_after(6, 5, 4)]
    #[case::deep_cousin_before(5, 6, 2)]
    fn compare_position_reports_where_the_other_node_is(
        #[case] this: usize,
        #[case] other: usize,
        #[case] expected: u32,
    ) {
        let (tree, ids) = shape();

        assert_eq!(tree.compare_position(ids[this], ids[other]), Ok(expected));
    }

    #[rstest]
    #[case::loose_pair(0, 1, 37)]
    #[case::loose_pair_reversed(1, 0, 35)]
    #[case::child_of_the_other_tree(0, 2, 37)]
    #[case::child_of_the_other_tree_reversed(2, 0, 35)]
    fn compare_position_of_separate_trees_orders_them_by_the_id_of_their_tops(
        #[case] this: usize,
        #[case] other: usize,
        #[case] expected: u32,
    ) {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        let (nested, _) = family(&mut tree, 0);
        let leaf = node(&mut tree, "span");
        tree.append_child(second, nested).unwrap();
        tree.append_child(nested, leaf).unwrap();
        let ids = [first, second, leaf];

        assert_eq!(tree.compare_position(ids[this], ids[other]), Ok(expected));
    }

    #[rstest]
    #[case::this(1, 0)]
    #[case::other(0, 1)]
    fn compare_position_of_an_unknown_id_fails(#[case] this: usize, #[case] other: usize) {
        let mut tree = VirtualTree::new();
        let known = node(&mut tree, "div");
        let ids = [known, 99];

        assert_eq!(
            tree.compare_position(ids[this], ids[other]),
            Err(TreeError::NodeNotFound(99))
        );
    }

    #[test]
    fn root_of_a_connected_node_is_the_root() {
        let (mut tree, ids) = shape();
        tree.set_root(ids[0]);

        assert_eq!(tree.root_of(ids[6]), Ok(ids[0]));
    }

    #[test]
    fn root_of_a_detached_node_is_the_top_of_its_subtree() {
        let (tree, ids) = shape();

        assert_eq!(tree.root_of(ids[6]), Ok(ids[0]));
    }

    #[test]
    fn root_of_a_node_in_a_fragment_is_the_fragment() {
        let mut tree = VirtualTree::new();
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let child = node(&mut tree, "span");
        tree.append_child(fragment, child).unwrap();

        assert_eq!(tree.root_of(child), Ok(fragment));
    }

    #[test]
    fn root_of_an_unknown_id_fails() {
        assert_eq!(
            VirtualTree::new().root_of(3),
            Err(TreeError::NodeNotFound(3))
        );
    }

    #[rstest]
    #[case::copied_document(0, true)]
    #[case::element_of_a_copied_document(1, true)]
    #[case::parentless_element(2, false)]
    fn in_document_is_true_under_a_copied_document(#[case] at: usize, #[case] expected: bool) {
        let (mut f, document) = Fixture::with_document();
        let copies = f.tree.clone_subtree(document, true).unwrap();
        let loose = node(&mut f.tree, "div");
        let ids = [copies[0].1, copies[1].1, loose];

        assert_eq!(f.tree.in_document(ids[at]), expected);
    }

    #[rstest]
    #[case::same("div", |_: &mut VirtualTree, _: NodeId| {}, true)]
    #[case::number_equals_its_text("div", |t: &mut VirtualTree, r: NodeId| t.set_attribute(r, "tabindex", "1").unwrap(), true)]
    #[case::tag("p", |_: &mut VirtualTree, _: NodeId| {}, false)]
    #[case::attribute_value("div", |t: &mut VirtualTree, r: NodeId| t.set_attribute(r, "class", "b").unwrap(), false)]
    #[case::extra_attribute("div", |t: &mut VirtualTree, r: NodeId| t.set_attribute(r, "id", "x").unwrap(), false)]
    #[case::style_prop("div", |t: &mut VirtualTree, r: NodeId| t.set_style(r, "width", 5.0).unwrap(), false)]
    #[case::child_count(
        "div",
        |t: &mut VirtualTree, r: NodeId| {
            let extra = node(t, "span");
            t.append_child(r, extra).unwrap();
        },
        false
    )]
    #[case::deep_child(
        "div",
        |t: &mut VirtualTree, r: NodeId| {
            let inner = t.first_child(t.first_child(r).unwrap()).unwrap();
            t.set_data(inner, "yo").unwrap();
        },
        false
    )]
    #[case::text_data(
        "div",
        |t: &mut VirtualTree, r: NodeId| {
            let tail = t.last_child(r).unwrap();
            t.set_data(tail, "u").unwrap();
        },
        false
    )]
    fn is_equal_compares_tags_attributes_styles_and_children(
        #[case] second_tag: &str,
        #[case] change: fn(&mut VirtualTree, NodeId),
        #[case] expected: bool,
    ) {
        let mut tree = VirtualTree::new();
        let first = sample(&mut tree, "div");
        let second = sample(&mut tree, second_tag);
        change(&mut tree, second);

        assert_eq!(tree.is_equal(first, second), Ok(expected));
    }

    #[rstest]
    #[case::equal_comments("#comment", "a", "#comment", "a", true)]
    #[case::different_comments("#comment", "a", "#comment", "b", false)]
    #[case::equal_texts("#text", "a", "#text", "a", true)]
    #[case::different_texts("#text", "a", "#text", "b", false)]
    #[case::a_text_and_a_comment("#text", "a", "#comment", "a", false)]
    fn is_equal_tells_texts_and_comments_apart_by_data(
        #[case] first_tag: &str,
        #[case] first_data: &str,
        #[case] second_tag: &str,
        #[case] second_data: &str,
        #[case] expected: bool,
    ) {
        let mut tree = VirtualTree::new();
        let first = node(&mut tree, first_tag);
        tree.set_data(first, first_data).unwrap();
        let second = node(&mut tree, second_tag);
        tree.set_data(second, second_data).unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(expected));
    }

    #[rstest]
    #[case::html_by_name(Some(HTML), None, true)]
    #[case::namespace(Some(SVG), None, false)]
    #[case::prefix(Some(HTML), Some("x"), false)]
    fn is_equal_compares_the_namespace_and_the_prefix(
        #[case] namespace: Option<&str>,
        #[case] prefix: Option<&str>,
        #[case] expected: bool,
    ) {
        let mut tree = VirtualTree::new();
        let plain = node(&mut tree, "a");
        let other = tree.create_element_ns(namespace, prefix, "a").unwrap();

        assert_eq!(tree.is_equal(plain, other), Ok(expected));
    }

    #[test]
    fn is_equal_of_an_unknown_id_fails() {
        let mut tree = VirtualTree::new();
        let known = node(&mut tree, "div");

        assert_eq!(tree.is_equal(known, 9), Err(TreeError::NodeNotFound(9)));
    }

    #[test]
    fn is_equal_ignores_the_insertion_order_of_attributes() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        for key in ["a", "b", "c", "d"] {
            tree.set_attribute(first, key, key).unwrap();
        }
        for key in ["d", "c", "b", "a"] {
            tree.set_attribute(second, key, key).unwrap();
        }

        assert_eq!(tree.is_equal(first, second), Ok(true));
    }

    #[test]
    fn is_equal_compares_the_derived_style_text() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        tree.set_style(first, "text_color", "#f00").unwrap();
        tree.set_style(second, "text_color", "rgb(255, 0, 0)")
            .unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(true));
    }

    #[test]
    fn is_equal_tells_another_declaration_order_apart() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        tree.set_style_text(first, "width: 1px; height: 2px")
            .unwrap();
        tree.set_style_text(second, "height: 2px; width: 1px")
            .unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(false));
    }

    #[test]
    fn is_equal_tells_an_empty_style_attribute_from_none() {
        let mut tree = VirtualTree::new();
        let (first, second) = (node(&mut tree, "div"), node(&mut tree, "div"));
        tree.set_style_text(first, "").unwrap();

        assert_eq!(tree.is_equal(first, second), Ok(false));
    }
}
