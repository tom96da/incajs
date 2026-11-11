// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writing a subtree of the tree as HTML.

use std::io;

use html5ever::serialize::SerializeOpts;
use html5ever::serialize::{Serialize, Serializer, TraversalScope, serialize};
use html5ever::{LocalName, QualName, ns};

use crate::tree::{
    COMMENT_TAG, FRAGMENT_TAG, NodeId, TEXT_TAG, TreeError, VirtualNode, VirtualTree,
};

/// What [`VirtualTree::serialize_html`] writes of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlScope {
    /// The children of the node.
    Inner,
    /// The node and its children.
    Outer,
}

/// The elements that have no content: they write no end tag and no children.
const VOID_TAGS: [&str; 18] = [
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

/// A subtree as a value the serializer writes.
struct Subtree<'a> {
    tree: &'a VirtualTree,
    id: NodeId,
}

enum Step {
    Enter(NodeId),
    Exit(NodeId),
}

fn html_name(tag: &str) -> QualName {
    QualName::new(None, ns!(html), LocalName::from(tag))
}

/// The attributes of `node` in insertion order, each written as its text.
fn attribute_list(node: &VirtualNode) -> Vec<(QualName, String)> {
    node.attributes()
        .iter()
        .map(|(key, value)| {
            (
                QualName::new(None, ns!(), LocalName::from(key.as_str())),
                value.text(),
            )
        })
        .collect()
}

impl Subtree<'_> {
    /// The children that stand for the children of `id`: the content of a template, or the own
    /// children.
    fn children(&self, id: NodeId) -> Vec<NodeId> {
        let source = self.tree.template_content(id).unwrap_or(id);
        self.tree
            .get(source)
            .map(|node| node.children().to_vec())
            .unwrap_or_default()
    }
}

