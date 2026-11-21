// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The kind of a node, and the name, namespace and prefix of an element. The tests of these sit in
//! the same file.

use std::borrow::Cow;

use html5ever::ns;

use super::{COMMENT_TAG, DOCUMENT_TAG, FRAGMENT_TAG, TEXT_TAG, VirtualNode};

/// The kind of a node, set when the node is allocated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// An element, whose tag does not start with `#`.
    Element,
    /// A text node, tagged `#text`.
    Text,
    /// A comment, tagged `#comment`.
    Comment,
    /// A document, tagged `#document`.
    Document,
    /// A fragment, tagged `#fragment`.
    Fragment,
}

impl NodeKind {
    /// The kind a node with the tag `tag` has. `None` for a tag that starts with `#` and names no
    /// kind.
    #[must_use]
    pub fn of_tag(tag: &str) -> Option<Self> {
        match tag {
            TEXT_TAG => Some(Self::Text),
            COMMENT_TAG => Some(Self::Comment),
            DOCUMENT_TAG => Some(Self::Document),
            FRAGMENT_TAG => Some(Self::Fragment),
            _ if tag.starts_with('#') => None,
            _ => Some(Self::Element),
        }
    }
}

impl VirtualNode {
    /// The kind of the node.
    #[must_use]
    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    /// Whether the node is an element.
    #[must_use]
    pub fn is_element(&self) -> bool {
        self.kind == NodeKind::Element
    }

    /// Whether the node is a text node or a comment.
    #[must_use]
    pub fn is_character_data(&self) -> bool {
        matches!(self.kind, NodeKind::Text | NodeKind::Comment)
    }

    /// Whether the node is a document: the declared one or a copy of it.
    #[must_use]
    pub fn is_document(&self) -> bool {
        self.kind == NodeKind::Document
    }

    /// Whether the node is an HTML `template` element.
    #[must_use]
    pub fn is_template(&self) -> bool {
        self.is_html("template")
    }

    /// Whether the node is a `style` element of the HTML or the SVG namespace.
    #[must_use]
    pub fn is_style(&self) -> bool {
        self.is_html("style") || self.is_svg("style")
    }

    /// Whether the node is an HTML `button` or `input`.
    #[must_use]
    pub fn is_form_control(&self) -> bool {
        self.is_html("button") || self.is_html("input")
    }

    /// Whether the node is a link: an HTML `a` with `href`, or an SVG `a` with `href` or
    /// `xlink:href`.
    #[must_use]
    pub fn is_link(&self) -> bool {
        let has = |name| self.attributes().contains_key(name);
        (self.is_html("a") && has("href"))
            || (self.is_svg("a") && (has("href") || has("xlink:href")))
    }

    /// Whether the node is an HTML `button` or `input` or a link, which takes focus without a
    /// `tabindex`.
    #[must_use]
    pub fn is_focusable_by_default(&self) -> bool {
        self.is_form_control() || self.is_link()
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
        self.in_html_namespace() && self.local_name == local_name
    }

    /// Whether the node is an element of the SVG namespace named `local_name`.
    #[must_use]
    pub fn is_svg(&self, local_name: &str) -> bool {
        self.namespace == Some(ns!(svg)) && self.local_name == local_name
    }

    /// Whether the node is an element of the `MathML` namespace named `local_name`.
    #[must_use]
    pub fn is_mathml(&self, local_name: &str) -> bool {
        self.namespace == Some(ns!(mathml)) && self.local_name == local_name
    }

