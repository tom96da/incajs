// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Parsing HTML into the tree.
//!
//! [`VirtualTree::parse_html`] reads a markup fragment with the HTML fragment parsing algorithm
//! and builds the nodes directly in the tree. The result is a fragment that has no parent and is
//! not held, so the caller moves its children and destroys it in the same task.
//!
//! [`VirtualTree::serialize_html`] writes a subtree back as markup.
//!
//! An element keeps the namespace the parser reports. The names and attribute keys of the HTML
//! namespace are in lower case, and those of `svg` and `math` content keep the case the parser
//! delivers, such as `viewBox`. A `style` attribute applies as inline style props and no `style`
//! attribute is stored.

mod depth;
mod serialize;
mod sink;

use crate::tree::{COMMENT_TAG, FRAGMENT_TAG, NodeId, TEXT_TAG, TreeError, VirtualTree};
pub use serialize::HtmlScope;
use sink::{Context, HtmlSink, node_of, parse_fragment};

/// One node of a parsed fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNode {
    pub id: NodeId,
    pub tag: String,
    /// The namespace of an element, `None` for a text node, a comment and a fragment.
    pub namespace: Option<String>,
    /// The node that holds this one. A top-level node has the result fragment, and the content
    /// fragment of a template has the template.
    pub parent: Option<NodeId>,
    /// The text of a `#text` or `#comment` node.
    pub data: Option<String>,
}

/// The result of [`VirtualTree::parse_html`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedHtml {
    /// The fragment that holds the parsed nodes. It has no parent and is not held.
    pub fragment: NodeId,
    /// Every node below `fragment` in pre-order. A template is followed by its content fragment
    /// and that fragment's subtree, then by its own children.
    pub nodes: Vec<ParsedNode>,
    /// One line for each declaration of a `style` attribute that was dropped, with the node.
    pub warnings: Vec<(NodeId, String)>,
}

impl VirtualTree {
    /// Parses `markup` as the children of `context` and returns the nodes in a new fragment.
    ///
    /// The namespace and tag of `context` select the tokenizer state and the insertion mode. A
    /// fragment and `None` stand for `body`. The context node is not changed.
    ///
    /// An element or comment appended to an open element at a depth of 512 levels lands beside
    /// that element, and text stays in its element. Formatting elements that the parser reopens
    /// nest one level deeper each, and each of the eight repairs that one misnested end tag or
    /// one `a` or `nobr` start tag starts nests up to five levels deeper.
    ///
    /// # Errors
    ///
    /// - [`TreeError::NodeNotFound`] if `context` names no node.
    /// - [`TreeError::LeafParent`] if `context` is a text node or a comment.
    /// - [`TreeError::IdsExhausted`] when no id is left. The tree keeps none of the nodes made.
    pub fn parse_html(
        &mut self,
        context: Option<NodeId>,
        markup: &str,
    ) -> Result<ParsedHtml, TreeError> {
        let context = match context {
            None => Context::body(),
            Some(id) => {
                let node = self.get(id).ok_or(TreeError::NodeNotFound(id))?;
                match node.tag_name() {
                    TEXT_TAG | COMMENT_TAG => return Err(TreeError::LeafParent(id)),
                    FRAGMENT_TAG => Context::body(),
                    _ => Context::of(node),
                }
            }
        };
        let sink = HtmlSink::new(self)?;
        let document = sink.document_id();
        let context_handle = sink.create_context(&context);
        let context_id = node_of(context_handle);
        let end = parse_fragment(sink, context_handle, markup);
        let outcome = match end.failure {
            Some(failure) => Err(failure),
            None => self.collect(document),
        };
        self.destroy_node(document);
        if let Some(id) = context_id {
            self.destroy_node(id);
        }
        let fragment = match outcome {
            Ok(fragment) => fragment,
            Err(failure) => {
                for id in end.created {
                    self.destroy_node(id);
                }
                return Err(failure);
            }
        };
        Ok(ParsedHtml {
            fragment,
            nodes: self.entries(fragment),
            warnings: end.warnings,
        })
    }

    /// Moves the children of the algorithm root under a new fragment and returns its id. Every
    /// move succeeds: the fragment is new and parentless, and each child is a node of this tree.
    fn collect(&mut self, document: NodeId) -> Result<NodeId, TreeError> {
        let fragment = self.create_node(FRAGMENT_TAG)?;
        if let Some(root) = self.first_child(document) {
            let _ = self.move_children(root, fragment);
        }
        Ok(fragment)
    }

