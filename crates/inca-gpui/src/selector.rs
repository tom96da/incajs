// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! CSS selector matching over the retained tree, for `matches()`, `closest()` and the two query
//! methods.

use std::fmt;

use cssparser::{CowRcStr, ParseError, ParseErrorKind, Parser as CssParser, ToCss, Token};
use precomputed_hash::PrecomputedHash;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::bloom::BloomFilter;
use selectors::context::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches,
};
use selectors::matching::{ElementSelectorFlags, matches_selector_list};
use selectors::parser::{
    NonTSPseudoClass, ParseRelative, Parser, SelectorImpl, SelectorList, SelectorParseErrorKind,
};
use selectors::{Element, OpaqueElement};

use crate::tree::{
    AttributeValue, COMMENT_TAG, NodeId, TEXT_TAG, VirtualNode, VirtualTree, is_disabled_control,
};

/// The text JavaScript makes of a number. `-0` prints as `0`, and values from `1e21` up or below
/// `1e-6` use the exponent form.
#[must_use]
pub(crate) fn js_number_text(value: f64) -> String {
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

/// A selector that failed to parse, with a short reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorError {
    reason: &'static str,
}

impl fmt::Display for SelectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the selector is not valid. {}", self.reason)
    }
}

impl std::error::Error for SelectorError {}

/// One string type for every name the selector crate asks for.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
struct Atom(String);

impl ToCss for Atom {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        dest.write_str(&self.0)
    }
}

impl From<&str> for Atom {
    fn from(text: &str) -> Self {
        Atom(text.to_owned())
    }
}

impl AsRef<str> for Atom {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Bloom filters are off, so a constant hash is enough.
impl PrecomputedHash for Atom {
    fn precomputed_hash(&self) -> u32 {
        0
    }
}

/// The names a selector accepts without a way to evaluate them.
const NEVER_MATCHING_PSEUDO_CLASSES: &[&str] = &[
    "hover",
    "active",
    "focus",
    "focus-visible",
    "focus-within",
    "target",
    "visited",
    "link",
    "any-link",
    "checked",
    "indeterminate",
    "required",
    "optional",
    "valid",
    "invalid",
    "in-range",
    "out-of-range",
    "placeholder-shown",
    "default",
    "fullscreen",
    "modal",
    "popover-open",
    "open",
    "autofill",
    "user-valid",
    "user-invalid",
    "-webkit-autofill",
    "-webkit-any-link",
    "-webkit-full-screen",
    "picture-in-picture",
];

const PSEUDO_ELEMENTS: &[&str] = &[
    "before",
    "after",
    "first-line",
    "first-letter",
    "placeholder",
    "selection",
    "marker",
    "backdrop",
    "file-selector-button",
    "spelling-error",
    "grammar-error",
    "target-text",
    "cue",
    "view-transition",
];

#[derive(Clone, Debug, Eq, PartialEq)]
enum PseudoClass {
    /// A name the host accepts and cannot evaluate.
    Never(String),
    Defined,
    /// `:dir(ltr)`: every element has the directionality `ltr`.
    DirLtr,
    ReadOnly,
    ReadWrite,
    Disabled,
    Enabled,
}

impl ToCss for PseudoClass {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        let name = match self {
            PseudoClass::Never(name) => name,
            PseudoClass::Defined => "defined",
            PseudoClass::DirLtr => "dir(ltr)",
            PseudoClass::ReadOnly => "read-only",
            PseudoClass::ReadWrite => "read-write",
            PseudoClass::Disabled => "disabled",
            PseudoClass::Enabled => "enabled",
        };
        write!(dest, ":{name}")
    }
}

impl NonTSPseudoClass for PseudoClass {
    // The bloom filter and the hover quirk are off.
    fn is_active_or_hover(&self) -> bool {
        false
    }

    // No pseudo-class follows a pseudo-element.
    fn is_user_action_state(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PseudoElement(String);

impl ToCss for PseudoElement {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        write!(dest, "::{}", self.0)
    }
}

impl selectors::parser::PseudoElement for PseudoElement {}

#[derive(Clone, Debug, PartialEq)]
struct Impl;

impl SelectorImpl for Impl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = Atom;
    type Identifier = Atom;
    type LocalName = Atom;
    type NamespaceUrl = Atom;
    type NamespacePrefix = Atom;
    type BorrowedLocalName = Atom;
    type BorrowedNamespaceUrl = Atom;
    type NonTSPseudoClass = PseudoClass;
    type PseudoElement = PseudoElement;
}

struct SelectorParser;

