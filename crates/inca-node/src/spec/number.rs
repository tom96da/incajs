// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reads a style number, a px length and an opacity from a raw value.

use super::LengthSpec;
use crate::tree::AttributeValue;

/// A decimal number in the CSS number syntax: it ends in an ASCII digit and every `.` is
/// followed by one. `5.`, `1.e2`, `inf`, `infinity` and `nan` are not decimals.
pub(super) fn decimal(s: &str) -> Option<f64> {
    if !s.as_bytes().last()?.is_ascii_digit() {
        return None;
    }
    if s.as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b'.' && !pair[1].is_ascii_digit())
    {
        return None;
    }
    s.parse().ok()
}

/// A number, or a string holding one decimal with optional surrounding
/// ASCII whitespace. The result can be negative or infinite; each key filters
/// its range.
pub(super) fn number(value: &AttributeValue) -> Option<f64> {
    match value {
        AttributeValue::Number(n) => Some(*n),
        AttributeValue::String(s) => decimal(s.trim_ascii()),
        AttributeValue::Bool(_) => None,
    }
}

/// A px value: a number, or a string holding one decimal followed by `px`
/// (any case), with optional surrounding ASCII whitespace. A string without a
/// unit counts when it reads as zero (`"0"`). The result can be negative or
/// infinite (`"1e999px"`); each caller filters its range.
pub(super) fn style_px(value: &AttributeValue) -> Option<f64> {
    match value {
        AttributeValue::Number(n) => Some(*n),
        AttributeValue::String(s) => {
            let s = s.trim_ascii();
            match s.len().checked_sub(2).and_then(|at| s.get(at..)) {
                Some(unit) if unit.eq_ignore_ascii_case("px") => decimal(&s[..s.len() - 2]),
                _ => decimal(s).filter(|n| *n == 0.0),
            }
        }
        AttributeValue::Bool(_) => None,
    }
}

/// A finite number `>= 0`; negatives, `NaN` and infinities are unset.
pub(super) fn non_negative(value: &AttributeValue) -> Option<f64> {
    number(value).filter(|n| n.is_finite() && *n >= 0.0)
}

/// A finite px value `>= 0`; negatives, `NaN` and infinities are unset.
pub(super) fn non_negative_px(value: &AttributeValue) -> Option<f64> {
    style_px(value).filter(|n| n.is_finite() && *n >= 0.0)
}

/// `"auto"` or a px value.
pub(super) fn length_spec_from(value: &AttributeValue) -> Option<LengthSpec> {
    match value {
        AttributeValue::String(s) if s.eq_ignore_ascii_case("auto") => Some(LengthSpec::Auto),
        _ => style_px(value).map(LengthSpec::Px),
    }
}

/// A finite px value or `"auto"`; `NaN` and infinities are unset.
pub(super) fn margin_length_from(value: &AttributeValue) -> Option<LengthSpec> {
    length_spec_from(value).filter(|l| !matches!(l, LengthSpec::Px(n) if !n.is_finite()))
}

/// `"auto"` or a finite px value `>= 0`; negatives, `NaN` and infinities are
/// unset.
pub(super) fn size_spec_from(value: &AttributeValue) -> Option<LengthSpec> {
    match value {
        AttributeValue::String(s) if s.eq_ignore_ascii_case("auto") => Some(LengthSpec::Auto),
        _ => non_negative_px(value).map(LengthSpec::Px),
    }
}

