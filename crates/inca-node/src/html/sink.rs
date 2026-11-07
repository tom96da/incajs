// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tree sink that builds a parsed fragment directly in a [`VirtualTree`].
//!
//! A handle below `2^32` is the id of a node of the tree. A handle from `2^32` up stands in for
//! a node that could not be created, so the parse can finish and report the failure.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use html5ever::interface::{
    AppendNode, AppendText, ElemName, ElementFlags, NodeOrText, QuirksMode, TreeSink,
};
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{
    BufferQueue, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};
use html5ever::tree_builder::{TreeBuilder, TreeBuilderOpts};
use html5ever::{Attribute, LocalName, Namespace, QualName, TokenizerResult, local_name, ns};

use crate::tree::{COMMENT_TAG, FRAGMENT_TAG, NodeId, TEXT_TAG};
use crate::tree::{TreeError, VirtualNode, VirtualTree};

pub(super) type Handle = u64;

/// The first handle that stands in for a node.
const STAND_IN: Handle = 1 << 32;

/// The result of a parse: the first failure and the warnings of the `style` attributes.
#[derive(Debug, Default)]
pub(super) struct SinkEnd {
    /// Every node the sink created, the document and the context excluded.
    pub(super) created: Vec<NodeId>,
    pub(super) failure: Option<TreeError>,
    pub(super) warnings: Vec<(NodeId, String)>,
}

/// The name of an element as the tree builder reads it.
#[derive(Debug)]
pub(super) struct ElementName {
    ns: Namespace,
    local: LocalName,
}

impl ElemName for ElementName {
    fn ns(&self) -> &Namespace {
        &self.ns
    }

    fn local_name(&self) -> &LocalName {
        &self.local
    }
}

pub(super) struct HtmlSink<'a> {
    pub(super) tree: RefCell<&'a mut VirtualTree>,
    /// The qualified name of every element the tree builder created, the context included.
    pub(super) names: RefCell<HashMap<Handle, QualName>>,
    failure: Cell<Option<TreeError>>,
    warnings: RefCell<Vec<(NodeId, String)>>,
    created: RefCell<Vec<NodeId>>,
    next_stand_in: Cell<Handle>,
    document: Handle,
    /// The number of open elements the tree builder has down to each handle, the root included.
    pub(super) stack_depth: RefCell<HashMap<Handle, u32>>,
    /// The template that each content fragment handed to the tree builder belongs to.
    pub(super) content_owner: RefCell<HashMap<Handle, Handle>>,
    /// The nodes put into the tree while the current token is processed. Their parent counts as
    /// none until the token ends.
    pub(super) attached: RefCell<HashSet<Handle>>,
    /// The last table section or row put into each element, by the element it was meant for.
    pub(super) last_section: RefCell<HashMap<Handle, Handle>>,
    /// The elements the tree builder has closed.
    pub(super) closed: RefCell<HashSet<Handle>>,
}

/// The id a handle names, `None` for a stand-in.
pub(super) fn node_of(handle: Handle) -> Option<NodeId> {
    NodeId::try_from(handle).ok()
}

impl<'a> HtmlSink<'a> {
    /// A sink whose document is a new fragment of `tree`.
    pub(super) fn new(tree: &'a mut VirtualTree) -> Result<Self, TreeError> {
        let document = tree.create_node(FRAGMENT_TAG)?;
        Ok(Self {
            tree: RefCell::new(tree),
            names: RefCell::default(),
            failure: Cell::new(None),
            warnings: RefCell::default(),
            created: RefCell::default(),
            next_stand_in: Cell::new(STAND_IN),
            document: Handle::from(document),
            stack_depth: RefCell::default(),
            content_owner: RefCell::default(),
            attached: RefCell::default(),
            last_section: RefCell::default(),
            closed: RefCell::default(),
        })
    }

    pub(super) fn document_id(&self) -> NodeId {
        node_of(self.document).unwrap_or_default()
    }

    /// Creates an element of the HTML namespace that the tree builder takes as the context.
    pub(super) fn create_context(&self, tag: &str) -> Handle {
        let name = QualName::new(None, ns!(html), LocalName::from(tag));
        self.create_element(name, Vec::new(), ElementFlags::default())
    }

