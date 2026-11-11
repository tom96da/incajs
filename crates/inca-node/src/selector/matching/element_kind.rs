// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a `Node` answers by the kind of element it is: the attribute lookup and the local-name
//! and same-type checks by namespace, and the form states `:disabled`, `:enabled`, `:read-write`
//! and `:read-only`. The tests of these sit in the same file.

use super::{Node, PseudoClass};
use crate::tree::{AttributeValue, is_disabled_control};

impl Node<'_> {
    /// An HTML `input` that has neither `readonly` nor `disabled`.
    fn is_editable(&self) -> bool {
        self.node.is_html("input")
            && !self.node.attributes().contains_key("readonly")
            && !self.node.attributes().contains_key("disabled")
    }

    /// The value of the attribute `name`. An element outside the HTML namespace finds the key
    /// without ASCII case.
    pub(super) fn attribute(&self, name: &str) -> Option<String> {
        let attributes = self.node.attributes();
        let value = if self.node.in_html_namespace() {
            attributes.get(name)
        } else {
            attributes
                .iter()
                .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value))
        };
        value.map(AttributeValue::text)
    }

    /// Whether the local name of the element is `name`. An element outside the HTML namespace
    /// compares without ASCII case.
    pub(super) fn has_name(&self, name: &str) -> bool {
        if self.node.in_html_namespace() {
            self.node.tag_name() == name
        } else {
            self.node.tag_name().eq_ignore_ascii_case(name)
        }
    }

    /// Whether `other` has the same namespace and local name.
    pub(super) fn is_same_type_as(&self, other: &Self) -> bool {
        self.node.namespace_uri() == other.node.namespace_uri()
            && self.node.tag_name() == other.node.tag_name()
    }

    /// Whether the element is in the form state `class`. Any other pseudo-class is false.
    pub(super) fn is_in_state(&self, class: &PseudoClass) -> bool {
        match class {
            PseudoClass::Disabled => is_disabled_control(self.node),
            PseudoClass::Enabled => {
                self.node.is_focusable_by_default() && !is_disabled_control(self.node)
            }
            PseudoClass::ReadWrite => self.is_editable(),
            PseudoClass::ReadOnly => self.node.in_html_namespace() && !self.is_editable(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::tests::*;
    use crate::tree::NodeId;
    use rstest::rstest;

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

    /// An element of `namespace` named `local_name` under the root.
    fn add_ns(f: &mut Fixture, namespace: &str, local_name: &str) -> NodeId {
        let id = f
            .tree
            .create_element_ns(Some(namespace), None, local_name)
            .unwrap();
        f.tree.append_child(f.root, id).unwrap();
        id
    }

    #[rstest]
    #[case::type_name("foreignobject", "foreignObject")]
    #[case::attribute_name("[viewbox]", "svg")]
    fn a_name_in_another_namespace_ignores_case(#[case] selector: &str, #[case] local: &str) {
        let mut f = Fixture::new();
        let node = add_ns(&mut f, SVG, local);
        f.tree.set_attribute(node, "viewBox", "0 0 1 1").unwrap();

        assert!(f.matches(node, selector));
    }

    #[test]
    fn an_html_name_with_another_case_matches_no_type_selector() {
        let mut f = Fixture::new();
        let node = add_ns(&mut f, HTML, "DIV");

        assert!(!f.matches(node, "div"));
    }

    #[test]
    fn an_html_attribute_name_is_compared_exactly() {
        let mut f = Fixture::new();
        let node = add_ns(&mut f, HTML, "div");
        f.tree.set_attribute(node, "viewBox", "1").unwrap();

        assert!(!f.matches(node, "[viewbox]"));
    }

    #[test]
    fn an_html_a_and_an_svg_a_are_of_different_types() {
        let mut f = Fixture::new();
        let html = add_ns(&mut f, HTML, "a");
        let svg = add_ns(&mut f, SVG, "a");

        assert!(!f.matches(html, "a:nth-of-type(2)"));
        assert!(!f.matches(svg, "a:nth-of-type(2)"));
    }

    #[rstest]
    #[case::enabled("button", ":enabled")]
    #[case::read_write("input", ":read-write")]
    #[case::read_only_input("input", ":read-only")]
    #[case::read_only_a("a", ":read-only")]
    fn an_svg_element_matches_no_form_state(#[case] local: &str, #[case] selector: &str) {
        let mut f = Fixture::new();
        let node = add_ns(&mut f, SVG, local);

        assert!(!f.matches(node, selector));
    }

    #[test]
    fn an_svg_input_with_a_disabled_attribute_is_not_disabled() {
        let mut f = Fixture::new();
        let node = add_ns(&mut f, SVG, "input");
        f.tree.set_attribute(node, "disabled", "").unwrap();

        assert!(!f.matches(node, ":disabled"));
    }
}
