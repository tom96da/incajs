// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reads behind `HTMLCollection` and `NodeList`: one child at a time, the count and the
//! supported names.

use super::{NodeId, VirtualNode, VirtualTree};

/// The value of the attribute `key` of `node` as text, `None` when it has none.
fn attribute_text(node: &VirtualNode, key: &str) -> Option<String> {
    node.attributes().get(key).map(super::AttributeValue::text)
}

impl VirtualTree {
    /// The children of `id`, only the elements with `elements_only`. Empty for an unknown id.
    fn children_of(&self, id: NodeId, elements_only: bool) -> impl Iterator<Item = &VirtualNode> {
        self.get(id)
            .into_iter()
            .flat_map(VirtualNode::children)
            .filter_map(|&child| self.get(child))
            .filter(move |child| !elements_only || child.is_element())
    }

    /// The number of children of `id`, only the elements with `elements_only`. `0` for an unknown
    /// id.
    #[must_use]
    pub fn child_count(&self, id: NodeId, elements_only: bool) -> usize {
        if elements_only {
            self.children_of(id, true).count()
        } else {
            self.get(id).map_or(0, |node| node.children().len())
        }
    }

    /// The child of `id` at `index`, counting only the elements with `elements_only`. `None` past
    /// the last child and for an unknown id.
    #[must_use]
    pub fn child_at(&self, id: NodeId, index: usize, elements_only: bool) -> Option<NodeId> {
        if elements_only {
            self.children_of(id, true).nth(index).map(VirtualNode::id)
        } else {
            self.get(id)?.children().get(index).copied()
        }
    }

    /// The first element child of `id` in tree order that has the id `name`, or that is in the
    /// HTML namespace and has a `name` attribute of `name`. `None` for `""`, for no match and for
    /// an unknown id.
    #[must_use]
    pub fn named_child(&self, id: NodeId, name: &str) -> Option<NodeId> {
        if name.is_empty() {
            return None;
        }
        self.children_of(id, true)
            .find(|child| {
                attribute_text(child, "id").as_deref() == Some(name)
                    || (child.in_html_namespace()
                        && attribute_text(child, "name").as_deref() == Some(name))
            })
            .map(VirtualNode::id)
    }

    /// The supported property names of the element children of `id`: for each child in tree order
    /// its id, then its `name` attribute when it is in the HTML namespace. Each value comes once
    /// and `""` never. Empty for an unknown id.
    #[must_use]
    pub fn child_names(&self, id: NodeId) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for child in self.children_of(id, true) {
            let name = child
                .in_html_namespace()
                .then(|| attribute_text(child, "name"))
                .flatten();
            for value in [attribute_text(child, "id"), name].into_iter().flatten() {
                if !value.is_empty() && !names.contains(&value) {
                    names.push(value);
                }
            }
        }
        names
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use crate::tests::{SVG, node};
    use crate::tree::{NodeId, VirtualTree};

    /// A `div` parent with the children `span`, `#text`, `p`, `#comment`, `em`.
    fn mixed() -> (VirtualTree, NodeId, [NodeId; 5]) {
        let mut tree = VirtualTree::new();
        let parent = node(&mut tree, "div");
        let children = ["span", "#text", "p", "#comment", "em"].map(|tag| {
            let child = node(&mut tree, tag);
            tree.append_child(parent, child).unwrap();
            child
        });
        (tree, parent, children)
    }

    #[rstest]
    #[case::all(false, 5)]
    #[case::elements(true, 3)]
    fn child_count_counts_the_children(#[case] elements_only: bool, #[case] want: usize) {
        let (tree, parent, _) = mixed();

        assert_eq!(tree.child_count(parent, elements_only), want);
    }

    #[rstest]
    #[case::all_first(false, 1, Some(1))]
    #[case::all_last(false, 4, Some(4))]
    #[case::all_past_the_end(false, 5, None)]
    #[case::elements_second(true, 1, Some(2))]
    #[case::elements_last(true, 2, Some(4))]
    #[case::elements_past_the_end(true, 3, None)]
    fn child_at_reads_the_child_at_an_index(
        #[case] elements_only: bool,
        #[case] index: usize,
        #[case] want: Option<usize>,
    ) {
        let (tree, parent, children) = mixed();

        assert_eq!(
            tree.child_at(parent, index, elements_only),
            want.map(|at| children[at])
        );
    }