/// Why a selector failed: the selector crate's own error, or an argument the host rejects.
#[derive(Clone, Debug, PartialEq)]
enum ParseKind {
    Selector(SelectorParseErrorKind),
    Argument,
}

impl From<SelectorParseErrorKind> for ParseKind {
    fn from(kind: SelectorParseErrorKind) -> Self {
        ParseKind::Selector(kind)
    }
}

type SelectorParseError = ParseError<ParseKind>;

fn unsupported() -> SelectorParseError {
    ParseError::custom(SelectorParseErrorKind::UnsupportedPseudoClassOrElement)
}

fn invalid_argument() -> SelectorParseError {
    ParseError::custom(ParseKind::Argument)
}

/// Reads one identifier and nothing else.
fn single_ident(parser: &mut CssParser<'_>) -> Result<(), SelectorParseError> {
    match parser.next() {
        Ok(Token::Ident(_)) => {}
        _ => return Err(invalid_argument()),
    }
    parser.expect_exhausted().map_err(|_| invalid_argument())
}

/// Checks the argument of `:lang()`: a comma list of identifiers and strings.
fn check_lang_arguments(parser: &mut CssParser<'_>) -> Result<(), SelectorParseError> {
    loop {
        match parser.next() {
            Ok(Token::Ident(_) | Token::QuotedString(_)) => {}
            _ => return Err(invalid_argument()),
        }
        if parser.try_parse(CssParser::expect_comma).is_err() {
            return parser.expect_exhausted().map_err(|_| invalid_argument());
        }
    }
}

/// Checks the argument of `:host-context()`: one compound selector, so no combinator, no list
/// and no pseudo-element.
fn check_compound_argument(
    selectors: &SelectorParser,
    parser: &mut CssParser<'_>,
) -> Result<(), SelectorParseError> {
    let list = SelectorList::parse_disallow_pseudo(selectors, parser, ParseRelative::No)
        .map_err(|_| invalid_argument())?;
    let [selector] = list.slice() else {
        return Err(invalid_argument());
    };
    let mut compound = selector.iter();
    for _ in &mut compound {}
    if compound.next_sequence().is_some() {
        return Err(invalid_argument());
    }
    Ok(())
}

impl<'i> Parser<'i> for SelectorParser {
    type Impl = Impl;
    type Error = ParseKind;

    fn parse_slotted(&self) -> bool {
        true
    }

    fn parse_part(&self) -> bool {
        true
    }

    fn parse_nth_child_of(&self) -> bool {
        true
    }

    fn parse_is_and_where(&self) -> bool {
        true
    }

    fn parse_has(&self) -> bool {
        true
    }

    fn parse_parent_selector(&self) -> bool {
        true
    }

    fn parse_host(&self) -> bool {
        true
    }

    fn is_is_alias(&self, name: &str) -> bool {
        name.eq_ignore_ascii_case("-webkit-any")
    }

    fn parse_non_ts_pseudo_class(
        &self,
        name: CowRcStr<'i>,
    ) -> Result<PseudoClass, SelectorParseError> {
        let name = name.to_ascii_lowercase();
        Ok(match name.as_str() {
            "defined" => PseudoClass::Defined,
            "read-only" => PseudoClass::ReadOnly,
            "read-write" => PseudoClass::ReadWrite,
            "disabled" => PseudoClass::Disabled,
            "enabled" => PseudoClass::Enabled,
            _ if NEVER_MATCHING_PSEUDO_CLASSES.contains(&name.as_str()) => PseudoClass::Never(name),
            _ => return Err(unsupported()),
        })
    }

    fn parse_non_ts_functional_pseudo_class(
        &self,
        name: CowRcStr<'i>,
        parser: &mut CssParser<'i>,
        _after_part: bool,
    ) -> Result<PseudoClass, SelectorParseError> {
        let name = name.to_ascii_lowercase();
        match name.as_str() {
            "lang" => check_lang_arguments(parser)?,
            "host-context" => check_compound_argument(self, parser)?,
            "state" => single_ident(parser)?,
            "dir" => {
                if parser.try_parse(|p| p.expect_ident_matching("ltr")).is_ok() {
                    parser.expect_exhausted().map_err(|_| invalid_argument())?;
                    return Ok(PseudoClass::DirLtr);
                }
                single_ident(parser)?;
            }
            _ => return Err(unsupported()),
        }
        Ok(PseudoClass::Never(name))
    }

