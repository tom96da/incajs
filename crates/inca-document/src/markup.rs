// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The algorithms of the markup setters `innerHTML`, `outerHTML` and `insertAdjacentHTML`. A
//! setter parses the markup in the context of a node, then puts the parsed nodes in place and
//! reports each node that leaves the tree.

use inca_node::{AttributeValue, NodeId, ParsedHtml, TreeError, VirtualNode, VirtualTree};
use inca_style::set_style_text;

use crate::{DocumentState, View, settle};

/// Where `insertAdjacentHTML` puts the nodes, relative to the node it is called on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    BeforeBegin,
    AfterBegin,
    BeforeEnd,
    AfterEnd,
}

impl Position {
    /// Reads `text` ignoring ASCII case. Any other string gives `None`.
    #[must_use]
    pub fn from_name(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "beforebegin" => Some(Self::BeforeBegin),
            "afterbegin" => Some(Self::AfterBegin),
            "beforeend" => Some(Self::BeforeEnd),
            "afterend" => Some(Self::AfterEnd),
            _ => None,
        }
    }
}

/// The children of `id`, or none when the node is gone.
#[must_use]
pub fn children_of(tree: &VirtualTree, id: NodeId) -> Vec<NodeId> {
    tree.get(id)
        .map_or_else(Vec::new, |node| node.children().to_vec())
}

/// Parses `markup` for `innerHTML` on `node_id`. Returns the node that takes the children, which
/// is the content of a template, and the parse result.
///
/// # Errors
///
/// `NodeNotFound` when `node_id` is gone, or the error of the parse.
pub fn parse_inner(
    tree: &mut VirtualTree,
    node_id: NodeId,
    markup: &str,
) -> Result<(NodeId, ParsedHtml), TreeError> {
    tree.get(node_id).ok_or(TreeError::NodeNotFound(node_id))?;
    let container = tree.template_content(node_id).unwrap_or(node_id);
    let parsed = tree.parse_html_with(Some(node_id), markup, &mut set_style_text)?;
    Ok((container, parsed))
}

/// Parses `markup` for `outerHTML` on `node_id` in the context of its parent. A node without a
/// parent gives `None`.
///
/// # Errors
///
/// `NodeNotFound` when `node_id` is gone, `DocumentParent` when its parent is a document, or the
/// error of the parse.
pub fn parse_outer(
    tree: &mut VirtualTree,
    node_id: NodeId,
    markup: &str,
) -> Result<Option<ParsedHtml>, TreeError> {
    let node = tree.get(node_id).ok_or(TreeError::NodeNotFound(node_id))?;
    let Some(parent) = node.parent() else {
        return Ok(None);
    };
    if tree.get(parent).is_some_and(VirtualNode::is_document) {
        return Err(TreeError::DocumentParent(node_id));
    }
    tree.parse_html_with(Some(parent), markup, &mut set_style_text)
        .map(Some)
}

/// Parses `markup` for `insertAdjacentHTML` at `position` of `node_id`. Returns the node that
/// takes the nodes, the node they go before (the last place without one) and the parse result.
///
/// # Errors
///
/// `NodeNotFound` when `node_id` is gone, `NoParent` or `DocumentParent` for a sibling position
/// without an element parent, or the error of the parse.
pub fn parse_adjacent(
    tree: &mut VirtualTree,
    node_id: NodeId,
    position: Position,
    markup: &str,
) -> Result<(NodeId, Option<NodeId>, ParsedHtml), TreeError> {
    let node = tree.get(node_id).ok_or(TreeError::NodeNotFound(node_id))?;
    let sibling = matches!(position, Position::BeforeBegin | Position::AfterEnd);
    let container = if sibling {
        node.parent().ok_or(TreeError::NoParent(node_id))?
    } else {
        node_id
    };
    if sibling && tree.get(container).is_some_and(VirtualNode::is_document) {
        return Err(TreeError::DocumentParent(node_id));
    }
    let context = match tree.get(container) {
        Some(node) if node.is_html("html") => None,
        _ => Some(container),
    };
    let parsed = tree.parse_html_with(context, markup, &mut set_style_text)?;
    let reference = match position {
        Position::BeforeBegin => Some(node_id),
        Position::AfterBegin => tree.first_child(node_id),
        Position::BeforeEnd => None,
        Position::AfterEnd => tree.next_sibling(node_id),
    };
    Ok((container, reference, parsed))
}

