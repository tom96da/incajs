// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Parsing a markup fragment into new nodes of the tree.

use super::sink::{Context, HtmlSink, node_of, parse_fragment};
use crate::tree::{FRAGMENT_TAG, NodeId, NodeKind, TreeError, VirtualTree};

/// What a `style` hook receives, the tree, the element and the attribute text, and what it
/// returns, the lines to report.
pub type StyleHook<'a> =
    dyn FnMut(&mut VirtualTree, NodeId, &str) -> Result<Vec<String>, TreeError> + 'a;

/// The hook of [`VirtualTree::parse_html`]: stores the attribute as written and reports nothing.
pub(super) fn store_as_written(
    tree: &mut VirtualTree,
    id: NodeId,
    text: &str,
) -> Result<Vec<String>, TreeError> {
    tree.set_attribute(id, "style", text)?;
    Ok(Vec::new())
}

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
    /// The lines the `style` hook returned, in parse order, each with its node. A node the parser
    /// drops from the result is listed too.
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
        self.parse_html_with(context, markup, &mut store_as_written)
    }

    /// Parses like [`VirtualTree::parse_html`] and calls `style` with the tree, the element and
    /// the text of each `style` attribute as the parser creates the element, in source order.
    /// The hook stores the attribute and returns the lines to report, which come back in
    /// [`ParsedHtml::warnings`].
    ///
    /// # Errors
    ///
    /// The errors of [`VirtualTree::parse_html`].
    pub fn parse_html_with(
        &mut self,
        context: Option<NodeId>,
        markup: &str,
        style: &mut StyleHook<'_>,
    ) -> Result<ParsedHtml, TreeError> {
        let context = match context {
            None => Context::body(),
            Some(id) => {
                let node = self.get(id).ok_or(TreeError::NodeNotFound(id))?;
                match node.kind() {
                    NodeKind::Text | NodeKind::Comment => return Err(TreeError::LeafParent(id)),
                    NodeKind::Fragment => Context::body(),
                    _ => Context::of(node),
                }
            }
        };
        let sink = HtmlSink::new(self, style)?;
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
            entries.push(ParsedNode {
                id,
                tag: node.local_name().to_owned(),
                namespace: node.namespace_uri().map(str::to_owned),
                parent: Some(parent),
                data: node.is_character_data().then(|| node.data().to_owned()),
            });
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{children_shape, node, parsed};

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

    #[test]
    fn a_style_attribute_is_kept_as_written_and_sets_no_style_props() {
        let (tree, html) = parsed(Some("div"), r#"<p style="COLOR: #f00; bogus:1">"#);

        let p = tree.get(html.nodes[0].id).unwrap();
        assert_eq!(
            p.attributes().get("style"),
            Some(&"COLOR: #f00; bogus:1".into())
        );
        assert!(p.style_props().is_empty());
        assert!(html.warnings.is_empty());
    }

    /// Parses with a hook that records each call and returns the line `warn <text>`.
    fn hooked(context: &str, markup: &str) -> (ParsedHtml, Vec<(NodeId, String)>) {
        let mut tree = VirtualTree::new();
        let context = node(&mut tree, context);
        let mut calls = Vec::new();
        let html = tree
            .parse_html_with(Some(context), markup, &mut |_, id, text| {
                calls.push((id, text.to_owned()));
                Ok(vec![format!("warn {text}")])
            })
            .unwrap();
        (html, calls)
    }

    #[test]
    fn the_style_hook_runs_for_each_style_attribute_in_parse_order() {
        let (html, calls) = hooked(
            "div",
            "<p style=a><i style=b></i></p><u style=c></u><s></s>",
        );

        let ids: Vec<NodeId> = html.nodes.iter().map(|entry| entry.id).collect();
        assert_eq!(
            calls,
            [
                (ids[0], "a".to_owned()),
                (ids[1], "b".to_owned()),
                (ids[2], "c".to_owned()),
            ]
        );
    }

    #[test]
    fn the_hook_lines_come_back_as_warnings_in_parse_order() {
        let (html, _) = hooked("div", "<p style=a><i style=b></i></p><u style=c></u>");

        let ids: Vec<NodeId> = html.nodes.iter().map(|entry| entry.id).collect();
        assert_eq!(
            html.warnings,
            [
                (ids[0], "warn a".to_owned()),
                (ids[1], "warn b".to_owned()),
                (ids[2], "warn c".to_owned()),
            ]
        );
    }

    #[test]
    fn an_element_dropped_from_the_result_still_warns_once() {
        let (html, calls) = hooked("html", "<p style=x><frameset>");

        let dropped = calls[0].0;
        assert!(html.nodes.iter().all(|entry| entry.id != dropped));
        assert_eq!(html.warnings, [(dropped, "warn x".to_owned())]);
    }

    #[test]
    fn a_hook_stores_the_attribute_before_the_parse_returns() {
        let mut tree = VirtualTree::new();
        let context = node(&mut tree, "div");

        let html = tree
            .parse_html_with(Some(context), "<p style=a id=b>", &mut |tree, id, text| {
                tree.set_attribute(id, "style", format!("hooked {text}"))?;
                Ok(Vec::new())
            })
            .unwrap();

        let keys: Vec<(&String, String)> = tree
            .get(html.nodes[0].id)
            .unwrap()
            .attributes()
            .iter()
            .map(|(key, value)| (key, value.text().clone()))
            .collect();
        assert_eq!(
            keys,
            [
                (&"style".to_owned(), "hooked a".to_owned()),
                (&"id".to_owned(), "b".to_owned())
            ]
        );
    }
}
