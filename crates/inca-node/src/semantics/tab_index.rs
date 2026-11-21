// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `tabindex` of an element: the parsed attribute and the default of each element kind.

use crate::tree::{AttributeValue, NodeId, VirtualNode, VirtualTree, js_number_text};

/// Parses a `tabindex` value: optional leading ASCII whitespace, an optional
/// sign, then digits. Trailing characters are ignored. Returns `None` when the
/// digits are missing or exceed `i32`.
fn parse_tab_index(value: &str) -> Option<i32> {
    let value = value.trim_start_matches([' ', '\t', '\n', '\x0c', '\r']);
    let (negative, rest) = match value.as_bytes().first()? {
        b'-' => (true, &value[1..]),
        b'+' => (false, &value[1..]),
        _ => (false, value),
    };
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if digits == 0 {
        return None;
    }
    let magnitude: i64 = rest[..digits].parse().ok()?;
    i32::try_from(if negative { -magnitude } else { magnitude }).ok()
}

impl VirtualTree {
    /// The `tabindex` attribute of `node` parsed as an integer. `None` for a missing, unparsable
    /// or out of range value, a boolean, and a node the tree does not have.
    #[must_use]
    pub fn explicit_tab_index(&self, node: NodeId) -> Option<i32> {
        match self.get(node)?.attributes().get("tabindex")? {
            AttributeValue::String(value) => parse_tab_index(value),
            AttributeValue::Number(value) => parse_tab_index(&js_number_text(*value)),
            AttributeValue::Bool(_) => None,
        }
    }

    /// The reflected `tabIndex` of `node`: the parsed attribute, else 0 for the element kinds
    /// the HTML standard lists and -1 for the others. `None` for a node the tree does not have
    /// and a node that is no element.
    #[must_use]
    pub fn tab_index(&self, node: NodeId) -> Option<i32> {
        let element = self.get(node).filter(|element| element.is_element())?;
        Some(self.explicit_tab_index(node).unwrap_or_else(|| {
            if self.focusable_kind(node, element) {
                0
            } else {
                -1
            }
        }))
    }

    /// Whether the element kind of `element` has a default `tabIndex` of 0.
    fn focusable_kind(&self, node: NodeId, element: &VirtualNode) -> bool {
        const HTML: [&str; 9] = [
            "a", "area", "button", "frame", "iframe", "input", "object", "select", "textarea",
        ];
        element.in_html_namespace() && HTML.contains(&element.local_name())
            || element.is_svg("a")
            || element.is_mathml("a")
            || element.is_html("summary") && self.is_summary_of_details(node, element)
    }

