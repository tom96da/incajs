// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Matching one node against a selector, and `closest`.

use super::{
    Atom, AttrSelectorOperation, AttributeValue, BloomFilter, COMMENT_TAG, CaseSensitivity,
    Element, ElementSelectorFlags, FRAGMENT_TAG, Impl, MatchingContext, MatchingForInvalidation,
    MatchingMode, NamespaceConstraint, NeedsSelectorFlags, NodeId, OpaqueElement, PseudoClass,
    PseudoElement, QuirksMode, SelectorCaches, SelectorError, SelectorList, TEXT_TAG, VirtualNode,
    VirtualTree, fmt, is_disabled_control, js_number_text, matches_selector_list, parse,
};
use selectors::matching::matches_selector as matches_selector_one;

/// An element of the tree: a node that is neither text content, a comment nor a fragment.
#[derive(Clone)]
pub(super) struct Node<'t> {
    tree: &'t VirtualTree,
    pub(super) node: &'t VirtualNode,
}

impl fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node.id())
    }
}

fn is_element(node: &VirtualNode) -> bool {
    !matches!(node.tag_name(), COMMENT_TAG | FRAGMENT_TAG | TEXT_TAG)
}

/// The text of an attribute value: a string as is, a number through `js_number_text`, a bool as
/// `true` or `false`.
pub(crate) fn attribute_text(value: &AttributeValue) -> String {
    match value {
        AttributeValue::String(text) => text.clone(),
        AttributeValue::Number(number) => js_number_text(*number),
        AttributeValue::Bool(flag) => flag.to_string(),
    }
}

/// Whether the `"value"` of `node` is not empty.
fn has_text(node: &VirtualNode) -> bool {
    matches!(
        node.attributes().get("value"),
        Some(value) if !attribute_text(value).is_empty()
    )
}

impl<'t> Node<'t> {
    pub(super) fn of(tree: &'t VirtualTree, id: NodeId) -> Option<Self> {
        tree.get(id)
            .filter(|node| is_element(node))
            .map(|node| Node { tree, node })
    }

    /// The nearest element above `node`.
    fn nearest_above(tree: &'t VirtualTree, node: &VirtualNode) -> Option<Self> {
        let mut parent = node.parent();
        while let Some(id) = parent {
            if let Some(element) = Node::of(tree, id) {
                return Some(element);
            }
            parent = tree.get(id)?.parent();
        }
        None
    }

    /// An `input` that has neither `readonly` nor `disabled`.
    fn is_editable(&self) -> bool {
        self.node.tag_name() == "input"
            && !self.node.attributes().contains_key("readonly")
            && !self.node.attributes().contains_key("disabled")
    }

    fn attribute(&self, name: &str) -> Option<String> {
        self.node.attributes().get(name).map(attribute_text)
    }

    /// The element `step` places from this one among its parent's children, skipping nodes that
    /// are not elements.
    fn sibling(&self, step: isize) -> Option<Self> {
        let siblings = self.tree.get(self.node.parent()?)?.children();
        let at = siblings.iter().position(|&id| id == self.node.id())?;
        let mut index = at.checked_add_signed(step);
        while let Some(&id) = index.and_then(|i| siblings.get(i)) {
            if let Some(found) = Node::of(self.tree, id) {
                return Some(found);
            }
            index = index.and_then(|i| i.checked_add_signed(step));
        }
        None
    }
}