    fn stand_in(&self) -> Handle {
        let handle = self.next_stand_in.get();
        self.next_stand_in.set(handle + 1);
        handle
    }

    /// Runs `change` on the tree unless a failure is stored. A failed create stores the failure.
    /// Any other error names an id that is not in the tree and changes nothing.
    fn change<R>(
        &self,
        change: impl FnOnce(&mut VirtualTree) -> Result<R, TreeError>,
    ) -> Option<R> {
        if self.failure.get().is_some() {
            return None;
        }
        match change(&mut self.tree.borrow_mut()) {
            Ok(result) => Some(result),
            Err(TreeError::IdsExhausted) => {
                self.failure.set(Some(TreeError::IdsExhausted));
                None
            }
            Err(_) => None,
        }
    }

    /// Creates a node of `tag` with `data` as its `value` attribute.
    fn leaf(&self, tag: &str, data: &str) -> Handle {
        let created = self.change(|tree| {
            let id = tree.create_node(tag)?;
            tree.set_attribute(id, "value", data)?;
            Ok(id)
        });
        if let Some(id) = created {
            self.created.borrow_mut().push(id);
        }
        created.map_or_else(|| self.stand_in(), Handle::from)
    }

    fn comment(&self, data: &str) -> Handle {
        let handle = self.leaf(COMMENT_TAG, data);
        if let Some(id) = node_of(handle) {
            self.change(|tree| tree.set_style(id, "display", "none"));
        }
        handle
    }

    /// Puts `child` into `parent`, before `before` when it is given. Text joins the text node
    /// that touches it.
    fn insert(&self, parent: Handle, before: Option<Handle>, child: NodeOrText<Handle>) {
        let (Some(parent_id), before_id) = (node_of(parent), before.and_then(node_of)) else {
            return;
        };
        match child {
            AppendNode(child) => {
                if let Some(child_id) = node_of(child) {
                    let (parent_id, before_id) = self.landing(parent_id, before, child);
                    self.change(|tree| tree.insert_before(parent_id, child_id, before_id));
                }
            }
            AppendText(text) => {
                let touching = self.change(|tree| {
                    Ok(match before_id {
                        Some(sibling) => tree.previous_sibling(sibling),
                        None => tree.last_child(parent_id),
                    }
                    .filter(|&id| tree.get(id).is_some_and(|node| node.tag_name() == TEXT_TAG)))
                });
                if let Some(id) = touching.flatten() {
                    self.change(|tree| tree.append_text(id, &text));
                } else {
                    let created = self.leaf(TEXT_TAG, &text);
                    if let Some(id) = node_of(created) {
                        self.change(|tree| tree.insert_before(parent_id, id, before_id));
                    }
                }
            }
        }
    }
}

/// The tree builder behind a token sink that tells the sink where each token ends.
struct Tokens<'a>(TreeBuilder<Handle, HtmlSink<'a>>);

impl TokenSink for Tokens<'_> {
    type Handle = Handle;

    fn process_token(&self, token: Token, line_number: u64) -> TokenSinkResult<Handle> {
        let result = self.0.process_token(token, line_number);
        self.0.sink.end_token();
        result
    }

    fn end(&self) {
        self.0.end();
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        self.0
            .adjusted_current_node_present_but_not_in_html_namespace()
    }
}

/// Parses `markup` as the children of the element `context` into `sink`.
pub(super) fn parse_fragment(sink: HtmlSink<'_>, context: Handle, markup: &str) -> SinkEnd {
    let builder = TreeBuilder::new_for_fragment(sink, context, None, TreeBuilderOpts::default());
    let initial_state = builder.tokenizer_state_for_context_elem(true);
    let tokenizer = Tokenizer::new(
        Tokens(builder),
        TokenizerOpts {
            initial_state: Some(initial_state),
            ..TokenizerOpts::default()
        },
    );
    let input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(markup));
    while !matches!(tokenizer.feed(&input), TokenizerResult::Done) {}
    tokenizer.end();
    tokenizer.sink.0.sink.finish()
}

