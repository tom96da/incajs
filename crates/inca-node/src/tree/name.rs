// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The name, namespace and prefix of a node, and the kinds of element they identify.

use std::borrow::Cow;

use html5ever::ns;

use super::{VirtualNode, is_template_tag};

impl VirtualNode {
    /// Whether a node with the tag `tag` is an element. The other kinds have tags that start
    /// with `#`.
    #[must_use]
    pub(crate) fn is_element_tag(tag: &str) -> bool {
        !tag.starts_with('#')
    }

    /// Whether the node is an element.
    #[must_use]
    pub fn is_element(&self) -> bool {
        Self::is_element_tag(&self.tag_name)
    }

    /// Whether the tag of the node is `template`.
    #[must_use]
    pub fn is_template(&self) -> bool {
        is_template_tag(&self.tag_name)
    }

    /// The namespace of the element, `None` for a node that has none.
    #[must_use]
    pub fn namespace_uri(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// The prefix of the element name, `None` for a name without one.
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }

    /// Whether the node is an element of the HTML namespace.
    #[must_use]
    pub fn in_html_namespace(&self) -> bool {
        self.namespace == Some(ns!(html))
    }

    /// Whether the node is an element of the HTML namespace named `local_name`.
    #[must_use]
    pub fn is_html(&self, local_name: &str) -> bool {
        self.in_html_namespace() && self.tag_name == local_name
    }

    /// The name with its prefix: `prefix:local`, or the local name alone.
    #[must_use]
    pub fn qualified_name(&self) -> Cow<'_, str> {
        match &self.prefix {
            Some(prefix) => Cow::Owned(format!("{prefix}:{}", self.tag_name)),
            None => Cow::Borrowed(&self.tag_name),
        }
    }

    /// The qualified name, in ASCII upper case for an element of the HTML namespace.
    #[must_use]
    pub fn dom_tag_name(&self) -> Cow<'_, str> {
        let name = self.qualified_name();
        if self.in_html_namespace() {
            Cow::Owned(name.to_ascii_uppercase())
        } else {
            name
        }
    }

    /// The key an attribute called `name` has on this node: ASCII lower case for an element of
    /// the HTML namespace, `name` as given for any other node.
    #[must_use]
    pub fn attribute_key<'a>(&self, name: &'a str) -> Cow<'a, str> {
        if self.in_html_namespace() && name.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(name.to_ascii_lowercase())
        } else {
            Cow::Borrowed(name)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::{HTML, SVG, node};
    use crate::tree::VirtualTree;
    use rstest::rstest;

    #[rstest]
    #[case::html(Some(HTML), true)]
    #[case::svg(Some(SVG), false)]
    #[case::no_namespace(None, false)]
    fn in_html_namespace_is_true_for_the_html_namespace_only(
        #[case] namespace: Option<&str>,
        #[case] expected: bool,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_element_ns(namespace, None, "a").unwrap();

        assert_eq!(tree.get(id).unwrap().in_html_namespace(), expected);
    }

    #[rstest]
    #[case::element("div", true)]
    #[case::text("#text", false)]
    #[case::comment("#comment", false)]
    #[case::document("#document", false)]
    #[case::fragment("#fragment", false)]
    fn is_element_is_true_for_an_element_only(#[case] tag: &str, #[case] expected: bool) {
        let mut tree = VirtualTree::new();
        let id = node(&mut tree, tag);

        assert_eq!(tree.get(id).unwrap().is_element(), expected);
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

    #[rstest]
    #[case::html(Some(HTML), None, "div", "div", "DIV")]
    #[case::svg(Some(SVG), None, "foreignObject", "foreignObject", "foreignObject")]
    #[case::no_namespace(None, None, "a", "a", "a")]
    #[case::prefixed_svg(Some(SVG), Some("s"), "g", "s:g", "s:g")]
    #[case::prefixed_html(Some(HTML), Some("x"), "Y", "x:Y", "X:Y")]
    fn names_of_an_element_follow_its_namespace_and_prefix(
        #[case] namespace: Option<&str>,
        #[case] prefix: Option<&str>,
        #[case] local_name: &str,
        #[case] qualified: &str,
        #[case] dom: &str,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree
            .create_element_ns(namespace, prefix, local_name)
            .unwrap();

        let created = tree.get(id).unwrap();
        assert_eq!(created.qualified_name(), qualified);
        assert_eq!(created.dom_tag_name(), dom);
    }

    #[rstest]
    #[case::html_exact(Some(HTML), "div", "div", true)]
    #[case::html_other_name(Some(HTML), "div", "span", false)]
    #[case::html_other_case(Some(HTML), "DIV", "div", false)]
    #[case::svg(Some(SVG), "input", "input", false)]
    #[case::no_namespace(None, "input", "input", false)]
    fn is_html_needs_the_html_namespace_and_the_exact_name(
        #[case] namespace: Option<&str>,
        #[case] local_name: &str,
        #[case] asked: &str,
        #[case] expected: bool,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_element_ns(namespace, None, local_name).unwrap();

        assert_eq!(tree.get(id).unwrap().is_html(asked), expected);
    }

    #[rstest]
    #[case::html(Some(HTML), "viewBox", "viewbox")]
    #[case::svg(Some(SVG), "viewBox", "viewBox")]
    fn attribute_key_lowers_the_name_for_an_html_element_only(
        #[case] namespace: Option<&str>,
        #[case] name: &str,
        #[case] expected: &str,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_element_ns(namespace, None, "e").unwrap();

        assert_eq!(tree.get(id).unwrap().attribute_key(name), expected);
    }
}