impl Element for Node<'_> {
    type Impl = Impl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(self.node)
    }

    fn parent_element(&self) -> Option<Self> {
        Node::nearest_above(self.tree, self.node)
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.sibling(-1)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.sibling(1)
    }

    fn first_element_child(&self) -> Option<Self> {
        self.node
            .children()
            .iter()
            .find_map(|&id| Node::of(self.tree, id))
    }

    fn is_html_element_in_html_document(&self) -> bool {
        true
    }

    fn has_local_name(&self, name: &Atom) -> bool {
        self.node.tag_name().eq_ignore_ascii_case(&name.0)
    }

    fn has_namespace(&self, _namespace: &Atom) -> bool {
        true
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.node.tag_name() == other.node.tag_name()
    }

    fn attr_matches(
        &self,
        _namespace: &NamespaceConstraint<&Atom>,
        local_name: &Atom,
        operation: &AttrSelectorOperation<&Atom>,
    ) -> bool {
        self.attribute(&local_name.0)
            .is_some_and(|value| operation.eval_str(&value))
    }

    fn match_non_ts_pseudo_class(
        &self,
        class: &PseudoClass,
        _context: &mut MatchingContext<Impl>,
    ) -> bool {
        let tag = self.node.tag_name();
        let control = matches!(tag, "button" | "input");
        match class {
            PseudoClass::Never(_) => false,
            PseudoClass::Defined | PseudoClass::DirLtr => true,
            PseudoClass::Disabled => is_disabled_control(self.node),
            PseudoClass::Enabled => control && !is_disabled_control(self.node),
            PseudoClass::ReadWrite => self.is_editable(),
            PseudoClass::ReadOnly => !self.is_editable(),
        }
    }

    fn match_pseudo_element(
        &self,
        _pseudo: &PseudoElement,
        _context: &mut MatchingContext<Impl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        false
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_id(&self, id: &Atom, case_sensitivity: CaseSensitivity) -> bool {
        self.attribute("id")
            .is_some_and(|value| case_sensitivity.eq(value.as_bytes(), id.0.as_bytes()))
    }

    fn has_class(&self, name: &Atom, case_sensitivity: CaseSensitivity) -> bool {
        self.attribute("class").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|class| case_sensitivity.eq(class.as_bytes(), name.0.as_bytes()))
        })
    }

    fn has_custom_state(&self, _name: &Atom) -> bool {
        false
    }

    fn imported_part(&self, _name: &Atom) -> Option<Atom> {
        None
    }

    fn is_part(&self, _name: &Atom) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        !self.node.children().iter().any(|&id| {
            self.tree.get(id).is_some_and(|child| {
                if is_element(child) {
                    true
                } else {
                    child.tag_name() == TEXT_TAG && has_text(child)
                }
            })
        })
    }

    fn is_root(&self) -> bool {
        match self.tree.root() {
            Some(root) => root == self.node.id(),
            None => self.node.parent().is_none(),
        }
    }

    fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
        false
    }
}

pub(super) fn matching_context(
    caches: &mut SelectorCaches,
    scope: OpaqueElement,
) -> MatchingContext<'_, Impl> {
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    context.scope_element = Some(scope);
    context
}

pub(super) fn matches_with(
    list: &SelectorList<Impl>,
    element: &Node<'_>,
    scope: OpaqueElement,
) -> bool {
    let mut caches = SelectorCaches::default();
    matches_selector_list(list, element, &mut matching_context(&mut caches, scope))
}

pub(super) fn specificity_of_match(
    list: &SelectorList<Impl>,
    tree: &VirtualTree,
    id: NodeId,
) -> Option<u32> {
    let element = Node::of(tree, id)?;
    let mut caches = SelectorCaches::default();
    let mut context = matching_context(&mut caches, element.opaque());
    list.slice()
        .iter()
        .filter(|selector| matches_selector_one(selector, 0, None, &element, &mut context))
        .map(selectors::parser::Selector::specificity)
        .max()
}

/// Whether the element `node` matches `selector`. A node that is not an element, such as a text
/// leaf or a node tagged `#comment`, and an unknown id match nothing.
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn matches_selector(
    tree: &VirtualTree,
    node: NodeId,
    selector: &str,
) -> Result<bool, SelectorError> {
    let list = parse(selector)?;
    Ok(Node::of(tree, node).is_some_and(|element| {
        let scope = element.opaque();
        matches_with(&list, &element, scope)
    }))
}

