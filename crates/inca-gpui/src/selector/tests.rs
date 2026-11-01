// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::*;

mod invalid;

/// A tree under construction. Every node an `add` call makes gets an `id` attribute of `name`.
struct Fixture {
    tree: VirtualTree,
    root: NodeId,
}

impl Fixture {
    fn new() -> Self {
        let mut tree = VirtualTree::new();
        let root = tree.create_node("div").unwrap();
        tree.set_root(root);
        tree.set_attribute(root, "id", "root").unwrap();
        Fixture { tree, root }
    }

    fn add(&mut self, parent: NodeId, tag: &str, name: &str) -> NodeId {
        let node = self.tree.create_node(tag).unwrap();
        self.tree.append_child(parent, node).unwrap();
        self.tree.set_attribute(node, "id", name).unwrap();
        node
    }

    fn attr(&mut self, node: NodeId, key: &str, value: impl Into<AttributeValue>) {
        self.tree.set_attribute(node, key, value).unwrap();
    }

    fn matches(&self, node: NodeId, selector: &str) -> bool {
        matches_selector(&self.tree, node, selector).unwrap()
    }

    fn closest(&self, node: NodeId, selector: &str) -> Option<NodeId> {
        closest_selector(&self.tree, node, selector).unwrap()
    }
}

/// `root > main > (p1, p2)` and `root > foot`, with the attributes the selector rows read.
struct Page {
    f: Fixture,
    main: NodeId,
    p1: NodeId,
    p2: NodeId,
    p3: NodeId,
    foot: NodeId,
}

fn page() -> Page {
    let mut f = Fixture::new();
    let main = f.add(f.root, "section", "main");
    f.attr(main, "class", "card");
    f.attr(main, "data-k", "ab-cd ef");
    let p1 = f.add(main, "p", "p1");
    let p2 = f.add(main, "span", "p2");
    let p3 = f.add(main, "p", "p3");
    let foot = f.add(f.root, "footer", "foot");
    Page {
        f,
        main,
        p1,
        p2,
        p3,
        foot,
    }
}

impl Page {
    fn node(&self, name: &str) -> NodeId {
        match name {
            "root" => self.f.root,
            "main" => self.main,
            "p1" => self.p1,
            "p2" => self.p2,
            "p3" => self.p3,
            _ => self.foot,
        }
    }
}

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