    /// Every node below `fragment` in pre-order, with its parent.
    fn entries(&self, fragment: NodeId) -> Vec<ParsedNode> {
        let mut entries = Vec::new();
        let mut pending: Vec<(NodeId, NodeId)> = self
            .get(fragment)
            .map(|node| {
                node.children()
                    .iter()
                    .rev()
                    .map(|&c| (c, fragment))
                    .collect()
            })
            .unwrap_or_default();
        while let Some((id, parent)) = pending.pop() {
            let Some(node) = self.get(id) else { continue };
            pending.extend(node.children().iter().rev().map(|&child| (child, id)));
            pending.extend(self.template_content(id).map(|content| (content, id)));
            let is_leaf = matches!(node.tag_name(), TEXT_TAG | COMMENT_TAG);
            entries.push(ParsedNode {
                id,
                tag: node.tag_name().to_owned(),
                namespace: node.namespace_uri().map(str::to_owned),
                parent: Some(parent),
                data: is_leaf.then(|| node.data().to_owned()),
            });
        }
        entries
    }
}

/// The nodes below `id` as text: `tag[key=value](children)`, `"text"`, `<!--comment-->` and
/// `template{content}`.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::format_push_string)]
fn shape(tree: &VirtualTree, id: NodeId) -> String {
    let node = tree.get(id).unwrap();
    match node.tag_name() {
        TEXT_TAG => return format!("{:?}", node.data()),
        COMMENT_TAG => return format!("<!--{}-->", node.data()),
        _ => {}
    }
    let mut text = if node.tag_name() == FRAGMENT_TAG {
        String::new()
    } else {
        node.tag_name().to_owned()
    };
    if let Some(content) = tree.template_content(id) {
        text.push_str(&format!("{{{}}}", children_shape(tree, content)));
    }
    if !node.children().is_empty() {
        text.push_str(&format!("({})", children_shape(tree, id)));
    }
    text
}

/// The nodes under `id`, separated by spaces.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
fn children_shape(tree: &VirtualTree, id: NodeId) -> String {
    let children = tree.get(id).unwrap().children();
    let shapes: Vec<String> = children.iter().map(|&c| shape(tree, c)).collect();
    shapes.join(" ")
}

