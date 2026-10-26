// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::*;
use rstest::rstest;

#[rstest]
#[case::px("10px", 10.0)]
#[case::upper_case_unit("10PX", 10.0)]
#[case::surrounding_space(" 10px ", 10.0)]
#[case::plus_sign("+10px", 10.0)]
#[case::leading_dot(".5px", 0.5)]
#[case::exponent("1e1px", 10.0)]
#[case::negative_zero("-0px", 0.0)]
#[case::unitless_zero("0", 0.0)]
#[case::unitless_decimal_zero("0.0", 0.0)]
#[case::unitless_signed_zero("+0", 0.0)]
fn a_px_string_equals_the_number(#[case] written: &str, #[case] want: f64) {
    let (style, warnings) = style_and_warnings(&[("width", text(written))]);

    assert_eq!(style.width, Some(LengthSpec::Px(want)), "{written}");
    assert!(warnings.is_empty(), "{written}: {warnings:?}");
}

#[rstest]
#[case::no_unit("10")]
#[case::space_before_unit("10 px")]
#[case::space_inside_number("1 0px")]
#[case::unit_only("px")]
#[case::trailing_dot("5.px")]
#[case::infinity("infpx")]
#[case::em("10em")]
#[case::percent("50%")]
fn a_px_string_in_a_wrong_form_warns(#[case] written: &str) {
    let (style, warnings) = style_and_warnings(&[("width", text(written))]);

    assert_eq!(style.width, None, "{written}");
    assert_eq!(
        warnings,
        ["ignoring unsupported value for style key `width`"],
        "{written}"
    );
}

#[rstest]
#[case::gap_negative("gap", "-1px", false)]
#[case::width_negative("width", "-1px", false)]
#[case::margin_negative("margin_top", "-1px", true)]
#[case::width_infinite("width", "1e999px", false)]
#[case::margin_infinite("margin_top", "1e999px", false)]
fn a_px_string_follows_the_key_range(
    #[case] key: &str,
    #[case] written: &str,
    #[case] applied: bool,
) {
    let (_, warnings) = style_and_warnings(&[(key, text(written))]);

    assert_eq!(
        warnings.is_empty(),
        applied,
        "{key} {written}: {warnings:?}"
    );
}

#[rstest]
#[case::gap("gap")]
#[case::border_width("border_width")]
#[case::corner_radius("corner_radius")]
#[case::text_size("text_size")]
#[case::width("width")]
#[case::height("height")]
#[case::min_width("min_width")]
#[case::min_height("min_height")]
#[case::max_width("max_width")]
#[case::max_height("max_height")]
#[case::padding("padding")]
#[case::margin("margin")]
fn every_px_key_takes_a_px_string(#[case] key: &str) {
    let (style, warnings) = style_and_warnings(&[(key, text("10px"))]);
    let (want, _) = style_and_warnings(&[(key, num(10.0))]);

    assert_ne!(style, StyleSpec::default(), "{key}");
    assert_eq!(style, want, "{key}");
    assert!(warnings.is_empty(), "{key}: {warnings:?}");
}

#[test]
fn a_px_string_in_a_side_key_falls_through_to_the_shorthand() {
    let style = style_of(&[
        ("padding", num(2.0)),
        ("padding_top", text("4px")),
        ("padding_left", text("1em")),
    ]);

    assert_eq!(style.padding.top, Some(4.0));
    assert_eq!(style.padding.left, Some(2.0));
}

#[rstest]
#[case::plain("0.5", 0.5)]
#[case::leading_dot(".5", 0.5)]
#[case::plus_dot("+.5", 0.5)]
#[case::exponent("1e-1", 0.1)]
#[case::surrounding_space(" 0.5 ", 0.5)]
fn a_numeric_string_equals_the_number(#[case] written: &str, #[case] want: f64) {
    let (style, warnings) = style_and_warnings(&[("opacity", text(written))]);

    assert_eq!(style.opacity, Some(want), "{written}");
    assert!(warnings.is_empty(), "{written}: {warnings:?}");
}

#[rstest]
#[case::opacity("opacity")]
#[case::flex_grow("flex_grow")]
#[case::flex_shrink("flex_shrink")]
fn every_numeric_key_takes_a_numeric_string(#[case] key: &str) {
    let (style, warnings) = style_and_warnings(&[(key, text("0.5"))]);
    let (want, _) = style_and_warnings(&[(key, num(0.5))]);

    assert_ne!(style, StyleSpec::default(), "{key}");
    assert_eq!(style, want, "{key}");
    assert!(warnings.is_empty(), "{key}: {warnings:?}");
}

#[rstest]
#[case::hex("0x1")]
#[case::infinity("Infinity")]
#[case::trailing_dot("5.")]
#[case::percent("50%")]
fn a_numeric_string_in_a_wrong_form_warns(#[case] written: &str) {
    let (style, warnings) = style_and_warnings(&[("opacity", text(written))]);

    assert_eq!(style.opacity, None, "{written}");
    assert_eq!(
        warnings,
        ["ignoring unsupported value for style key `opacity`"],
        "{written}"
    );
}

#[rstest]
#[case::flex_grow("flex_grow")]
#[case::flex_shrink("flex_shrink")]
fn a_negative_numeric_string_warns_on_a_flex_factor(#[case] key: &str) {
    let (style, warnings) = style_and_warnings(&[(key, text("-1"))]);

    assert_eq!(style, StyleSpec::default(), "{key}");
    assert_eq!(
        warnings,
        [format!("ignoring unsupported value for style key `{key}`")]
    );
}

#[test]
fn a_flex_start_keyword_applies_as_start_on_justify_content() {
    let (style, warnings) = style_and_warnings(&[("justify_content", text("flex-start"))]);

    assert_eq!(style.justify_content, Some(AlignSpec::Start));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_flex_end_keyword_applies_as_end_on_align_items() {
    let (style, warnings) = style_and_warnings(&[("align_items", text("flex-end"))]);

    assert_eq!(style.align_items, Some(AlignSpec::End));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[rstest]
#[case::row_hyphen("row-reverse", FlexDirectionSpec::RowReverse)]
#[case::row_underscore("row_reverse", FlexDirectionSpec::RowReverse)]
#[case::column_hyphen("column-reverse", FlexDirectionSpec::ColumnReverse)]
#[case::column_underscore("column_reverse", FlexDirectionSpec::ColumnReverse)]
fn a_reverse_direction_value_applies_in_both_spellings(
    #[case] written: &str,
    #[case] want: FlexDirectionSpec,
) {
    let (style, warnings) = style_and_warnings(&[("flex_direction", text(written))]);

    assert_eq!(style.flex_direction, Some(want), "{written}");
    assert!(warnings.is_empty(), "{written}: {warnings:?}");
}

#[rstest]
#[case::underscore_start("justify_content", "flex_start")]
#[case::capitalized_start("align_items", "Flex-Start")]
#[case::upper_case_direction("flex_direction", "ROW-REVERSE")]
fn a_keyword_in_another_spelling_warns(#[case] key: &str, #[case] written: &str) {
    let (style, warnings) = style_and_warnings(&[(key, text(written))]);

    assert_eq!(style, StyleSpec::default(), "{key} {written}");
    assert_eq!(
        warnings,
        [format!("ignoring unsupported value for style key `{key}`")],
        "{key} {written}"
    );
}

#[test]
fn a_boolean_for_a_numeric_key_warns() {
    let (style, warnings) = style_and_warnings(&[("opacity", AttributeValue::Bool(true))]);

    assert_eq!(style.opacity, None);
    assert_eq!(
        warnings,
        ["ignoring unsupported value for style key `opacity`"]
    );
}