/// The nearest element at or above `node` that matches `selector`. A `#text` node starts at its
/// nearest element ancestor, and a node tagged `#comment` and an unknown id find nothing.
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn closest_selector(
    tree: &VirtualTree,
    node: NodeId,
    selector: &str,
) -> Result<Option<NodeId>, SelectorError> {
    let list = parse(selector)?;
    let Some(start) = tree.get(node) else {
        return Ok(None);
    };
    if start.tag_name() == COMMENT_TAG {
        return Ok(None);
    }
    let scope = OpaqueElement::new(start);
    let mut current = Node::of(tree, node).or_else(|| Node::nearest_above(tree, start));
    while let Some(element) = current {
        if matches_with(&list, &element, scope) {
            return Ok(Some(element.node.id()));
        }
        current = element.parent_element();
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use rstest::rstest;

    #[rstest]
    #[case::type_selector("p1", "p", true)]
    #[case::type_selector_miss("p1", "span", false)]
    #[case::universal("p1", "*", true)]
    #[case::id("main", "#main", true)]
    #[case::id_miss("main", "#p1", false)]
    #[case::class("main", ".card", true)]
    #[case::class_miss("main", ".other", false)]
    #[case::attribute_exists("main", "[data-k]", true)]
    #[case::attribute_equals("main", "[data-k=\"ab-cd ef\"]", true)]
    #[case::attribute_word("main", "[data-k~=ef]", true)]
    #[case::attribute_prefix_word("main", "[data-k|=ab]", true)]
    #[case::attribute_starts("main", "[data-k^=ab]", true)]
    #[case::attribute_ends("main", "[data-k$=ef]", true)]
    #[case::attribute_contains("main", "[data-k*=cd]", true)]
    #[case::attribute_miss("main", "[data-k$=ab]", false)]
    #[case::list("foot", "p, footer", true)]
    #[case::child("p1", "section > p", true)]
    #[case::child_miss("p1", "div > p", false)]
    #[case::descendant("p1", "div p", true)]
    #[case::descendant_miss("foot", "section footer", false)]
    #[case::adjacent("p2", "p + span", true)]
    #[case::adjacent_miss("p1", "span + p", false)]
    #[case::sibling("foot", "section ~ footer", true)]
    #[case::sibling_miss("main", "footer ~ section", false)]
    fn a_selector_form_matches_by_its_grammar(
        #[case] target: &str,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        let p = page();

        assert_eq!(p.f.matches(p.node(target), selector), expected);
    }

    #[test]
    fn closest_returns_the_node_itself_when_it_matches() {
        let p = page();

        assert_eq!(p.f.closest(p.p1, "p"), Some(p.p1));
    }

    #[test]
    fn closest_returns_the_nearest_of_two_matching_ancestors() {
        let mut f = Fixture::new();
        let outer = f.add(f.root, "div", "outer");
        let inner = f.add(outer, "div", "inner");
        let leaf = f.add(inner, "p", "leaf");

        assert_eq!(f.closest(leaf, "div"), Some(inner));
    }

    #[test]
    fn closest_returns_none_without_a_match() {
        let p = page();

        assert_eq!(p.f.closest(p.p1, ".missing"), None);
    }

    #[rstest]
    #[case::second_class(".b", true)]
    #[case::both_classes(".a.b", true)]
    #[case::substring(".ab", false)]
    fn a_class_selector_reads_the_class_list(#[case] selector: &str, #[case] expected: bool) {
        let mut p = page();
        p.f.attr(p.p1, "class", "a b");

        assert_eq!(p.f.matches(p.p1, selector), expected);
    }

    #[test]
    fn a_type_name_ignores_case() {
        let p = page();

        assert!(p.f.matches(p.f.root, "DIV"));
    }

    #[test]
    fn an_attribute_name_ignores_case() {
        let p = page();

        assert!(p.f.matches(p.main, "[DATA-K]"));
    }

    #[test]
    fn an_attribute_value_keeps_its_case() {
        let p = page();

        assert!(!p.f.matches(p.main, "[data-k=\"AB-CD EF\"]"));
    }

    #[test]
    fn the_type_attribute_value_ignores_case() {
        let mut p = page();
        p.f.attr(p.p1, "type", "Text");

        assert!(p.f.matches(p.p1, "[type=text]"));
    }

    #[test]
    fn an_ampersand_matches_the_called_node() {
        let p = page();

        assert!(p.f.matches(p.p1, "&"));
    }

    #[test]
    fn closest_with_an_ampersand_stops_at_the_called_node() {
        let p = page();

        assert_eq!(p.f.closest(p.p2, "&"), Some(p.p2));
    }

    #[rstest]
    #[case::first_of_its_type("p1", "p:first-of-type", true)]
    #[case::first_of_another_type("p2", "span:first-of-type", true)]
    #[case::second_of_its_type("p3", "p:first-of-type", false)]
    #[case::last_of_its_type("p3", "p:last-of-type", true)]
    fn of_type_pseudo_classes_compare_the_tag(
        #[case] target: &str,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        let p = page();

        assert_eq!(p.f.matches(p.node(target), selector), expected);
    }

    #[test]
    fn a_negated_interactive_state_matches() {
        let p = page();

        assert!(p.f.matches(p.p1, ":not(:hover)"));
    }

    #[rstest]
    #[case::is("p1", ":is(p, div)", true)]
    #[case::where_list("p1", ":where(span, p)", true)]
    #[case::not("p1", ":not(span)", true)]
    #[case::not_miss("p1", ":not(p)", false)]
    #[case::nth_child_of("p2", ":nth-child(1 of span)", true)]
    #[case::nth_child_of_miss("p2", ":nth-child(2 of span)", false)]
    fn a_functional_pseudo_class_matches_through_its_list(
        #[case] target: &str,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        let p = page();

        assert_eq!(p.f.matches(p.node(target), selector), expected);
    }

    #[rstest]
    #[case::first_child(":first-child", true, false)]
    #[case::nth_child(":nth-child(2)", false, true)]
    #[case::last_child(":last-child", false, true)]
    fn position_classes_skip_a_comment_and_a_text_node(
        #[case] selector: &str,
        #[case] a_matches: bool,
        #[case] b_matches: bool,
    ) {
        let m = mixed();

        assert_eq!(m.f.matches(m.a, selector), a_matches);
        assert_eq!(m.f.matches(m.b, selector), b_matches);
    }

    #[test]
    fn an_adjacent_combinator_skips_the_non_elements() {
        let m = mixed();

        assert!(m.f.matches(m.b, "span + span"));
    }

    #[test]
    fn empty_counts_a_non_empty_text_leaf() {
        let mut f = Fixture::new();
        let holder = f.add(f.root, "div", "holder");
        let leaf = f.add(holder, "#text", "l");
        f.attr(leaf, "value", "x");

        assert!(!f.matches(holder, ":empty"));
    }

    #[test]
    fn empty_ignores_a_comment_stand_in() {
        let mut f = Fixture::new();
        let holder = f.add(f.root, "div", "holder");
        f.add(holder, "#comment", "c");

        assert!(f.matches(holder, ":empty"));
    }

    #[test]
    fn empty_is_false_for_an_element_child() {
        let p = page();

        assert!(!p.f.matches(p.main, ":empty"));
    }

    #[test]
    fn empty_is_true_for_a_childless_element() {
        let p = page();

        assert!(p.f.matches(p.p1, ":empty"));
    }

    #[test]
    fn root_falls_back_to_the_parentless_node_without_a_declared_root() {
        let mut tree = VirtualTree::new();
        let top = tree.create_node("div").unwrap();
        let child = tree.create_node("p").unwrap();
        tree.append_child(top, child).unwrap();

        assert!(matches_selector(&tree, top, ":root").unwrap());
        assert!(!matches_selector(&tree, child, ":root").unwrap());
    }

    #[test]
    fn empty_ignores_a_text_leaf_with_no_value() {
        let mut f = Fixture::new();
        let holder = f.add(f.root, "div", "holder");
        let leaf = f.add(holder, "#text", "l");
        f.attr(leaf, "value", "");

        assert!(f.matches(holder, ":empty"));
    }

    #[rstest]
    #[case::the_root("root", true)]
    #[case::a_child("p1", false)]
    fn root_matches_the_tree_root_only(#[case] node: &str, #[case] expected: bool) {
        let p = page();

        assert_eq!(p.f.matches(p.node(node), ":root"), expected);
    }

    #[test]
    fn scope_matches_the_called_node() {
        let p = page();

        assert!(p.f.matches(p.p1, ":scope"));
    }

    #[rstest]
    #[case::the_called_node("span:scope", Some("p2"))]
    #[case::an_ancestor("section:scope", None)]
    fn closest_keeps_the_scope_across_the_ancestors(
        #[case] selector: &str,
        #[case] expected: Option<&str>,
    ) {
        let p = page();

        assert_eq!(
            p.f.closest(p.p2, selector),
            expected.map(|name| p.node(name))
        );
    }

    #[test]
    fn a_comment_stand_in_matches_nothing() {
        let m = mixed();

        assert!(!m.f.matches(m.comment, "*"));
    }

    #[test]
    fn a_comment_stand_in_finds_no_closest() {
        let m = mixed();

        assert_eq!(m.f.closest(m.comment, "*"), None);
    }

    #[rstest]
    #[case::number("n", AttributeValue::Number(3.0), "[n=\"3\"]", true)]
    #[case::big_number_as_javascript_prints_it(
        "n",
        AttributeValue::Number(1e21),
        "[n=\"1e+21\"]",
        true
    )]
    #[case::boolean_true("b", AttributeValue::Bool(true), "[b=\"true\"]", true)]
    #[case::boolean_false("b", AttributeValue::Bool(false), "[b=\"false\"]", true)]
    fn an_attribute_compares_its_printed_text(
        #[case] key: &str,
        #[case] value: AttributeValue,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        let mut p = page();
        p.f.attr(p.p1, key, value);

        assert_eq!(p.f.matches(p.p1, selector), expected);
    }

    /// `parent > [a, comment, b]` with a `c` under `b`.
    fn has_fixture() -> (Fixture, NodeId, NodeId) {
        let mut f = Fixture::new();
        let parent = f.add(f.root, "div", "parent");
        let a = f.add(parent, "span", "a");
        f.add(parent, "#comment", "comment");
        let b = f.add(parent, "b", "b");
        f.add(b, "c", "c");
        (f, parent, a)
    }

    #[rstest]
    #[case::child(":has(> span)", true)]
    #[case::descendant(":has(b c)", true)]
    #[case::deep(":has(c)", true)]
    fn has_looks_through_relative_selectors_below(#[case] selector: &str, #[case] expected: bool) {
        let (f, parent, _) = has_fixture();

        assert_eq!(f.matches(parent, selector), expected);
    }

    #[rstest]
    #[case::adjacent(":has(+ b)")]
    #[case::general_sibling(":has(~ b)")]
    fn has_looks_past_a_comment_stand_in_to_the_next_siblings(#[case] selector: &str) {
        let (f, _, a) = has_fixture();

        assert!(f.matches(a, selector));
    }

    #[rstest]
    #[case::defined("div", ":defined", true)]
    #[case::read_only_for_a_div("div", ":read-only", true)]
    #[case::read_write_for_an_input("input", ":read-write", true)]
    #[case::read_only_misses_an_editable_input("input", ":read-only", false)]
    fn an_evaluated_state_matches_by_the_element(
        #[case] tag: &str,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        let mut f = Fixture::new();
        let node = f.add(f.root, tag, "n");

        assert_eq!(f.matches(node, selector), expected);
    }

    #[rstest]
    #[case::readonly("readonly")]
    #[case::disabled("disabled")]
    fn an_input_with_a_blocking_attribute_is_read_only(#[case] attribute: &str) {
        let mut f = Fixture::new();
        let input = f.add(f.root, "input", "i");
        f.attr(input, attribute, "");

        assert!(f.matches(input, ":read-only"));
        assert!(!f.matches(input, ":read-write"));
    }

    #[rstest]
    #[case::disabled_button("button", true, ":disabled", true)]
    #[case::disabled_input("input", true, ":disabled", true)]
    #[case::enabled_button("button", false, ":enabled", true)]
    #[case::enabled_input("input", false, ":enabled", true)]
    #[case::disabled_is_not_enabled("button", true, ":enabled", false)]
    #[case::div_is_not_disabled("div", true, ":disabled", false)]
    #[case::div_is_not_enabled("div", false, ":enabled", false)]
    #[case::textarea_is_not_disabled("textarea", true, ":disabled", false)]
    #[case::textarea_is_not_enabled("textarea", false, ":enabled", false)]
    fn disabled_and_enabled_apply_to_buttons_and_inputs_only(
        #[case] tag: &str,
        #[case] disabled: bool,
        #[case] selector: &str,
        #[case] expected: bool,
    ) {
        let mut f = Fixture::new();
        let node = f.add(f.root, tag, "n");
        if disabled {
            f.attr(node, "disabled", "");
        }

        assert_eq!(f.matches(node, selector), expected);
    }

    #[test]
    fn a_text_node_is_no_subject_of_a_selector() {
        let m = mixed();

        assert!(!m.f.matches(m.text_node, "*"));
    }

    #[test]
    fn a_text_node_is_no_child_for_has() {
        let m = mixed();

        assert!(!m.f.matches(m.parent, "div:has(> :not(span))"));
    }

    #[test]
    fn closest_from_a_text_node_starts_at_its_nearest_element_ancestor() {
        let m = mixed();

        assert_eq!(m.f.closest(m.text_node, "div"), Some(m.parent));
        assert_eq!(m.f.closest(m.text_node, ":not(div)"), None);
    }

    #[test]
    fn closest_from_a_child_in_a_fragment_finds_no_fragment() {
        let mut f = crate::tests::Fixture::new();
        let fragment = f.tree.create_node(FRAGMENT_TAG).unwrap();
        f.attr(fragment, "id", "box");
        let inside = f.tree.create_node("span").unwrap();
        f.tree.append_child(fragment, inside).unwrap();

        assert_eq!(f.closest(inside, "#box"), None);
    }

    #[test]
    fn an_unknown_node_matches_nothing() {
        let p = page();

        assert!(!matches_selector(&p.f.tree, 9999, "*").unwrap());
        assert_eq!(closest_selector(&p.f.tree, 9999, "*").unwrap(), None);
    }
}