#[cfg(test)]
pub(super) mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::node;
    use crate::tree::AttributeValue;

    pub(super) fn parsed(context: Option<&str>, markup: &str) -> (VirtualTree, ParsedHtml) {
        let mut tree = VirtualTree::new();
        let context = context.map(|tag| node(&mut tree, tag));
        let html = tree.parse_html(context, markup).unwrap();
        (tree, html)
    }

    fn result_shape(context: Option<&str>, markup: &str) -> String {
        let (tree, html) = parsed(context, markup);
        children_shape(&tree, html.fragment)
    }

    #[rstest]
    #[case::tr_keeps_td(Some("tr"), "<td>a</td>", "td(\"a\")")]
    #[case::div_drops_td(Some("div"), "<td>a</td>", "\"a\"")]
    #[case::style_reads_raw_text(Some("style"), "<b>", "\"<b>\"")]
    #[case::template_accepts_td(Some("template"), "<td>a</td>", "td(\"a\")")]
    fn the_context_tag_selects_the_parsing_rules(
        #[case] context: Option<&str>,
        #[case] markup: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(result_shape(context, markup), expected);
    }

    #[test]
    fn the_context_tag_keeps_its_case() {
        assert_eq!(result_shape(Some("TR"), "<td>a</td>"), "\"a\"");
    }

    #[rstest]
    #[case::none(None)]
    #[case::fragment(Some("#fragment"))]
    fn no_context_and_a_fragment_context_read_as_body(#[case] context: Option<&str>) {
        assert_eq!(result_shape(context, "<td>a</td>"), "\"a\"");
    }

    #[rstest]
    #[case::text("#text")]
    #[case::comment("#comment")]
    fn a_leaf_context_is_refused(#[case] tag: &str) {
        let mut tree = VirtualTree::new();
        let leaf = node(&mut tree, tag);

        assert_eq!(
            tree.parse_html(Some(leaf), "x"),
            Err(TreeError::LeafParent(leaf))
        );
    }

    #[test]
    fn an_unknown_context_is_refused() {
        let mut tree = VirtualTree::new();

        assert_eq!(
            tree.parse_html(Some(7), "x"),
            Err(TreeError::NodeNotFound(7))
        );
    }

    #[test]
    fn a_parse_leaves_only_the_result_in_the_tree() {
        let mut tree = VirtualTree::new();
        let context = node(&mut tree, "div");
        tree.hold(context).unwrap();
        let html = tree
            .parse_html(
                Some(context),
                "<p>a<!--c--></p><template><i></i></template>",
            )
            .unwrap();

        let mut freed = tree.sweep();

        let mut listed: Vec<NodeId> = html.nodes.iter().map(|entry| entry.id).collect();
        listed.push(html.fragment);
        freed.sort_unstable();
        listed.sort_unstable();
        assert_eq!(freed, listed);
    }

    #[test]
    fn the_result_fragment_has_no_parent() {
        let (tree, html) = parsed(Some("div"), "<a></a><b></b>");

        assert_eq!(tree.get(html.fragment).unwrap().parent(), None);
    }

    #[test]
    fn the_result_fragment_holds_the_parsed_nodes_in_order() {
        let (tree, html) = parsed(Some("div"), "<a></a><b></b>");

        assert_eq!(children_shape(&tree, html.fragment), "a b");
    }

    #[test]
    fn the_entries_are_pre_order_with_parents_and_data() {
        let (_, html) = parsed(Some("div"), "<p>a<!--c--></p><template><i></i></template>");

        let list: Vec<(&str, Option<&str>)> = html
            .nodes
            .iter()
            .map(|entry| (entry.tag.as_str(), entry.data.as_deref()))
            .collect();
        assert_eq!(
            list,
            [
                ("p", None),
                ("#text", Some("a")),
                ("#comment", Some("c")),
                ("template", None),
                ("#fragment", None),
                ("i", None),
            ]
        );
    }

    #[test]
    fn the_entries_name_the_holder_of_each_node() {
        let (_, html) = parsed(Some("div"), "<p>a</p><template><i></i></template>");

        let parents: Vec<NodeId> = html
            .nodes
            .iter()
            .map(|entry| entry.parent.unwrap())
            .collect();
        let ids: Vec<NodeId> = html.nodes.iter().map(|entry| entry.id).collect();
        // p, its text, template, its content, i
        assert_eq!(
            parents,
            [html.fragment, ids[0], html.fragment, ids[2], ids[3]]
        );
    }

    fn props(markup: &str) -> (VirtualTree, ParsedHtml) {
        parsed(Some("div"), markup)
    }

    fn style_prop(tree: &VirtualTree, html: &ParsedHtml, key: &str) -> Option<AttributeValue> {
        tree.get(html.nodes[0].id)
            .unwrap()
            .style_props()
            .get(key)
            .cloned()
    }

    #[rstest]
    #[case::normalised_key("<div style='COLOR: #f00'>", "text_color", "#f00")]
    #[case::later_wins("<div style='width: 1px; width: 2px'>", "width", "2px")]
    #[case::invalid_later_keeps_earlier("<div style='width: 1px; width: bogus'>", "width", "1px")]
    #[case::important_reads_as_normal("<div style='width: 3px !important'>", "width", "3px")]
    fn a_style_attribute_applies_as_style_props(
        #[case] markup: &str,
        #[case] key: &str,
        #[case] value: &str,
    ) {
        let (tree, html) = props(markup);

        assert_eq!(style_prop(&tree, &html, key), Some(value.into()));
    }

    #[test]
    fn a_style_attribute_is_stored_at_its_source_position() {
        let (tree, html) = props("<div style='width: 1px' id='a'>");

        let keys: Vec<&String> = tree
            .get(html.nodes[0].id)
            .unwrap()
            .attributes()
            .keys()
            .collect();
        assert_eq!(keys, ["style", "id"]);
    }

    #[test]
    fn a_style_attribute_is_stored_as_the_derived_text() {
        let (tree, html) = props("<div style='WIDTH:1px; bogus: 1; height:2PX'>");

        let attributes = tree.get(html.nodes[0].id).unwrap().attributes();
        assert_eq!(
            attributes.get("style"),
            Some(&AttributeValue::String("width: 1px; height: 2px;".into()))
        );
    }

    #[test]
    fn a_dropped_declaration_warns_with_the_node() {
        let (_, html) = props("<div style='bogus: 1; width: x; color'>");

        let id = html.nodes[0].id;
        assert_eq!(
            html.warnings,
            [
                (id, "ignoring unknown style key `bogus`".to_owned()),
                (
                    id,
                    "ignoring unsupported value for style key `width`".to_owned()
                ),
                (id, "ignoring the declaration `color`".to_owned()),
            ]
        );
    }

    #[rstest]
    #[case::empty("<div style=''>")]
    #[case::blank("<div style='  '>")]
    #[case::valueless("<div style>")]
    fn an_empty_style_attribute_sets_and_warns_nothing(#[case] markup: &str) {
        let (tree, html) = props(markup);

        assert!(html.warnings.is_empty());
        assert!(tree.get(html.nodes[0].id).unwrap().style_props().is_empty());
    }
}