/// Moves the nodes of the parse result into `container` before `reference`, or last without one,
/// then finishes the parse.
///
/// # Errors
///
/// The error of the insertion.
pub fn insert_parsed(
    state: &mut DocumentState,
    view: &dyn View,
    parsed: &ParsedHtml,
    container: NodeId,
    reference: Option<NodeId>,
) -> Result<(), TreeError> {
    let (_, left) = state
        .tree
        .insert_before(container, parsed.fragment, reference)?;
    settle(state, view, &left);
    finish(state, parsed);
    Ok(())
}

/// Removes every child of `container` and puts the parse result in. Returns the old children.
///
/// # Errors
///
/// The error of a removal or of the insertion.
pub fn replace_children(
    state: &mut DocumentState,
    view: &dyn View,
    container: NodeId,
    parsed: &ParsedHtml,
) -> Result<Vec<NodeId>, TreeError> {
    let old = children_of(&state.tree, container);
    for &child in &old {
        let left = state.tree.remove_child(container, child)?;
        settle(state, view, &left);
    }
    insert_parsed(state, view, parsed, container, None)?;
    Ok(old)
}

/// Replaces `node_id` with the parse result and finishes the parse. Returns `false` and changes nothing when the node has
/// no parent.
///
/// # Errors
///
/// The error of the replacement.
pub fn replace_with(
    state: &mut DocumentState,
    view: &dyn View,
    node_id: NodeId,
    parsed: &ParsedHtml,
) -> Result<bool, TreeError> {
    let tree = &mut state.tree;
    let Some(parent) = tree.get(node_id).and_then(VirtualNode::parent) else {
        return Ok(false);
    };
    let (_, left) = tree.replace_child(parent, parsed.fragment, node_id)?;
    settle(state, view, &left);
    finish(state, parsed);
    Ok(true)
}