    #[rstest]
    #[case::all_nodes(false)]
    #[case::elements_only(true)]
    fn a_missing_node_has_no_children_to_count(#[case] elements_only: bool) {
        let (tree, _, _) = mixed();

        assert_eq!(tree.child_count(999, elements_only), 0);
    }

    #[rstest]
    #[case::all_nodes(false)]
    #[case::elements_only(true)]
    fn a_missing_node_has_no_child_at_an_index(#[case] elements_only: bool) {
        let (tree, _, _) = mixed();

        assert_eq!(tree.child_at(999, 0, elements_only), None);
    }

    #[test]
    fn a_missing_node_has_no_named_child() {
        let (tree, _, _) = mixed();

        assert_eq!(tree.named_child(999, "x"), None);
    }

    #[test]
    fn a_missing_node_has_no_child_names() {
        let (tree, _, _) = mixed();

        assert_eq!(tree.child_names(999), Vec::<String>::new());
    }

    /// A `div` parent whose children are made by `children`, each a tag, an optional namespace, an
    /// id and a `name`.
    fn named(children: &[(&str, Option<&str>, &str, &str)]) -> (VirtualTree, NodeId, Vec<NodeId>) {
        let mut tree = VirtualTree::new();
        let parent = node(&mut tree, "div");
        let ids = children
            .iter()
            .map(|&(tag, namespace, id, name)| {
                let child = tree.create_element_ns(namespace, None, tag).unwrap();
                tree.append_child(parent, child).unwrap();
                if !id.is_empty() {
                    tree.set_attribute(child, "id", id).unwrap();
                }
                if !name.is_empty() {
                    tree.set_attribute(child, "name", name).unwrap();
                }
                child
            })
            .collect();
        (tree, parent, ids)
    }

    const HTML: Option<&str> = Some(crate::tests::HTML);

    #[rstest]
    #[case::id(&[("a", HTML, "x", "")], "x", Some(0))]
    #[case::name(&[("a", HTML, "", "x")], "x", Some(0))]
    #[case::tree_order_over_id(&[("a", HTML, "", "x"), ("b", HTML, "x", "")], "x", Some(0))]
    #[case::first_of_two(&[("a", HTML, "x", ""), ("b", HTML, "x", "")], "x", Some(0))]
    #[case::name_of_a_foreign_element(&[("a", Some(SVG), "", "x")], "x", None)]
    #[case::id_of_a_foreign_element(&[("a", Some(SVG), "x", "")], "x", Some(0))]
    #[case::no_match(&[("a", HTML, "y", "z")], "x", None)]
    fn named_child_finds_the_first_match_in_tree_order(
        #[case] children: &[(&str, Option<&str>, &str, &str)],
        #[case] name: &str,
        #[case] want: Option<usize>,
    ) {
        let (tree, parent, ids) = named(children);

        assert_eq!(tree.named_child(parent, name), want.map(|at| ids[at]));
    }

    #[test]
    fn named_child_of_the_empty_string_is_none() {
        let (mut tree, parent, ids) = named(&[("a", HTML, "", "")]);
        tree.set_attribute(ids[0], "id", "").unwrap();

        assert_eq!(tree.named_child(parent, ""), None);
    }

    #[test]
    fn named_child_skips_a_text_child() {
        let (mut tree, parent, _) = named(&[]);
        let text = node(&mut tree, "#text");
        tree.append_child(parent, text).unwrap();
        tree.set_attribute(text, "id", "x").unwrap();

        assert_eq!(tree.named_child(parent, "x"), None);
    }

    #[rstest]
    #[case::id_then_name(&[("a", HTML, "x", "y")], &["x", "y"])]
    #[case::tree_order(&[("a", HTML, "", "n"), ("b", HTML, "i", "")], &["n", "i"])]
    #[case::each_value_once(&[("a", HTML, "x", "x"), ("b", HTML, "x", "")], &["x"])]
    #[case::never_empty(&[("a", HTML, "", "")], &[])]
    #[case::name_of_a_foreign_element(&[("a", Some(SVG), "x", "y")], &["x"])]
    fn child_names_lists_each_supported_name_once(
        #[case] children: &[(&str, Option<&str>, &str, &str)],
        #[case] want: &[&str],
    ) {
        let (tree, parent, _) = named(children);

        assert_eq!(tree.child_names(parent), want);
    }
}
