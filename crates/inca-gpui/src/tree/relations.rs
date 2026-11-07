// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cloning nodes and comparing them: the top of a tree, tree positions and equality.

use super::{NodeId, TreeError, VirtualNode, VirtualTree};
use crate::selector::attribute_text;

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
                .is_some_and(|other| attribute_text(value) == attribute_text(other))
        })
}

impl VirtualTree {
    /// The top of the parent chain of `id`. A connected node gives the root.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn root_of(&self, id: NodeId) -> Result<NodeId, TreeError> {
        self.get(id).ok_or(missing(id))?;
        self.ancestors(id).last().ok_or(missing(id))
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

    /// Whether the subtrees of `a` and `b` are equal: the same tags, attributes (compared by text
    /// form), style props and children, pairwise in order.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `a` or `b` names no node.
    pub fn is_equal(&self, a: NodeId, b: NodeId) -> Result<bool, TreeError> {
        let mut pending = vec![(a, b)];
        while let Some((x, y)) = pending.pop() {
            let nx = self.get(x).ok_or(missing(x))?;
            let ny = self.get(y).ok_or(missing(y))?;
            if nx.tag_name != ny.tag_name
                || nx.style_props != ny.style_props
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
    use crate::tests::{family, node, sample};
    use crate::tree::FRAGMENT_TAG;

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
            t.set_attribute(inner, "value", "yo").unwrap();
        },
        false
    )]
    #[case::text_data(
        "div",
        |t: &mut VirtualTree, r: NodeId| {
            let tail = t.last_child(r).unwrap();
            t.set_attribute(tail, "value", "u").unwrap();
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
}