    fn parse_pseudo_element(
        &self,
        name: CowRcStr<'i>,
    ) -> Result<PseudoElement, SelectorParseError> {
        let name = name.to_ascii_lowercase();
        if PSEUDO_ELEMENTS.contains(&name.as_str()) || name.starts_with("-webkit-") {
            Ok(PseudoElement(name))
        } else {
            Err(unsupported())
        }
    }

    fn parse_functional_pseudo_element(
        &self,
        name: CowRcStr<'i>,
        parser: &mut CssParser<'i>,
    ) -> Result<PseudoElement, SelectorParseError> {
        if !name.eq_ignore_ascii_case("highlight") {
            return Err(unsupported());
        }
        single_ident(parser)?;
        Ok(PseudoElement("highlight".to_owned()))
    }
}

/// An element of the tree: a node that is neither text content nor tagged `#comment`.
#[derive(Clone)]
struct Node<'t> {
    tree: &'t VirtualTree,
    node: &'t VirtualNode,
}

impl fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node.id())
    }
}

fn is_element(node: &VirtualNode) -> bool {
    !matches!(node.tag_name(), COMMENT_TAG | TEXT_TAG)
}

/// The text of an attribute value: a string as is, a number through `js_number_text`, a bool as
/// `true` or `false`.
fn attribute_text(value: &AttributeValue) -> String {
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
    fn of(tree: &'t VirtualTree, id: NodeId) -> Option<Self> {
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

fn reason(error: &SelectorParseError) -> &'static str {
    match &error.kind {
        ParseErrorKind::Custom(ParseKind::Argument) => {
            "its pseudo-class or pseudo-element argument is not valid"
        }
        ParseErrorKind::Custom(ParseKind::Selector(kind)) => match kind {
            SelectorParseErrorKind::EmptySelector => "it has an empty part",
            SelectorParseErrorKind::DanglingCombinator => "it has a combinator without an operand",
            SelectorParseErrorKind::UnsupportedPseudoClassOrElement => {
                "it names an unknown pseudo-class or pseudo-element"
            }
            _ => MALFORMED,
        },
        ParseErrorKind::Basic(_) => MALFORMED,
    }
}

const MALFORMED: &str = "it does not follow the selector grammar";

fn parse(selector: &str) -> Result<SelectorList<Impl>, SelectorError> {
    let mut input = CssParser::new(selector);
    SelectorList::parse(&SelectorParser, &mut input, ParseRelative::No).map_err(|error| {
        SelectorError {
            reason: reason(&error),
        }
    })
}

fn matching_context(
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

fn matches_with(list: &SelectorList<Impl>, element: &Node<'_>, scope: OpaqueElement) -> bool {
    let mut caches = SelectorCaches::default();
    matches_selector_list(list, element, &mut matching_context(&mut caches, scope))
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

/// Parses `selector` as a selector list.
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn check_selector(selector: &str) -> Result<(), SelectorError> {
    parse(selector).map(drop)
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

/// The descendants of `start` that match `selector`, in document order. The walk stops at the
/// first match when `first_only` is set.
fn search(
    tree: &VirtualTree,
    start: NodeId,
    selector: &str,
    first_only: bool,
) -> Result<Vec<NodeId>, SelectorError> {
    let list = parse(selector)?;
    let mut found = Vec::new();
    let Some(root) = Node::of(tree, start) else {
        return Ok(found);
    };
    let mut caches = SelectorCaches::default();
    let mut context = matching_context(&mut caches, root.opaque());
    let mut pending: Vec<NodeId> = root.node.children().iter().rev().copied().collect();
    while let Some(id) = pending.pop() {
        let Some(element) = Node::of(tree, id) else {
            continue;
        };
        if matches_selector_list(&list, &element, &mut context) {
            found.push(id);
            if first_only {
                break;
            }
        }
        pending.extend(element.node.children().iter().rev().copied());
    }
    Ok(found)
}

/// The first descendant of `node` that matches `selector` in document order. `:scope` and a
/// top-level `&` are `node`. A node that is not an element, such as a text leaf or a node tagged
/// `#comment`, and an unknown id find nothing.
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn query_selector(
    tree: &VirtualTree,
    node: NodeId,
    selector: &str,
) -> Result<Option<NodeId>, SelectorError> {
    Ok(search(tree, node, selector, true)?.first().copied())
}

/// Every descendant of `node` that matches `selector`, each once, in document order. The other
/// rules are those of [`query_selector`].
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn query_selector_all(
    tree: &VirtualTree,
    node: NodeId,
    selector: &str,
) -> Result<Vec<NodeId>, SelectorError> {
    search(tree, node, selector, false)
}

#[cfg(test)]
mod tests;