impl Serialize for Subtree<'_> {
    fn serialize<S: Serializer>(
        &self,
        serializer: &mut S,
        scope: TraversalScope,
    ) -> io::Result<()> {
        let mut steps: Vec<Step> = match scope {
            TraversalScope::IncludeNode => vec![Step::Enter(self.id)],
            TraversalScope::ChildrenOnly(_) => self
                .children(self.id)
                .into_iter()
                .rev()
                .map(Step::Enter)
                .collect(),
        };
        while let Some(step) = steps.pop() {
            match step {
                Step::Exit(id) => {
                    if let Some(node) = self.tree.get(id) {
                        serializer.end_elem(html_name(node.tag_name()))?;
                    }
                }
                Step::Enter(id) => {
                    let Some(node) = self.tree.get(id) else {
                        continue;
                    };
                    match node.tag_name() {
                        TEXT_TAG => serializer.write_text(node.data())?,
                        COMMENT_TAG => serializer.write_comment(node.data())?,
                        FRAGMENT_TAG => {
                            steps.extend(self.children(id).into_iter().rev().map(Step::Enter));
                        }
                        tag => {
                            let attributes = attribute_list(node);
                            serializer.start_elem(
                                html_name(tag),
                                attributes.iter().map(|(name, text)| (name, text.as_str())),
                            )?;
                            steps.push(Step::Exit(id));
                            if !VOID_TAGS.contains(&tag) {
                                steps.extend(self.children(id).into_iter().rev().map(Step::Enter));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl VirtualTree {
    /// The markup of the node `id`: its children with [`HtmlScope::Inner`], and the node itself
    /// with [`HtmlScope::Outer`]. A text node and a comment have no children, so their inner
    /// markup is empty. A fragment has no markup of its own. A template writes its content as its
    /// children. Void elements write no end tag and no children.
    ///
    /// The attributes are written in their order. The `style` attribute holds the inline style
    /// declarations as `name: value;` in the order they were set.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::NodeNotFound`] if `id` names no node.
    pub fn serialize_html(&self, id: NodeId, scope: HtmlScope) -> Result<String, TreeError> {
        let node = self.get(id).ok_or(TreeError::NodeNotFound(id))?;
        let tag = node.tag_name();
        let leaf = matches!(tag, TEXT_TAG | COMMENT_TAG);
        let traversal_scope = match scope {
            HtmlScope::Inner if leaf => return Ok(String::new()),
            HtmlScope::Inner => TraversalScope::ChildrenOnly(Some(html_name(tag))),
            HtmlScope::Outer => TraversalScope::IncludeNode,
        };
        let mut out = Vec::new();
        let options = SerializeOpts {
            traversal_scope,
            ..SerializeOpts::default()
        };
        // A `Vec` writer cannot fail, and the walk calls the serializer in matching pairs.
        serialize(&mut out, &Subtree { tree: self, id }, options).ok();
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::node;
    use crate::tree::AttributeValue;

    fn text_node(tree: &mut VirtualTree, data: &str) -> NodeId {
        let id = node(tree, TEXT_TAG);
        tree.set_attribute(id, "value", data).unwrap();
        id
    }

    /// An element of `tag` holding `data` as a text node.
    fn holding(tree: &mut VirtualTree, tag: &str, data: &str) -> NodeId {
        let parent = node(tree, tag);
        let child = text_node(tree, data);
        tree.append_child(parent, child).unwrap();
        parent
    }

    fn outer(tree: &VirtualTree, id: NodeId) -> String {
        tree.serialize_html(id, HtmlScope::Outer).unwrap()
    }

    fn inner(tree: &VirtualTree, id: NodeId) -> String {
        tree.serialize_html(id, HtmlScope::Inner).unwrap()
    }

    #[rstest]
    #[case::ampersand("a&b", "a&amp;b")]
    #[case::less_than("a<b", "a&lt;b")]
    #[case::greater_than("a>b", "a&gt;b")]
    #[case::no_break_space("a\u{a0}b", "a&nbsp;b")]
    #[case::quote_stays("a\"b", "a\"b")]
    fn text_is_escaped(#[case] data: &str, #[case] expected: &str) {
        let mut tree = VirtualTree::new();
        let div = holding(&mut tree, "div", data);

        assert_eq!(inner(&tree, div), expected);
    }

    #[rstest]
    #[case::ampersand("a&b", "a&amp;b")]
    #[case::quote("a\"b", "a&quot;b")]
    #[case::less_than("a<b", "a&lt;b")]
    #[case::greater_than("a>b", "a&gt;b")]
    #[case::no_break_space("a\u{a0}b", "a&nbsp;b")]
    fn an_attribute_value_is_escaped(#[case] value: &str, #[case] expected: &str) {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        tree.set_attribute(div, "title", value).unwrap();

        assert_eq!(
            outer(&tree, div),
            format!("<div title=\"{expected}\"></div>")
        );
    }

    #[rstest]
    #[case::style("style")]
    #[case::script("script")]
    fn text_under_a_raw_text_element_is_not_escaped(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let element = holding(&mut tree, tag, "a<b&c");

        assert_eq!(outer(&tree, element), format!("<{tag}>a<b&c</{tag}>"));
    }

    #[test]
    fn the_inner_markup_of_a_style_element_is_not_escaped() {
        let mut tree = VirtualTree::new();
        let style = holding(&mut tree, "style", "a<b&c");

        assert_eq!(inner(&tree, style), "a<b&c");
    }

    #[rstest]
    #[case::br("br")]
    #[case::img("img")]
    fn a_void_element_writes_none_of_its_children(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let element = holding(&mut tree, tag, "x");

        assert_eq!(outer(&tree, element), format!("<{tag}>"));
    }

    #[test]
    fn a_custom_element_writes_an_end_tag() {
        let mut tree = VirtualTree::new();
        let element = node(&mut tree, "my-el");

        assert_eq!(outer(&tree, element), "<my-el></my-el>");
    }

    #[test]
    fn attributes_follow_the_insertion_order() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        for key in ["a", "b", "c"] {
            tree.set_attribute(div, key, "1").unwrap();
        }

        assert_eq!(outer(&tree, div), "<div a=\"1\" b=\"1\" c=\"1\"></div>");
    }

    #[test]
    fn a_removed_and_added_attribute_goes_last() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        for key in ["a", "b"] {
            tree.set_attribute(div, key, "1").unwrap();
        }
        tree.remove_attribute(div, "a").unwrap();
        tree.set_attribute(div, "a", "1").unwrap();

        assert_eq!(outer(&tree, div), "<div b=\"1\" a=\"1\"></div>");
    }

    #[rstest]
    #[case::string("x", "x")]
    #[case::number(1e21, "1e+21")]
    #[case::bool_true(true, "true")]
    fn an_attribute_value_prints_as_text(
        #[case] value: impl Into<AttributeValue>,
        #[case] expected: &str,
    ) {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        tree.set_attribute(div, "k", value).unwrap();

        assert_eq!(outer(&tree, div), format!("<div k=\"{expected}\"></div>"));
    }

    fn sample(tree: &mut VirtualTree) -> NodeId {
        let div = node(tree, "div");
        let comment = node(tree, COMMENT_TAG);
        tree.set_attribute(comment, "value", "c").unwrap();
        let text = text_node(tree, "t");
        tree.append_child(div, text).unwrap();
        tree.append_child(div, comment).unwrap();
        div
    }

    #[rstest]
    #[case::inner(HtmlScope::Inner, "t<!--c-->")]
    #[case::outer(HtmlScope::Outer, "<div>t<!--c--></div>")]
    fn the_scope_of_an_element_picks_the_node_or_its_children(
        #[case] scope: HtmlScope,
        #[case] expected: &str,
    ) {
        let mut tree = VirtualTree::new();
        let div = sample(&mut tree);

        assert_eq!(tree.serialize_html(div, scope).unwrap(), expected);
    }

    #[rstest]
    #[case::inner(HtmlScope::Inner)]
    #[case::outer(HtmlScope::Outer)]
    fn a_fragment_writes_its_children_in_both_scopes(#[case] scope: HtmlScope) {
        let mut tree = VirtualTree::new();
        let fragment = node(&mut tree, FRAGMENT_TAG);
        let div = sample(&mut tree);
        tree.append_child(fragment, div).unwrap();

        assert_eq!(
            tree.serialize_html(fragment, scope).unwrap(),
            "<div>t<!--c--></div>"
        );
    }

    #[rstest]
    #[case::text_inner(TEXT_TAG, HtmlScope::Inner, "")]
    #[case::text_outer(TEXT_TAG, HtmlScope::Outer, "a&amp;b")]
    #[case::comment_inner(COMMENT_TAG, HtmlScope::Inner, "")]
    #[case::comment_outer(COMMENT_TAG, HtmlScope::Outer, "<!--a&b-->")]
    fn a_leaf_has_no_inner_markup(
        #[case] tag: &str,
        #[case] scope: HtmlScope,
        #[case] expected: &str,
    ) {
        let mut tree = VirtualTree::new();
        let leaf = node(&mut tree, tag);
        tree.set_attribute(leaf, "value", "a&b").unwrap();

        assert_eq!(tree.serialize_html(leaf, scope).unwrap(), expected);
    }

    #[test]
    fn an_unknown_node_is_an_error() {
        assert_eq!(
            VirtualTree::new().serialize_html(3, HtmlScope::Inner),
            Err(TreeError::NodeNotFound(3))
        );
    }

    fn template_with_own_child(tree: &mut VirtualTree) -> NodeId {
        let html = tree
            .parse_html(None, "<template><p>x</p></template>")
            .unwrap();
        let template = html.nodes[0].id;
        let own = node(tree, "span");
        tree.append_child(template, own).unwrap();
        template
    }

    #[rstest]
    #[case::inner(HtmlScope::Inner, "<p>x</p>")]
    #[case::outer(HtmlScope::Outer, "<template><p>x</p></template>")]
    fn a_template_writes_its_content_as_its_children(
        #[case] scope: HtmlScope,
        #[case] expected: &str,
    ) {
        let mut tree = VirtualTree::new();
        let template = template_with_own_child(&mut tree);

        assert_eq!(tree.serialize_html(template, scope).unwrap(), expected);
    }

    #[test]
    fn a_style_entry_without_declarations_writes_an_empty_attribute() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        tree.set_attribute(div, "style", "").unwrap();

        assert_eq!(outer(&tree, div), "<div style=\"\"></div>");
    }

    #[test]
    fn a_node_without_style_writes_no_style_attribute() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        tree.set_style(div, "width", "").unwrap();

        assert_eq!(outer(&tree, div), "<div></div>");
    }

    #[test]
    fn the_style_attribute_keeps_the_position_of_the_entry() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        for key in ["a", "style", "b"] {
            tree.set_attribute(div, key, "1").unwrap();
        }
        tree.set_style(div, "width", "5px").unwrap();

        assert_eq!(
            outer(&tree, div),
            "<div a=\"1\" style=\"width: 5px;\" b=\"1\"></div>"
        );
    }

    #[test]
    fn declarations_are_written_in_the_order_they_were_set() {
        let mut tree = VirtualTree::new();
        let div = node(&mut tree, "div");
        tree.set_style(div, "width", 5.0).unwrap();
        tree.set_style(div, "text_color", "#f00").unwrap();
        tree.set_style(div, "corner_radius", 2.0).unwrap();

        assert_eq!(
            outer(&tree, div),
            "<div style=\"width: 5px; color: rgb(255, 0, 0); border-radius: 2px;\"></div>"
        );
    }

    #[test]
    fn deeply_nested_elements_serialize() {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("#fragment").unwrap();
        let mut parent = root;
        for _ in 0..10_000 {
            let child = tree.create_node("div").unwrap();
            tree.append_child(parent, child).unwrap();
            parent = child;
        }

        let written = tree.serialize_html(root, HtmlScope::Inner).unwrap();

        assert_eq!(written.len(), 10_000 * "<div></div>".len());
    }

    #[test]
    fn a_parsed_tree_and_its_template_content_round_trip() {
        let markup = "<table><tr><td class=a>x &amp; y</td></tr></table>\
            <template><b>t</b></template>\
            <my-el style=\"width: 5px; color: #f00\">&nbsp;<!--c--></my-el>";
        let mut tree = VirtualTree::new();
        let first = tree.parse_html(None, markup).unwrap();
        let written = tree
            .serialize_html(first.fragment, HtmlScope::Inner)
            .unwrap();

        let second = tree.parse_html(None, &written).unwrap();

        let content = |html: &crate::ParsedHtml| {
            let template = html
                .nodes
                .iter()
                .find(|entry| entry.tag == "template")
                .unwrap();
            tree.template_content(template.id).unwrap()
        };
        assert!(tree.is_equal(first.fragment, second.fragment).unwrap());
        assert!(tree.is_equal(content(&first), content(&second)).unwrap());
    }
}
