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
    AttributeValue, COMMENT_TAG, FRAGMENT_TAG, NodeId, TEXT_TAG, VirtualNode, VirtualTree,
    is_disabled_control,
};

mod matching;
mod query;

pub use matching::{closest_selector, matches_selector};
pub use query::{query_selector, query_selector_all};

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

impl SelectorError {
    /// Why the selector is not valid, as a short phrase.
    pub(crate) fn reason(&self) -> &'static str {
        self.reason
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

/// A parsed selector list.
#[derive(Debug)]
pub(crate) struct Selectors(SelectorList<Impl>);

impl Selectors {
    /// Parses `text` as a selector list.
    pub(crate) fn parse(text: &str) -> Result<Self, SelectorError> {
        parse(text).map(Self)
    }

    /// The highest specificity among the selectors that match the element `id`. `None` when no
    /// selector matches and for a node that is not an element.
    pub(crate) fn specificity_of_match(&self, tree: &VirtualTree, id: NodeId) -> Option<u32> {
        matching::specificity_of_match(&self.0, tree, id)
    }
}

/// Parses `selector` as a selector list.
///
/// # Errors
///
/// Returns a [`SelectorError`] when `selector` is not a valid selector list.
pub fn check_selector(selector: &str) -> Result<(), SelectorError> {
    parse(selector).map(drop)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use rstest::rstest;

    #[rstest]
    #[case::only_unknown_names(":is(:foo)", false)]
    #[case::an_unknown_name_beside_a_match(":is(:foo, p)", true)]
    fn an_is_list_drops_an_unknown_name(#[case] selector: &str, #[case] expected: bool) {
        let p = page();

        assert_eq!(p.f.matches(p.p1, selector), expected);
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
    fn js_number_text_prints_a_number_as_javascript_does(
        #[case] value: f64,
        #[case] expected: &str,
    ) {
        assert_eq!(js_number_text(value), expected);
    }

    fn check_matches(selector: &str) -> Result<(), SelectorError> {
        let p = page();
        matches_selector(&p.f.tree, p.p1, selector).map(drop)
    }

    fn check_query(selector: &str) -> Result<(), SelectorError> {
        let p = page();
        query_selector(&p.f.tree, p.main, selector).map(drop)
    }

    fn check_query_all(selector: &str) -> Result<(), SelectorError> {
        let p = page();
        query_selector_all(&p.f.tree, p.main, selector).map(drop)
    }

    fn check_query_on_a_comment(selector: &str) -> Result<(), SelectorError> {
        let m = mixed();
        query_selector(&m.f.tree, m.comment, selector).map(drop)
    }

    fn check_query_all_on_a_comment(selector: &str) -> Result<(), SelectorError> {
        let m = mixed();
        query_selector_all(&m.f.tree, m.comment, selector).map(drop)
    }

    #[test]
    fn check_selector_accepts_a_valid_selector_list() {
        assert_eq!(check_selector("div > p, span"), Ok(()));
    }

    #[rstest]
    #[case::empty("", "it has an empty part")]
    #[case::trailing_combinator("div >", "it has a combinator without an operand")]
    #[case::leading_combinator("> div", "it has an empty part")]
    #[case::doubled_combinator("div > > p", "it has a combinator without an operand")]
    #[case::empty_value("[a=]", "it does not follow the selector grammar")]
    #[case::digit_value("[a=1]", "it does not follow the selector grammar")]
    #[case::empty_list_item("a,,b", "it has an empty part")]
    #[case::namespace("ns|a", "it does not follow the selector grammar")]
    #[case::unknown_pseudo_class(":foo", "it names an unknown pseudo-class or pseudo-element")]
    #[case::unknown_pseudo_class_in_not(
        ":not(:foo)",
        "it names an unknown pseudo-class or pseudo-element"
    )]
    #[case::unknown_pseudo_element("::foo", "it names an unknown pseudo-class or pseudo-element")]
    #[case::target_within(":target-within", "it names an unknown pseudo-class or pseudo-element")]
    #[case::playing(":playing", "it names an unknown pseudo-class or pseudo-element")]
    #[case::paused(":paused", "it names an unknown pseudo-class or pseudo-element")]
    #[case::unknown_function(":foo(x)", "it names an unknown pseudo-class or pseudo-element")]
    #[case::unknown_function_pseudo_element(
        "::foo(x)",
        "it names an unknown pseudo-class or pseudo-element"
    )]
    #[case::lang_without_range(
        ":lang()",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::lang_with_number(
        ":lang(1)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::lang_with_two_idents(
        ":lang(en fr)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::state_without_name(
        ":state()",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::state_with_number(
        ":state(1)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::state_with_two_idents(
        ":state(x y)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::dir_without_name(":dir()", "its pseudo-class or pseudo-element argument is not valid")]
    #[case::dir_with_number(":dir(1)", "its pseudo-class or pseudo-element argument is not valid")]
    #[case::dir_with_two_names(
        ":dir(ltr rtl)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::highlight_without_name(
        "::highlight()",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::highlight_with_number(
        "::highlight(1)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_without_selector(
        ":host-context()",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_combinator(
        ":host-context(a b)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_child_combinator(
        ":host-context(a>b)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_an_adjacent_combinator(
        ":host-context(a+b)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_sibling_combinator(
        ":host-context(a~b)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_list(
        ":host-context(a, b)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_ending_in_a_dot(
        ":host-context(a.)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_ending_in_a_colon(
        ":host-context(a:)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_two_dots(
        ":host-context(..a)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_colon_after_a_dot(
        ":host-context(.:x)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_dot_after_a_colon(
        ":host-context(:.a)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_after_a_type(
        ":host-context(a*)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_before_a_type(
        ":host-context(*a)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_function(
        ":host-context(a(x))",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_attribute_before_a_type(
        ":host-context([x]a)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_class_before_universal(
        ":host-context(.a*)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_pseudo_element(
        ":host-context(a::before)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_number(
        ":host-context(1)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_delimiter(
        ":host-context(!)",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::host_context_with_a_string(
        ":host-context(\"x\")",
        "its pseudo-class or pseudo-element argument is not valid"
    )]
    #[case::class_after_a_pseudo_element(
        "::before:hover",
        "it does not follow the selector grammar"
    )]
    #[case::form_state_after_a_pseudo_element(
        "::before:checked",
        "it does not follow the selector grammar"
    )]
    #[case::structural_after_a_pseudo_element(
        "::before:first-child",
        "it does not follow the selector grammar"
    )]
    fn an_invalid_selector_returns_the_parse_error(
        #[case] selector: &str,
        #[case] reason: &str,
        #[values(
            check_matches,
            check_selector,
            check_query,
            check_query_all,
            check_query_on_a_comment,
            check_query_all_on_a_comment
        )]
        run: fn(&str) -> Result<(), SelectorError>,
    ) {
        let error = run(selector).unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("the selector is not valid. {reason}")
        );
    }
}