/// The key of an attribute: its name with the prefix, in ASCII lower case.
fn attribute_key(name: &QualName) -> String {
    let local = name.local.to_ascii_lowercase().to_string();
    match &name.prefix {
        Some(prefix) => format!("{}:{local}", prefix.to_ascii_lowercase()),
        None => local,
    }
}

impl TreeSink for HtmlSink<'_> {
    type Handle = Handle;
    type Output = SinkEnd;
    type ElemName<'a>
        = ElementName
    where
        Self: 'a;

    fn finish(self) -> SinkEnd {
        SinkEnd {
            created: self.created.into_inner(),
            failure: self.failure.get(),
            warnings: self.warnings.into_inner(),
        }
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        self.document
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> ElementName {
        let names = self.names.borrow();
        match names.get(target) {
            Some(name) => ElementName {
                ns: name.ns.clone(),
                local: name.local.clone(),
            },
            None => ElementName {
                ns: ns!(html),
                local: local_name!("div"),
            },
        }
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> Handle {
        let created = self.change(|tree| {
            let id = tree.create_node(&*name.local)?;
            let mut warnings = Vec::new();
            for attribute in &attrs {
                let key = attribute_key(&attribute.name);
                if key == "style" {
                    let lines = tree.set_style_attribute(id, &attribute.value)?;
                    warnings.extend(lines.into_iter().map(|line| (id, line)));
                } else {
                    tree.set_attribute(id, key, &*attribute.value)?;
                }
            }
            Ok((id, warnings))
        });
        let handle = match created {
            Some((id, warnings)) => {
                self.warnings.borrow_mut().extend(warnings);
                self.created.borrow_mut().push(id);
                Handle::from(id)
            }
            None => self.stand_in(),
        };
        self.names.borrow_mut().insert(handle, name);
        handle
    }

    fn create_comment(&self, text: StrTendril) -> Handle {
        self.comment(&text)
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        self.comment(&format!("?{target} {data}"))
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        self.insert(*parent, None, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        let has_parent = node_of(*element)
            .and_then(|id| self.tree.borrow().get(id).and_then(VirtualNode::parent))
            .is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public: StrTendril,
        _system: StrTendril,
    ) {
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        let content = node_of(*target).and_then(|id| self.tree.borrow().template_content(id));
        let Some(content) = content else {
            return self.stand_in();
        };
        let handle = Handle::from(content);
        let depth = self.stack_depth.borrow().get(target).copied().unwrap_or(0);
        self.stack_depth.borrow_mut().insert(handle, depth);
        self.content_owner.borrow_mut().insert(handle, *target);
        handle
    }

    fn pop(&self, node: &Handle) {
        self.closed.borrow_mut().insert(*node);
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        let parent = node_of(*sibling)
            .and_then(|id| self.tree.borrow().get(id).and_then(VirtualNode::parent));
        if let Some(parent) = parent {
            self.insert(Handle::from(parent), Some(*sibling), new_node);
        }
    }

    // The tree builder calls this for the `html` and `body` start tags. In a fragment parse the
    // first reaches the discarded root and the second finds no `body` to change.
    fn add_attrs_if_missing(&self, _target: &Handle, _attrs: Vec<Attribute>) {}

    fn remove_from_parent(&self, target: &Handle) {
        let Some(id) = node_of(*target) else { return };
        self.change(|tree| {
            if let Some(parent) = tree.get(id).and_then(VirtualNode::parent) {
                tree.remove_child(parent, id)?;
            }
            Ok(())
        });
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let (Some(from), Some(to)) = (node_of(*node), node_of(*new_parent)) else {
            return;
        };
        self.change(|tree| tree.move_children(from, to));
    }

    fn allow_declarative_shadow_roots(&self, _intended_parent: &Handle) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use html5ever::interface::{AppendNode, AppendText, ElemName, ElementFlags, TreeSink};
    use html5ever::tendril::StrTendril;
    use html5ever::{LocalName, QualName, ns};
    use rstest::rstest;

    use super::*;
    use crate::html::{children_shape, shape};

    fn parse(markup: &str) -> (VirtualTree, crate::ParsedHtml) {
        let mut tree = VirtualTree::new();
        let div = tree.create_node("div").unwrap();
        let html = tree.parse_html(Some(div), markup).unwrap();
        (tree, html)
    }

    fn shape_of(markup: &str) -> String {
        let (tree, html) = parse(markup);
        children_shape(&tree, html.fragment)
    }

    fn attributes_of(markup: &str) -> Vec<(String, String)> {
        let (tree, html) = parse(markup);
        tree.get(html.nodes[0].id)
            .unwrap()
            .attributes()
            .iter()
            .map(|(key, value)| (key.clone(), value.text()))
            .collect()
    }

    fn pair(key: &str, value: &str) -> (String, String) {
        (key.to_owned(), value.to_owned())
    }

    #[rstest]
    #[case::lower_case_key("<div id=a DATA-X=B>", vec![pair("id", "a"), pair("data-x", "B")])]
    #[case::valueless("<input disabled>", vec![pair("disabled", "")])]
    #[case::camel_case_foreign_key("<svg viewBox='0 0 1 1'>", vec![pair("viewbox", "0 0 1 1")])]
    #[case::prefixed_key("<svg xlink:href=u>", vec![pair("xlink:href", "u")])]
    fn an_element_keeps_its_attributes_as_lower_case_strings(
        #[case] markup: &str,
        #[case] expected: Vec<(String, String)>,
    ) {
        assert_eq!(attributes_of(markup), expected);
    }

    #[rstest]
    #[case::entity("a&amp;b", "\"a&b\"")]
    #[case::around_a_comment("a<!--c-->b", "\"a\" <!--c--> \"b\"")]
    fn adjacent_text_becomes_one_node(#[case] markup: &str, #[case] expected: &str) {
        assert_eq!(shape_of(markup), expected);
    }

    #[rstest]
    #[case::text_before_the_table(
        "<table>x<tr><td>y</td></tr></table>",
        "\"x\" table(tbody(tr(td(\"y\"))))"
    )]
    #[case::element_before_the_table("<table><div>", "div table")]
    #[case::text_joins_the_text_before_the_table("x<table>y", "\"xy\" table")]
    fn foster_parented_nodes_go_before_the_table(#[case] markup: &str, #[case] expected: &str) {
        assert_eq!(shape_of(markup), expected);
    }

    #[test]
    fn the_adoption_agency_rebuilds_the_formatting_element() {
        assert_eq!(shape_of("<b>1<p>2</b>3</p>"), "b(\"1\") p(b(\"2\") \"3\")");
    }

    fn element(sink: &HtmlSink<'_>, tag: &str) -> Handle {
        sink.create_element(
            QualName::new(None, ns!(html), LocalName::from(tag)),
            Vec::new(),
            ElementFlags::default(),
        )
    }

    fn text(data: &str) -> NodeOrText<Handle> {
        AppendText(StrTendril::from_slice(data))
    }

    #[test]
    fn an_element_with_a_parent_takes_the_child_before_itself() {
        let mut tree = VirtualTree::new();
        let sink = HtmlSink::new(&mut tree).unwrap();
        let parent = element(&sink, "div");
        let target = element(&sink, "p");
        let previous = element(&sink, "span");
        sink.append(&parent, AppendNode(target));

        sink.append_based_on_parent_node(&target, &previous, text("x"));

        sink.finish();
        assert_eq!(children_shape(&tree, 1), "\"x\" p");
    }

    #[test]
    fn an_element_without_a_parent_gives_the_child_to_the_previous_element() {
        let mut tree = VirtualTree::new();
        let sink = HtmlSink::new(&mut tree).unwrap();
        let target = element(&sink, "p");
        let previous = element(&sink, "span");

        sink.append_based_on_parent_node(&target, &previous, text("x"));

        sink.finish();
        assert_eq!(shape(&tree, 2), "span(\"x\")");
    }

    #[test]
    fn reparent_children_moves_them_in_order() {
        let mut tree = VirtualTree::new();
        let sink = HtmlSink::new(&mut tree).unwrap();
        let from = element(&sink, "div");
        let to = element(&sink, "section");
        sink.append(&from, text("a"));
        sink.append(&from, AppendNode(element(&sink, "i")));

        sink.reparent_children(&from, &to);

        sink.finish();
        assert_eq!(shape(&tree, 2), "section(\"a\" i)");
        assert_eq!(shape(&tree, 1), "div");
    }

    #[test]
    fn remove_from_parent_detaches_the_node() {
        let mut tree = VirtualTree::new();
        let sink = HtmlSink::new(&mut tree).unwrap();
        let parent = element(&sink, "div");
        let child = element(&sink, "p");
        sink.append(&parent, AppendNode(child));

        sink.remove_from_parent(&child);

        sink.finish();
        assert_eq!(shape(&tree, 1), "div");
    }

    #[test]
    fn a_template_holds_the_parsed_children_in_its_content() {
        assert_eq!(
            shape_of("<template><p>x</p><template><i></i></template></template>"),
            "template{p(\"x\") template{i}}"
        );
    }

    #[test]
    fn a_declarative_shadow_template_stays_a_template_with_its_attribute() {
        let (tree, html) = parse("<template shadowrootmode=open><p></p></template>");

        let template = tree.get(html.nodes[0].id).unwrap();
        assert_eq!(
            template.attributes().get("shadowrootmode"),
            Some(&"open".into())
        );
        assert_eq!(shape(&tree, template.id()), "template{p}");
    }

    #[test]
    fn a_cdata_section_in_a_foreign_element_is_text() {
        assert_eq!(shape_of("<svg><![CDATA[x]]></svg>"), "svg(\"x\")");
    }

    #[test]
    fn a_comment_is_a_hidden_node_with_its_text() {
        let (tree, html) = parse("<!--c-->");

        let comment = tree.get(html.nodes[0].id).unwrap();
        assert_eq!(comment.tag_name(), COMMENT_TAG);
        assert_eq!(comment.data(), "c");
        assert_eq!(comment.style_props().get("display"), Some(&"none".into()));
    }

    #[test]
    fn a_self_closing_foreign_element_has_no_children() {
        assert_eq!(shape_of("<svg><circle/><rect/></svg>"), "svg(circle rect)");
    }

    #[test]
    fn a_foreign_element_keeps_the_case_of_its_tag() {
        assert_eq!(shape_of("<svg><foreignObject></svg>"), "svg(foreignObject)");
    }

    #[test]
    fn running_out_of_ids_during_a_parse_is_an_error() {
        let mut tree = VirtualTree::new();
        tree.set_next_id(NodeId::MAX - 5);

        let result = tree.parse_html(None, "<table><tr><td>x</td></tr></table>");

        assert_eq!(result, Err(TreeError::IdsExhausted));
    }

    #[test]
    fn an_unknown_handle_names_a_div() {
        let mut tree = VirtualTree::new();
        let sink = HtmlSink::new(&mut tree).unwrap();

        let name = sink.elem_name(&99);

        assert_eq!(*name.local_name(), LocalName::from("div"));
        assert_eq!(*name.ns(), ns!(html));
    }

    #[test]
    fn appending_to_an_unknown_handle_is_ignored() {
        let mut tree = VirtualTree::new();
        let sink = HtmlSink::new(&mut tree).unwrap();

        sink.append(&98, AppendNode(99));

        assert_eq!(sink.finish().failure, None);
    }

    #[test]
    fn removing_an_unknown_handle_is_ignored() {
        let mut tree = VirtualTree::new();
        let sink = HtmlSink::new(&mut tree).unwrap();

        sink.remove_from_parent(&99);

        assert_eq!(sink.finish().failure, None);
    }

    #[test]
    fn a_failed_parse_leaves_nothing_in_the_tree() {
        let mut tree = VirtualTree::new();
        tree.set_next_id(NodeId::MAX - 5);

        tree.parse_html(None, "<table><tr><td>x</td></tr></table>")
            .unwrap_err();

        assert!(tree.sweep().is_empty());
    }
}
