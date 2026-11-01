// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::*;

fn check_matches(selector: &str) -> Result<(), SelectorError> {
    let p = page();
    matches_selector(&p.f.tree, p.p1, selector).map(drop)
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
#[case::lang_without_range(":lang()", "its pseudo-class or pseudo-element argument is not valid")]
#[case::lang_with_number(":lang(1)", "its pseudo-class or pseudo-element argument is not valid")]
#[case::lang_with_two_idents(
    ":lang(en fr)",
    "its pseudo-class or pseudo-element argument is not valid"
)]
#[case::state_without_name(":state()", "its pseudo-class or pseudo-element argument is not valid")]
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
#[case::class_after_a_pseudo_element("::before:hover", "it does not follow the selector grammar")]
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
    #[values(check_matches, check_selector)] run: fn(&str) -> Result<(), SelectorError>,
) {
    let error = run(selector).unwrap_err();

    assert_eq!(
        error.to_string(),
        format!("the selector is not valid. {reason}")
    );
}