/// The steps that follow a parse once its nodes are in place: prints the style warnings of the
/// parse, gives an `input` the typed text of its `value` attribute and frees the result fragment,
/// which is empty.
fn finish(state: &mut DocumentState, parsed: &ParsedHtml) {
    for (node_id, line) in &parsed.warnings {
        state.warnings.report(&state.tree, *node_id, line);
    }
    for node in &parsed.nodes {
        let typed = state
            .tree
            .get(node.id)
            .filter(|input| input.is_html("input"))
            .and_then(|input| input.attributes().get("value"))
            .and_then(AttributeValue::as_str)
            .map(str::to_owned);
        if let Some(typed) = typed {
            state.set_input_value(node.id, &typed);
        }
    }
    state.tree.destroy_node(parsed.fragment);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;
    use crate::tests::{RecordingView, rooted_tree};

    fn state_of(count: usize) -> (DocumentState, Vec<NodeId>) {
        let (tree, ids) = rooted_tree(count);
        let state = DocumentState {
            tree,
            ..DocumentState::default()
        };
        (state, ids)
    }

    #[rstest]
    #[case::lower("beforebegin", Some(Position::BeforeBegin))]
    #[case::mixed("AfterBegin", Some(Position::AfterBegin))]
    #[case::upper("BEFOREEND", Some(Position::BeforeEnd))]
    #[case::after_end("afterend", Some(Position::AfterEnd))]
    #[case::unknown("inside", None)]
    fn a_position_reads_ignoring_case(#[case] text: &str, #[case] expected: Option<Position>) {
        assert_eq!(Position::from_name(text), expected);
    }

    #[test]
    fn replace_children_settles_each_old_child_before_the_new_nodes() {
        let (mut state, ids) = state_of(2);
        let view = RecordingView::default();
        let (container, parsed) = parse_inner(&mut state.tree, 0, "<i></i>").unwrap();

        let old = replace_children(&mut state, &view, container, &parsed).unwrap();

        let resets = view.resets.borrow();
        assert_eq!(old, ids);
        assert!(resets.len() > 2);
        assert_eq!(resets[..2], ids);
    }

    #[test]
    fn replace_with_settles_the_node_and_the_moved_nodes() {
        let (mut state, ids) = state_of(1);
        let view = RecordingView::default();
        let parsed = parse_outer(&mut state.tree, ids[0], "<i></i>")
            .unwrap()
            .unwrap();

        assert!(replace_with(&mut state, &view, ids[0], &parsed).unwrap());

        let resets = view.resets.borrow();
        assert_eq!(resets[0], ids[0]);
        assert!(resets.len() > 1);
    }

    #[test]
    fn replace_with_a_parentless_node_changes_nothing() {
        let (mut state, _ids) = state_of(1);
        let view = RecordingView::default();
        let loose = state.tree.create_node("p").unwrap();
        let parsed = state.tree.parse_html(None, "<i></i>").unwrap();

        assert!(!replace_with(&mut state, &view, loose, &parsed).unwrap());

        assert!(view.resets.borrow().is_empty());
    }

    #[test]
    fn parse_outer_on_a_parentless_node_gives_none() {
        let (mut tree, _ids) = rooted_tree(0);
        let loose = tree.create_node("p").unwrap();

        assert_eq!(parse_outer(&mut tree, loose, "x").unwrap(), None);
    }

    #[test]
    fn parse_outer_under_a_document_is_refused() {
        let (mut tree, _ids) = rooted_tree(0);

        assert_eq!(
            parse_outer(&mut tree, 0, "x"),
            Err(TreeError::DocumentParent(0))
        );
    }

    /// `container` and `reference` index `[root, first, second]`, the nodes of `rooted_tree(2)`.
    #[rstest]
    #[case::before_begin(Position::BeforeBegin, 0, Some(1))]
    #[case::after_begin(Position::AfterBegin, 1, None)]
    #[case::before_end(Position::BeforeEnd, 1, None)]
    #[case::after_end(Position::AfterEnd, 0, Some(2))]
    fn parse_adjacent_gives_the_insertion_point_of_each_position(
        #[case] position: Position,
        #[case] container: usize,
        #[case] reference: Option<usize>,
    ) {
        let (mut tree, ids) = rooted_tree(2);
        let nodes = [0, ids[0], ids[1]];

        let (got_container, got_reference, _parsed) =
            parse_adjacent(&mut tree, ids[0], position, "x").unwrap();

        assert_eq!(
            (got_container, got_reference),
            (nodes[container], reference.map(|i| nodes[i]))
        );
    }

    #[rstest]
    #[case::before_begin(Position::BeforeBegin)]
    #[case::after_end(Position::AfterEnd)]
    fn a_sibling_position_without_a_parent_is_refused(#[case] position: Position) {
        let (mut tree, _ids) = rooted_tree(0);
        let loose = tree.create_node("p").unwrap();

        assert_eq!(
            parse_adjacent(&mut tree, loose, position, "x").map(|_| ()),
            Err(TreeError::NoParent(loose))
        );
    }

    #[test]
    fn a_parsed_style_attribute_applies_and_warns_once() {
        let (mut state, _ids) = state_of(0);
        let view = RecordingView::default();
        let lines = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = std::rc::Rc::clone(&lines);
        state.warnings.set_sink(std::rc::Rc::new(move |line| {
            sink.borrow_mut().push(line.to_owned());
        }));
        let (container, parsed) =
            parse_inner(&mut state.tree, 0, r#"<p style="bogus:1;width:2px"></p>"#).unwrap();

        replace_children(&mut state, &view, container, &parsed).unwrap();

        let id = state.tree.get(container).unwrap().children()[0];
        assert_eq!(
            state.tree.get(id).unwrap().style_props().get("width"),
            Some(&"2px".into())
        );
        assert_eq!(lines.borrow().len(), 1, "{:?}", lines.borrow());
    }

    #[test]
    fn a_parsed_element_has_its_style_props_before_it_is_inserted() {
        let (mut state, _ids) = state_of(0);
        let (_, parsed) = parse_inner(&mut state.tree, 0, r#"<p style="width:2px"></p>"#).unwrap();

        let props = state.tree.get(parsed.nodes[0].id).unwrap().style_props();

        assert_eq!(props.get("width"), Some(&"2px".into()));
    }

    #[test]
    fn a_dropped_element_warns_once_and_in_parse_order() {
        let (mut state, _ids) = state_of(0);
        let view = RecordingView::default();
        let lines = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = std::rc::Rc::clone(&lines);
        state.warnings.set_sink(std::rc::Rc::new(move |line| {
            sink.borrow_mut().push(line.to_owned());
        }));
        let html = state.tree.create_element_ns(None, None, "html").unwrap();
        // The frameset replaces the body, so the parser drops the paragraph before the result.
        let (container, parsed) = parse_inner(
            &mut state.tree,
            html,
            r#"<p style="bogus:1"></p><frameset style="bogus:2">"#,
        )
        .unwrap();

        replace_children(&mut state, &view, container, &parsed).unwrap();

        let lines = lines.borrow();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("(p)"), "{lines:?}");
        assert!(lines[1].contains("(frameset)"), "{lines:?}");
    }

    #[test]
    fn a_parsed_input_takes_the_value_attribute_as_typed_text() {
        let (mut state, _ids) = state_of(0);
        let view = RecordingView::default();
        let (container, parsed) = parse_inner(&mut state.tree, 0, "<input value=a>").unwrap();

        replace_children(&mut state, &view, container, &parsed).unwrap();

        let id = state.tree.get(container).unwrap().children()[0];
        assert!(state.inputs.borrow().contains_key(&id));
    }
}
