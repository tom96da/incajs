// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Creating nodes and elements, and the ids they take. The tests of these sit in the same file.

use html5ever::{Namespace, ns};

use super::{NodeId, TreeError, VirtualNode, VirtualTree};

impl VirtualTree {
    /// Allocates a new node and returns its id. Ids are assigned
    /// sequentially starting at 0 and are never reused, even after the node
    /// they named is destroyed, so a stale id never names another node.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::IdsExhausted`] once `u32::MAX` nodes have been
    /// created over the tree's lifetime. A template takes two ids, its own and its content's,
    /// and fails without creating either when fewer are left.
    pub fn create_node(&mut self, tag_name: impl Into<String>) -> Result<NodeId, TreeError> {
        let tag_name = tag_name.into();
        let namespace = VirtualNode::is_element_tag(&tag_name).then_some(ns!(html));
        self.create_element(namespace, None, tag_name)
    }

    /// Makes the next id `next`, so a test reaches the end of the id space.
    #[cfg(test)]
    pub(crate) fn set_next_id(&mut self, next: NodeId) {
        self.next_id = next;
    }

    pub(super) fn allocate(
        &mut self,
        tag_name: String,
        namespace: Option<Namespace>,
        prefix: Option<String>,
    ) -> Result<NodeId, TreeError> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(TreeError::IdsExhausted)?;
        self.nodes
            .insert(id, VirtualNode::new(id, tag_name, namespace, prefix));
        self.orphans.insert(id);
        Ok(id)
    }

    /// Allocates an element of `namespace` named `local_name` under `prefix`. An empty or absent
    /// namespace leaves the element without one. Only an element of the HTML namespace named
    /// `template` owns a content fragment.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::IdsExhausted`] as [`create_node`](Self::create_node) does.
    pub fn create_element_ns(
        &mut self,
        namespace: Option<&str>,
        prefix: Option<&str>,
        local_name: impl Into<String>,
    ) -> Result<NodeId, TreeError> {
        let namespace = namespace
            .filter(|text| !text.is_empty())
            .map(Namespace::from);
        self.create_element(namespace, prefix.map(str::to_owned), local_name.into())
    }

    pub(super) fn create_element(
        &mut self,
        namespace: Option<Namespace>,
        prefix: Option<String>,
        tag_name: String,
    ) -> Result<NodeId, TreeError> {
        if namespace == Some(ns!(html)) && tag_name.eq_ignore_ascii_case("template") {
            return self.create_template(tag_name, prefix);
        }
        self.allocate(tag_name, namespace, prefix)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::{HTML, SVG, node};
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
    fn a_fresh_node_has_no_parent() {
        let mut tree = VirtualTree::new();
        let node = tree.create_node("div").unwrap();

        assert_eq!(tree.get(node).unwrap().parent(), None);
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

    #[rstest]
    #[case::element("div", Some(HTML))]
    #[case::text("#text", None)]
    fn create_node_sets_the_namespace_of_an_element_only(
        #[case] tag: &str,
        #[case] namespace: Option<&str>,
    ) {
        let mut tree = VirtualTree::new();
        let id = node(&mut tree, tag);

        let created = tree.get(id).unwrap();
        assert_eq!(created.namespace_uri(), namespace);
        assert_eq!(created.prefix(), None);
    }

    #[test]
    fn create_element_ns_stores_the_namespace() {
        let mut tree = VirtualTree::new();

        let id = tree.create_element_ns(Some(SVG), None, "svg").unwrap();

        let created = tree.get(id).unwrap();
        assert_eq!(created.namespace_uri(), Some(SVG));
    }

    #[rstest]
    #[case::none(None)]
    #[case::empty(Some(""))]
    fn create_element_ns_without_a_namespace_has_none(#[case] namespace: Option<&str>) {
        let mut tree = VirtualTree::new();

        let id = tree.create_element_ns(namespace, None, "a").unwrap();

        assert_eq!(tree.get(id).unwrap().namespace_uri(), None);
    }
}