#[rstest]
#[case::only_unknown_names(":is(:foo)", false)]
#[case::an_unknown_name_beside_a_match(":is(:foo, p)", true)]
fn an_is_list_drops_an_unknown_name(#[case] selector: &str, #[case] expected: bool) {
    let p = page();

    assert_eq!(p.f.matches(p.p1, selector), expected);
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
#[case::host(":host")]
#[case::host_function(":host(a)")]
#[case::host_context(":host-context(x)")]
#[case::host_context_compound(":host-context(.a#b)")]
#[case::host_context_padded(":host-context( x )")]
#[case::host_context_universal(":host-context(*)")]
#[case::host_context_attribute(":host-context([x])")]
#[case::host_context_pseudo_class(":host-context(a:hover)")]
#[case::host_context_negation(":host-context(:not(x))")]
#[case::lang(":lang(en)")]
#[case::lang_list(":lang(en, fr)")]
#[case::lang_strings(":lang(\"fr\", \"de\")")]
#[case::lang_mixed(":lang(en, \"fr\")")]
#[case::dir_rtl(":dir(rtl)")]
#[case::upper_case_pseudo_class(":HOVER")]
#[case::upper_case_pseudo_element("::BEFORE")]
#[case::upper_case_functional_pseudo_element("::HIGHLIGHT(x)")]
#[case::state(":state(x)")]
#[case::webkit_autofill(":-webkit-autofill")]
#[case::webkit_any_link(":-webkit-any-link")]
#[case::webkit_full_screen(":-webkit-full-screen")]
#[case::picture_in_picture(":picture-in-picture")]
#[case::file_selector_button("::file-selector-button")]
#[case::spelling_error("::spelling-error")]
#[case::grammar_error("::grammar-error")]
#[case::target_text("::target-text")]
#[case::cue("::cue")]
#[case::view_transition("::view-transition")]
#[case::hover(":hover")]
#[case::focus(":focus")]
#[case::checked(":checked")]
#[case::open(":open")]
#[case::before("::before")]
#[case::slotted("::slotted(x)")]
#[case::part("::part(x)")]
#[case::highlight("::highlight(x)")]
#[case::webkit("::-webkit-scrollbar")]
fn an_accepted_name_the_host_cannot_evaluate_never_matches(#[case] selector: &str) {
    let p = page();

    assert!(!p.f.matches(p.p1, selector));
}

#[rstest]
#[case::lower_case(":dir(ltr)")]
#[case::upper_case(":DIR(LTR)")]
fn dir_ltr_matches_every_element(#[case] selector: &str) {
    let p = page();

    assert!(p.f.matches(p.p1, selector));
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

#[rstest]
#[case::two_members(":-webkit-any(p, span)", true)]
#[case::no_member(":-webkit-any(span)", false)]
fn webkit_any_matches_through_its_list(#[case] selector: &str, #[case] expected: bool) {
    let p = page();

    assert_eq!(p.f.matches(p.p1, selector), expected);
}

#[rstest]
#[case::unclosed_attribute("[data-k")]
#[case::unclosed_function(":is(p")]
fn the_end_of_the_input_closes_an_open_block(#[case] selector: &str) {
    let mut p = page();
    p.f.attr(p.p1, "data-k", "");

    assert!(p.f.matches(p.p1, selector));
}

#[test]
fn any_namespace_matches_by_the_type_name() {
    let p = page();

    assert!(p.f.matches(p.f.root, "*|div"));
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

/// `parent > [comment, text node, a, b]`.
struct Mixed {
    f: Fixture,
    parent: NodeId,
    comment: NodeId,
    a: NodeId,
    text_node: NodeId,
    b: NodeId,
}

fn mixed() -> Mixed {
    let mut f = Fixture::new();
    let parent = f.add(f.root, "div", "parent");
    let comment = f.add(parent, "#comment", "comment");
    let text_node = f.add(parent, "#text", "te");
    f.attr(text_node, "value", "t");
    let a = f.add(parent, "span", "a");
    let b = f.add(parent, "span", "b");
    Mixed {
        f,
        parent,
        comment,
        a,
        text_node,
        b,
    }
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
fn a_text_node_is_no_ancestor_of_a_selector() {
    let mut m = mixed();
    let inside = m.f.add(m.text_node, "span", "inside");

    assert!(!m.f.matches(inside, ":not(div) > span"));
    assert!(m.f.matches(inside, "div > span"));
}

#[test]
fn closest_from_a_text_node_starts_at_its_nearest_element_ancestor() {
    let m = mixed();

    assert_eq!(m.f.closest(m.text_node, "div"), Some(m.parent));
    assert_eq!(m.f.closest(m.text_node, ":not(div)"), None);
}

#[test]
fn an_unknown_node_matches_nothing() {
    let p = page();

    assert!(!matches_selector(&p.f.tree, 9999, "*").unwrap());
    assert_eq!(closest_selector(&p.f.tree, 9999, "*").unwrap(), None);
}

/// `root > (a > (a1, a2), b)`, created in the order a, a1, b, a2, so that the creation order,
/// the level order and the document order differ.
fn forest() -> (Fixture, [NodeId; 4]) {
    let mut f = Fixture::new();
    let a = f.add(f.root, "div", "a");
    let a1 = f.add(a, "div", "a1");
    let b = f.add(f.root, "div", "b");
    let a2 = f.add(a, "div", "a2");
    (f, [a, a1, a2, b])
}

#[test]
fn query_selector_all_lists_matches_in_document_order() {
    let (f, [a, a1, a2, b]) = forest();

    assert_eq!(
        query_selector_all(&f.tree, f.root, "*").unwrap(),
        [a, a1, a2, b]
    );
}

#[test]
fn a_selector_list_yields_one_ordered_sequence_without_duplicates() {
    let p = page();

    assert_eq!(
        query_selector_all(&p.f.tree, p.f.root, "span, p, #p1").unwrap(),
        [p.p1, p.p2, p.p3]
    );
}

#[test]
fn query_selector_returns_the_first_match_in_document_order() {
    let p = page();

    assert_eq!(
        query_selector(&p.f.tree, p.f.root, "p").unwrap(),
        Some(p.p1)
    );
}

#[test]
fn query_selector_returns_none_without_a_match() {
    let p = page();

    assert_eq!(
        query_selector(&p.f.tree, p.f.root, "article").unwrap(),
        None
    );
}

#[test]
fn a_matching_start_node_is_not_in_its_own_result() {
    let p = page();

    assert_eq!(
        query_selector_all(&p.f.tree, p.main, "section, p").unwrap(),
        [p.p1, p.p3]
    );
}

#[rstest]
#[case::scope_child(":scope > p", &["p1", "p3"])]
#[case::ampersand_child("& > p", &["p1", "p3"])]
#[case::scope_alone(":scope", &[])]
#[case::ampersand_alone("&", &[])]
#[case::negated_scope(":not(:scope)", &["p1", "p2", "p3"])]
fn scope_selects_the_start_node(#[case] selector: &str, #[case] expected: &[&str]) {
    let p = page();

    assert_eq!(
        query_selector_all(&p.f.tree, p.main, selector).unwrap(),
        expected.iter().map(|name| p.node(name)).collect::<Vec<_>>()
    );
}

#[test]
fn ancestors_outside_the_start_node_take_part_in_the_match() {
    let p = page();

    assert_eq!(
        query_selector_all(&p.f.tree, p.main, "div p").unwrap(),
        [p.p1, p.p3]
    );
}

#[test]
fn text_leaves_and_comment_stand_ins_are_not_results() {
    let m = mixed();

    assert_eq!(
        query_selector_all(&m.f.tree, m.parent, "*").unwrap(),
        [m.a, m.b]
    );
}

#[rstest]
#[case::unknown_id(9999)]
#[case::text_leaf(mixed().text_node)]
#[case::comment_stand_in(mixed().comment)]
fn an_unknown_text_or_comment_start_gives_an_empty_result(#[case] start: NodeId) {
    let m = mixed();

    assert!(
        query_selector_all(&m.f.tree, start, "*")
            .unwrap()
            .is_empty()
    );
    assert_eq!(query_selector(&m.f.tree, start, "*").unwrap(), None);
}

#[test]
fn a_parentless_node_searches_its_own_subtree() {
    let mut tree = VirtualTree::new();
    let top = tree.create_node("div").unwrap();
    let child = tree.create_node("p").unwrap();
    tree.append_child(top, child).unwrap();

    assert_eq!(query_selector_all(&tree, top, "p").unwrap(), [child]);
}

#[test]
fn query_selector_all_walks_a_deep_chain_without_overflow() {
    let mut tree = VirtualTree::new();
    let mut top = tree.create_node("div").unwrap();
    for _ in 1..100_000 {
        let parent = tree.create_node("div").unwrap();
        tree.append_child(parent, top).unwrap();
        top = parent;
    }

    assert_eq!(query_selector_all(&tree, top, "*").unwrap().len(), 99_999);
}

#[rstest]
#[case::negative_zero(-0.0, "0")]
#[case::infinity(f64::INFINITY, "Infinity")]
#[case::negative_infinity(f64::NEG_INFINITY, "-Infinity")]
#[case::nan(f64::NAN, "NaN")]
#[case::large(1e21, "1e+21")]
#[case::small(1e-7, "1e-7")]
#[case::integer(42.0, "42")]
#[case::negative_fraction(-1.5, "-1.5")]
#[case::below_the_exponent_form(1e20, "100000000000000000000")]
#[case::decimal_zeros(0.000_001, "0.000001")]
#[case::exponent_with_digits(1.5e-7, "1.5e-7")]
#[case::fraction(0.5, "0.5")]
fn js_number_text_prints_a_number_as_javascript_does(#[case] value: f64, #[case] expected: &str) {
    assert_eq!(js_number_text(value), expected);
}