    /// The name with its prefix: `prefix:local`, or the local name alone.
    #[must_use]
    pub fn qualified_name(&self) -> Cow<'_, str> {
        match &self.prefix {
            Some(prefix) => Cow::Owned(format!("{prefix}:{}", self.local_name)),
            None => Cow::Borrowed(&self.local_name),
        }
    }

    /// The qualified name, in ASCII upper case for an element of the HTML namespace.
    #[must_use]
    pub fn tag_name(&self) -> Cow<'_, str> {
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

    use super::NodeKind;
    use crate::tests::{HTML, MATHML, SVG, node};
    use crate::tree::VirtualTree;
    use rstest::rstest;

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
        assert_eq!(created.tag_name(), dom);
    }

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
    #[case::element("div", Some(NodeKind::Element))]
    #[case::text("#text", Some(NodeKind::Text))]
    #[case::comment("#comment", Some(NodeKind::Comment))]
    #[case::document("#document", Some(NodeKind::Document))]
    #[case::fragment("#fragment", Some(NodeKind::Fragment))]
    #[case::unknown("#other", None)]
    #[case::bare_hash("#", None)]
    fn a_tag_maps_to_its_kind(#[case] tag: &str, #[case] expected: Option<NodeKind>) {
        assert_eq!(NodeKind::of_tag(tag), expected);
    }

    #[rstest]
    #[case::element("div", NodeKind::Element)]
    #[case::text("#text", NodeKind::Text)]
    #[case::comment("#comment", NodeKind::Comment)]
    #[case::document("#document", NodeKind::Document)]
    #[case::fragment("#fragment", NodeKind::Fragment)]
    fn create_node_gives_the_node_the_kind_of_its_tag(
        #[case] tag: &str,
        #[case] expected: NodeKind,
    ) {
        let mut tree = VirtualTree::new();
        let id = node(&mut tree, tag);

        assert_eq!(tree.get(id).unwrap().kind(), expected);
    }

    #[rstest]
    #[case::element("div", false)]
    #[case::text("#text", true)]
    #[case::comment("#comment", true)]
    #[case::document("#document", false)]
    #[case::fragment("#fragment", false)]
    fn is_character_data_is_true_for_text_and_comment(#[case] tag: &str, #[case] expected: bool) {
        let mut tree = VirtualTree::new();
        let id = node(&mut tree, tag);

        assert_eq!(tree.get(id).unwrap().is_character_data(), expected);
    }

    #[rstest]
    #[case::document("#document", true)]
    #[case::fragment("#fragment", false)]
    #[case::element("div", false)]
    fn is_document_is_true_for_a_document_tag_only(#[case] tag: &str, #[case] expected: bool) {
        let mut tree = VirtualTree::new();
        let id = node(&mut tree, tag);

        assert_eq!(tree.get(id).unwrap().is_document(), expected);
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
    #[case::html(Some(HTML), "style", true)]
    #[case::html_other_case(Some(HTML), "STYLE", false)]
    #[case::svg(Some(SVG), "style", true)]
    #[case::svg_other_case(Some(SVG), "STYLE", false)]
    #[case::other_namespace(Some("urn:x"), "style", false)]
    #[case::no_namespace(None, "style", false)]
    #[case::html_other_name(Some(HTML), "div", false)]
    fn is_style_needs_the_exact_name_in_the_html_or_svg_namespace(
        #[case] namespace: Option<&str>,
        #[case] local_name: &str,
        #[case] expected: bool,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_element_ns(namespace, None, local_name).unwrap();

        assert_eq!(tree.get(id).unwrap().is_style(), expected);
    }

    #[rstest]
    #[case::html_a_href(Some(HTML), "a", &["href"], true, true, false)]
    #[case::html_a_bare(Some(HTML), "a", &[], false, false, false)]
    #[case::html_a_xlink_href(Some(HTML), "a", &["xlink:href"], false, false, false)]
    #[case::svg_a_href(Some(SVG), "a", &["href"], true, true, false)]
    #[case::svg_a_xlink_href(Some(SVG), "a", &["xlink:href"], true, true, false)]
    #[case::svg_a_bare(Some(SVG), "a", &[], false, false, false)]
    #[case::mathml_a_href(Some(MATHML), "a", &["href"], false, false, false)]
    #[case::html_button(Some(HTML), "button", &[], false, true, true)]
    #[case::html_input(Some(HTML), "input", &[], false, true, true)]
    #[case::html_div(Some(HTML), "div", &[], false, false, false)]
    #[case::svg_button(Some(SVG), "button", &[], false, false, false)]
    #[case::svg_input(Some(SVG), "input", &[], false, false, false)]
    fn link_form_control_and_default_focus_follow_the_kind_and_href(
        #[case] namespace: Option<&str>,
        #[case] local_name: &str,
        #[case] attributes: &[&str],
        #[case] link: bool,
        #[case] focusable: bool,
        #[case] form_control: bool,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_element_ns(namespace, None, local_name).unwrap();
        for attribute in attributes {
            tree.set_attribute(id, *attribute, "u").unwrap();
        }
        let node = tree.get(id).unwrap();

        assert_eq!(
            (
                node.is_link(),
                node.is_focusable_by_default(),
                node.is_form_control()
            ),
            (link, focusable, form_control)
        );
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