/// A finite decimal value clamped to `0..=1`; `NaN` and infinities are unset.
pub(super) fn opacity(value: &AttributeValue) -> Option<f64> {
    number(value)
        .filter(|n| n.is_finite())
        .map(|n| n.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use crate::{EdgesSpec, StyleSpec};
    use rstest::rstest;

    #[rstest]
    #[case::negative(num(-1.0))]
    #[case::nan(num(f64::NAN))]
    #[case::infinity(num(f64::INFINITY))]
    #[case::auto(text("auto"))]
    #[case::two_values(text("1 2"))]
    fn padding_ignores_a_value_that_is_not_a_finite_px_at_least_zero(#[case] bad: AttributeValue) {
        let padding = style_of(&[("padding", bad)]).padding;
        assert_eq!(padding, EdgesSpec::default());
    }

    #[rstest]
    #[case::nan(num(f64::NAN))]
    #[case::infinity(num(f64::INFINITY))]
    #[case::negative_infinity(num(f64::NEG_INFINITY))]
    #[case::two_values(text("1 2"))]
    fn margin_ignores_a_value_that_is_not_a_finite_px_or_auto(#[case] bad: AttributeValue) {
        let margin = style_of(&[("margin", bad)]).margin;
        assert_eq!(margin, EdgesSpec::default());
    }

    #[test]
    fn flex_factors_reject_negatives_and_nan() {
        for bad in [num(-1.0), num(f64::NAN), num(f64::INFINITY), text("1px")] {
            let style = style_of(&[("flex_grow", bad.clone()), ("flex_shrink", bad)]);
            assert_eq!((style.flex_grow, style.flex_shrink), (None, None));
        }
        let style = style_of(&[("flex_grow", num(0.0)), ("flex_shrink", num(1.5))]);
        assert_eq!((style.flex_grow, style.flex_shrink), (Some(0.0), Some(1.5)));
    }

    #[test]
    fn opacity_is_clamped_to_zero_through_one() {
        let opacity = |value| style_of(&[("opacity", value)]).opacity;
        assert_eq!(opacity(num(2.0)), Some(1.0));
        assert_eq!(opacity(num(-1.0)), Some(0.0));
        assert_eq!(opacity(num(0.25)), Some(0.25));
        assert_eq!(
            (opacity(num(0.0)), opacity(num(1.0))),
            (Some(0.0), Some(1.0))
        );
    }

    #[rstest]
    #[case::nan(num(f64::NAN))]
    #[case::infinity(num(f64::INFINITY))]
    #[case::negative_infinity(num(f64::NEG_INFINITY))]
    #[case::overflowing_text(text("1e999"))]
    #[case::negative_overflowing_text(text("-1e999"))]
    #[case::non_numeric_string(text("half"))]
    fn opacity_that_is_not_a_finite_number_is_unset(#[case] value: AttributeValue) {
        assert_eq!(style_of(&[("opacity", value)]).opacity, None);
    }

    #[test]
    fn min_and_max_ignore_a_wrong_shape() {
        let style = style_of(&[
            ("min_width", text("50%")),
            ("max_width", AttributeValue::Bool(true)),
            ("min_height", num(1.0)),
            ("max_height", text("auto")),
        ]);
        assert_eq!((style.min_width, style.max_width), (None, None));
        assert_eq!(style.min_height, Some(LengthSpec::Px(1.0)));
        assert_eq!(style.max_height, Some(LengthSpec::Auto));
    }

    #[rstest]
    #[case::px("10px", 10.0)]
    #[case::upper_case_unit("10PX", 10.0)]
    #[case::surrounding_space(" 10px ", 10.0)]
    #[case::plus_sign("+10px", 10.0)]
    #[case::leading_dot(".5px", 0.5)]
    #[case::exponent("1e1px", 10.0)]
    #[case::upper_case_exponent("1E2px", 100.0)]
    #[case::signed_exponent("1e+2px", 100.0)]
    #[case::negative_exponent("1e-1px", 0.1)]
    #[case::leading_dot_exponent(".5e1px", 5.0)]
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
    #[case::dot_before_exponent("1.e2px")]
    #[case::exponent_without_digits("1epx")]
    #[case::exponent_sign_without_digits("1e+px")]
    #[case::fraction_in_exponent("1e2.5px")]
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
    #[case::dot_before_exponent("1.e2")]
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
    fn a_boolean_for_a_numeric_key_warns() {
        let (style, warnings) = style_and_warnings(&[("opacity", AttributeValue::Bool(true))]);

        assert_eq!(style.opacity, None);
        assert_eq!(
            warnings,
            ["ignoring unsupported value for style key `opacity`"]
        );
    }
}
