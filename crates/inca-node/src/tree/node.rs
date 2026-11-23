// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A node of the tree and the values stored on it.

use std::collections::HashMap;

use html5ever::Namespace;
use indexmap::IndexMap;

use super::NodeKind;

/// Stable handle of a node, used in every later call on it.
pub type NodeId = u32;

/// An owned, primitive value for a style/attribute prop.
///
/// Restricted to primitives on purpose: a non-primitive value (an object or
/// array) can't be represented here, so a caller converting a value into
/// one of these must reject it explicitly. Nothing coerces or
/// stores it silently.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    String(String),
    Number(f64),
    Bool(bool),
}

impl AttributeValue {
    /// The text of a `String` value, `None` for any other kind.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The text of the value: a string as is, a number through [`shortest_number_text`], a bool as
    /// `true` or `false`.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::String(text) => text.clone(),
            Self::Number(number) => shortest_number_text(*number),
            Self::Bool(flag) => flag.to_string(),
        }
    }

    /// The number of a `Number` value, `None` for any other kind.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl From<&str> for AttributeValue {
    fn from(value: &str) -> Self {
        AttributeValue::String(value.to_owned())
    }
}

impl From<String> for AttributeValue {
    fn from(value: String) -> Self {
        AttributeValue::String(value)
    }
}

impl From<f64> for AttributeValue {
    fn from(value: f64) -> Self {
        AttributeValue::Number(value)
    }
}

impl From<bool> for AttributeValue {
    fn from(value: bool) -> Self {
        AttributeValue::Bool(value)
    }
}

/// The shortest text that reads back as the number. `-0` prints as `0`, and values from `1e21` up
/// or below `1e-6` use the exponent form.
#[must_use]
pub fn shortest_number_text(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-Infinity" } else { "Infinity" }.to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    // `{:e}` prints the shortest digits that read back, as `d.ddde<exp>`.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    // The value is 0.<digits> * 10^point.
    let point = exponent + 1;
    let count = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    let body = if count <= point && point <= 21 {
        let zeros = usize::try_from(point - count).unwrap_or(0);
        format!("{digits}{}", "0".repeat(zeros))
    } else if 0 < point && point <= 21 {
        let at = usize::try_from(point).unwrap_or(0);
        format!("{}.{}", &digits[..at], &digits[at..])
    } else if -6 < point && point <= 0 {
        let zeros = usize::try_from(-point).unwrap_or(0);
        format!("0.{}{digits}", "0".repeat(zeros))
    } else {
        let power = point - 1;
        let exponent_sign = if power < 0 { "-" } else { "+" };
        let fraction = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        format!("{}{fraction}e{exponent_sign}{}", &digits[..1], power.abs())
    };
    format!("{sign}{body}")
}

/// Whether `node` is an HTML `button` or `input` carrying the `disabled`
/// attribute.
#[must_use]
pub fn is_disabled_control(node: &VirtualNode) -> bool {
    node.is_form_control() && node.attributes().contains_key("disabled")
}

/// The inline style props of a node, in the order they were set. Setting a key that exists keeps
/// its position, and a removed key that is set again goes last.
pub type StyleMap = IndexMap<String, AttributeValue>;

/// A single retained node. Always accessed through a [`VirtualTree`](super::VirtualTree). There
/// is no way to construct one standalone, since its [`id`](VirtualNode::id)
/// is only meaningful within the tree that allocated it.
#[derive(Debug, Clone)]
pub struct VirtualNode {
    pub(super) id: NodeId,
    pub(super) kind: NodeKind,
    pub(super) local_name: String,
    /// The character data of a text node or a comment, empty for any other node.
    pub(super) data: String,
    /// The namespace of an element. `None` for a node that is no element and for an element made
    /// without one.
    pub(super) namespace: Option<Namespace>,
    pub(super) prefix: Option<String>,
    /// The style props of the node.
    pub style_props: StyleMap,
    /// The attributes in insertion order.
    pub attributes: IndexMap<String, AttributeValue>,
    pub(super) parent: Option<NodeId>,
    pub(super) children: Vec<NodeId>,
    /// The fragment a template owns.
    pub(super) content: Option<NodeId>,
    /// The template that owns this fragment.
    pub(super) owner: Option<NodeId>,
    /// The elements an ARIA attribute refers to by element reference, keyed by the attribute name.
    pub(crate) aria_refs: HashMap<&'static str, Vec<NodeId>>,
}

impl VirtualNode {
    pub(super) fn new(
        id: NodeId,
        kind: NodeKind,
        local_name: String,
        namespace: Option<Namespace>,
        prefix: Option<String>,
    ) -> Self {
        Self {
            id,
            kind,
            local_name,
            data: String::new(),
            namespace,
            prefix,
            style_props: StyleMap::new(),
            attributes: IndexMap::new(),
            parent: None,
            children: Vec::new(),
            content: None,
            owner: None,
            aria_refs: HashMap::new(),
        }
    }

    /// The elements the ARIA attribute `attribute` refers to by element reference.
    #[must_use]
    pub fn aria_refs(&self, attribute: &str) -> Option<&[NodeId]> {
        self.aria_refs.get(attribute).map(Vec::as_slice)
    }

    /// Keeps `ids` as the elements the ARIA attribute `attribute` refers to by element reference.
    pub fn set_aria_refs(&mut self, attribute: &'static str, ids: Vec<NodeId>) {
        self.aria_refs.insert(attribute, ids);
    }

    #[must_use]
    pub fn id(&self) -> NodeId {
        self.id
    }

    #[must_use]
    pub fn local_name(&self) -> &str {
        &self.local_name
    }

    #[must_use]
    pub fn style_props(&self) -> &StyleMap {
        &self.style_props
    }

    #[must_use]
    pub fn attributes(&self) -> &IndexMap<String, AttributeValue> {
        &self.attributes
    }

    /// The node this one is attached to, or `None` while it is detached.
    #[must_use]
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// Ordered child handles. Order is append order, not insertion-sorted.
    #[must_use]
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tree::VirtualTree;
    use rstest::rstest;

    #[rstest]
    #[case::button("button", &["disabled"], true)]
    #[case::input("input", &["disabled"], true)]
    #[case::div("div", &["disabled"], false)]
    #[case::enabled_button("button", &[], false)]
    #[case::link_with_href("a", &["href", "disabled"], false)]
    fn is_disabled_control_needs_a_control_tag_and_the_disabled_attribute(
        #[case] tag: &str,
        #[case] attributes: &[&str],
        #[case] want: bool,
    ) {
        let mut tree = VirtualTree::new();
        let node = tree.create_node(tag).unwrap();
        for attribute in attributes {
            tree.set_attribute(node, *attribute, true).unwrap();
        }
        assert_eq!(is_disabled_control(tree.get(node).unwrap()), want, "{tag}");
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
    fn shortest_number_text_prints_the_shortest_form(#[case] value: f64, #[case] expected: &str) {
        assert_eq!(shortest_number_text(value), expected);
    }
}