    /// Whether `element` is the first `summary` child of an HTML `details`.
    fn is_summary_of_details(&self, node: NodeId, element: &VirtualNode) -> bool {
        let Some(parent) = element.parent().and_then(|id| self.get(id)) else {
            return false;
        };
        parent.is_html("details")
            && parent
                .children()
                .iter()
                .filter_map(|id| self.get(*id))
                .find(|child| child.is_html("summary"))
                .is_some_and(|first| first.id() == node)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;

    fn tree_with(namespace: Option<&str>, tag: &str) -> (VirtualTree, NodeId) {
        let mut tree = VirtualTree::new();
        let id = tree.create_element_ns(namespace, None, tag).unwrap();
        (tree, id)
    }

    const HTML: &str = "http://www.w3.org/1999/xhtml";
    const SVG: &str = "http://www.w3.org/2000/svg";
    const MATHML: &str = "http://www.w3.org/1998/Math/MathML";

    #[rstest]
    #[case(" 1.5", Some(1))]
    #[case("+1", Some(1))]
    #[case("-0", Some(0))]
    #[case("-1", Some(-1))]
    #[case("\t\n 7x", Some(7))]
    #[case("1e3", Some(1))]
    #[case("abc", None)]
    #[case("", None)]
    #[case("+", None)]
    #[case("-", None)]
    #[case(".5", None)]
    #[case("2147483648", None)]
    #[case("-2147483649", None)]
    #[case("99999999999", None)]
    #[case("2147483647", Some(i32::MAX))]
    #[case("-2147483648", Some(i32::MIN))]
    fn a_tab_index_text_parses_to_its_leading_integer(
        #[case] text: &str,
        #[case] want: Option<i32>,
    ) {
        assert_eq!(parse_tab_index(text), want);
    }

    #[rstest]
    #[case::a(HTML, "a", 0)]
    #[case::area(HTML, "area", 0)]
    #[case::button(HTML, "button", 0)]
    #[case::frame(HTML, "frame", 0)]
    #[case::iframe(HTML, "iframe", 0)]
    #[case::input(HTML, "input", 0)]
    #[case::object(HTML, "object", 0)]
    #[case::select(HTML, "select", 0)]
    #[case::textarea(HTML, "textarea", 0)]
    #[case::svg_a(SVG, "a", 0)]
    #[case::mathml_a(MATHML, "a", 0)]
    #[case::div(HTML, "div", -1)]
    #[case::video(HTML, "video", -1)]
    #[case::audio(HTML, "audio", -1)]
    #[case::svg_circle(SVG, "circle", -1)]
    #[case::svg_button(SVG, "button", -1)]
    #[case::mathml_mi(MATHML, "mi", -1)]
    fn the_default_tab_index_follows_the_element_kind(
        #[case] namespace: &str,
        #[case] tag: &str,
        #[case] want: i32,
    ) {
        let (tree, id) = tree_with(Some(namespace), tag);

        assert_eq!(tree.tab_index(id), Some(want));
    }

    #[test]
    fn an_element_of_no_namespace_reads_the_default_of_an_unlisted_kind() {
        let (tree, id) = tree_with(None, "button");

        assert_eq!(tree.tab_index(id), Some(-1));
    }

    #[rstest]
    #[case::first(&["summary"], 0, HTML, 0)]
    #[case::after_a_div(&["div", "summary"], 1, HTML, 0)]
    #[case::second(&["summary", "summary"], 1, HTML, -1)]
    #[case::of_an_svg_details(&["summary"], 0, SVG, -1)]
    fn a_summary_reads_0_as_the_first_summary_of_its_details(
        #[case] tags: &[&str],
        #[case] read: usize,
        #[case] details_namespace: &str,
        #[case] want: i32,
    ) {
        let mut tree = VirtualTree::new();
        let details = tree
            .create_element_ns(Some(details_namespace), None, "details")
            .unwrap();
        let children: Vec<NodeId> = tags
            .iter()
            .map(|tag| {
                let id = tree.create_element_ns(Some(HTML), None, *tag).unwrap();
                tree.append_child(details, id).unwrap();
                id
            })
            .collect();

        assert_eq!(tree.tab_index(children[read]), Some(want));
    }

    #[test]
    fn a_summary_outside_details_reads_minus_1() {
        let (mut tree, summary) = tree_with(Some(HTML), "summary");
        let div = tree.create_element_ns(Some(HTML), None, "div").unwrap();
        tree.append_child(div, summary).unwrap();

        assert_eq!(tree.tab_index(summary), Some(-1));
    }

    #[rstest]
    #[case::on_a_button("button", "-1", -1)]
    #[case::on_a_div("div", "5", 5)]
    #[case::zero_on_a_div("div", "0", 0)]
    fn an_explicit_value_wins_over_the_default(
        #[case] tag: &str,
        #[case] text: &str,
        #[case] want: i32,
    ) {
        let (mut tree, id) = tree_with(Some(HTML), tag);
        tree.set_attribute(id, "tabindex", text).unwrap();

        assert_eq!(tree.tab_index(id), Some(want));
    }

    #[rstest]
    #[case::out_of_range("2147483648")]
    #[case::unparsable("abc")]
    fn an_unusable_value_reads_the_default_of_the_kind(#[case] text: &str) {
        let (mut tree, id) = tree_with(Some(HTML), "button");
        tree.set_attribute(id, "tabindex", text).unwrap();

        assert_eq!(tree.tab_index(id), Some(0));
    }

    #[test]
    fn a_node_that_is_no_element_has_no_tab_index() {
        let mut tree = VirtualTree::new();
        let text = tree.create_node("#text").unwrap();

        assert_eq!(tree.tab_index(text), None);
        assert_eq!(tree.tab_index(999), None);
    }

    #[rstest]
    #[case::string(AttributeValue::from("3"), Some(3))]
    #[case::number(AttributeValue::Number(0.0), Some(0))]
    #[case::negative_number(AttributeValue::Number(-1.0), Some(-1))]
    #[case::fraction(AttributeValue::Number(1.5), Some(1))]
    #[case::large_number(AttributeValue::Number(1e21), Some(1))]
    #[case::small_number(AttributeValue::Number(2.5e-7), Some(2))]
    #[case::nan(AttributeValue::Number(f64::NAN), None)]
    #[case::bool_true(AttributeValue::Bool(true), None)]
    #[case::bool_false(AttributeValue::Bool(false), None)]
    fn the_explicit_tab_index_reads_a_string_a_number_and_a_boolean(
        #[case] value: AttributeValue,
        #[case] want: Option<i32>,
    ) {
        let (mut tree, id) = tree_with(Some(HTML), "div");
        tree.set_attribute(id, "tabindex", value).unwrap();

        assert_eq!(tree.explicit_tab_index(id), want);
    }
}
