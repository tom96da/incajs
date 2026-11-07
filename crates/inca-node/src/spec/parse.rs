// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;

use super::keys::{read_edges, read_prop};
use super::{EdgesSpec, LengthSpec, OverflowSpec, StyleSpec};
use crate::tree::{AttributeValue, NodeId, VirtualTree};

/// One or two overflow values, x first; a single value applies to both axes.
pub(super) fn overflow_pair_from_str(s: &str) -> Option<(OverflowSpec, OverflowSpec)> {
    let mut values = s.split_ascii_whitespace().map(OverflowSpec::from_name);
    let x = values.next()??;
    let y = values.next().unwrap_or(Some(x))?;
    values.next().is_none().then_some((x, y))
}

/// The computed axes: beside an axis that is `hidden` or scrolls, an unset or `visible` axis
/// scrolls and a `clip` axis is `hidden`.
pub(super) fn computed_axes(
    x: Option<OverflowSpec>,
    y: Option<OverflowSpec>,
) -> (Option<OverflowSpec>, Option<OverflowSpec>) {
    let scrollable = |axis| matches!(axis, Some(OverflowSpec::Hidden | OverflowSpec::Scroll));
    let computed = |axis, other| match axis {
        None | Some(OverflowSpec::Visible) if scrollable(other) => Some(OverflowSpec::Scroll),
        Some(OverflowSpec::Clip) if scrollable(other) => Some(OverflowSpec::Hidden),
        axis => axis,
    };
    (computed(x, y), computed(y, x))
}

/// A decimal number: the syntax of `f64` ending in an ASCII digit. `5.`,
/// `inf`, `infinity` and `nan` are not decimals.
pub(super) fn decimal(s: &str) -> Option<f64> {
    if !s.as_bytes().last()?.is_ascii_digit() {
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

/// A decimal value clamped to `0..=1`; `NaN` is unset.
pub(super) fn opacity(value: &AttributeValue) -> Option<f64> {
    number(value)
        .filter(|n| !n.is_nan())
        .map(|n| n.clamp(0.0, 1.0))
}

/// Keys of one box property, most general first: the shorthand, the two
/// axes, then the four sides.
pub(super) struct EdgeKeys {
    pub(super) all: &'static str,
    pub(super) x: &'static str,
    pub(super) y: &'static str,
    pub(super) top: &'static str,
    pub(super) right: &'static str,
    pub(super) bottom: &'static str,
    pub(super) left: &'static str,
}

/// Resolves each side from the side key, else the axis key, else the
/// shorthand. A value `parse` rejects counts as unset and falls through to
/// the next key.
pub(super) fn edges_from<T>(
    props: &HashMap<String, AttributeValue>,
    keys: &EdgeKeys,
    parse: fn(&AttributeValue) -> Option<T>,
) -> EdgesSpec<T> {
    let pick = |side: &str, axis: &str| {
        [side, axis, keys.all]
            .into_iter()
            .find_map(|key| props.get(key).and_then(parse))
    };
    EdgesSpec {
        top: pick(keys.top, keys.y),
        right: pick(keys.right, keys.x),
        bottom: pick(keys.bottom, keys.y),
        left: pick(keys.left, keys.x),
    }
}

/// A style prop the spec layer drops, which [`super::keys::style_warning`] reports.
pub(super) enum StyleFault {
    UnknownKey,
    InvalidColor,
    /// A value of a shape the host does not support for its known key.
    UnsupportedValue,
}

/// The three overflow keys, resolved once every prop has been read.
#[derive(Default)]
pub(super) struct Overflows {
    /// `overflow`, as its x and y values.
    pub(super) all: Option<(OverflowSpec, OverflowSpec)>,
    pub(super) x: Option<OverflowSpec>,
    pub(super) y: Option<OverflowSpec>,
}

/// Stores `parsed` in `slot`, and returns `fault` when it is `None`.
pub(super) fn store<T>(
    slot: &mut Option<T>,
    parsed: Option<T>,
    fault: StyleFault,
) -> Option<StyleFault> {
    *slot = parsed;
    slot.is_none().then_some(fault)
}

/// Builds a [`StyleSpec`] from a node's raw `style_props`, ignoring any key
/// or value this layer doesn't (yet) recognize.
pub(super) fn style_spec_from_props(props: &HashMap<String, AttributeValue>) -> StyleSpec {
    let mut style = StyleSpec::default();
    let mut overflow = Overflows::default();
    for (key, value) in props {
        read_prop(&mut style, &mut overflow, key, value);
    }
    // `props` iterates in arbitrary order, so precedence is resolved here.
    (style.overflow_x, style.overflow_y) = computed_axes(
        overflow.x.or(overflow.all.map(|(x, _)| x)),
        overflow.y.or(overflow.all.map(|(_, y)| y)),
    );
    read_edges(props, &mut style);
    style
}

/// The window size an element's style declares, in px: `(width, height)`.
/// A dimension that is unset, `"auto"`, a percentage or not a finite size
/// `>= 0` is `None`.
#[must_use]
pub fn declared_window_size(tree: &VirtualTree, id: NodeId) -> (Option<f64>, Option<f64>) {
    let style = style_spec_from_props(&tree.computed_style(id));
    let px = |length| match length {
        Some(LengthSpec::Px(n)) => Some(n),
        _ => None,
    };
    (px(style.width), px(style.height))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use crate::{AlignSpec, FlexDirectionSpec, VirtualTree, build_spec};
    use rstest::rstest;

    #[test]
    fn a_side_beats_its_axis_which_beats_the_shorthand() {
        let mut props = vec![
            ("padding", num(1.0)),
            ("padding_x", num(2.0)),
            ("padding_left", num(3.0)),
        ];
        // top, right, bottom, left
        assert_eq!(
            sides(style_of(&props).padding),
            [Some(1.0), Some(2.0), Some(1.0), Some(3.0)]
        );
        props.remove(2);
        assert_eq!(
            sides(style_of(&props).padding),
            [Some(1.0), Some(2.0), Some(1.0), Some(2.0)]
        );
        props.push(("padding_y", num(4.0)));
        assert_eq!(
            sides(style_of(&props).padding),
            [Some(4.0), Some(2.0), Some(4.0), Some(2.0)]
        );
    }

    #[test]
    fn margin_resolves_sides_the_same_way() {
        let style = style_of(&[
            ("margin", num(1.0)),
            ("margin_y", num(2.0)),
            ("margin_bottom", text("auto")),
            ("margin_x", num(-3.0)),
        ]);
        assert_eq!(
            sides(style.margin),
            [
                Some(LengthSpec::Px(2.0)),
                Some(LengthSpec::Px(-3.0)),
                Some(LengthSpec::Auto),
                Some(LengthSpec::Px(-3.0)),
            ]
        );
    }

    #[test]
    fn a_wrong_typed_side_falls_through_to_the_axis_then_the_shorthand() {
        let style = style_of(&[
            ("padding", num(1.0)),
            ("padding_x", text("2")),
            ("padding_left", AttributeValue::Bool(true)),
            ("padding_top", num(-9.0)),
            ("padding_y", num(f64::NAN)),
        ]);
        assert_eq!(
            sides(style.padding),
            [Some(1.0), Some(1.0), Some(1.0), Some(1.0)]
        );
        let style = style_of(&[("margin", num(6.0)), ("margin_left", text("wide"))]);
        assert_eq!(style.margin.left, Some(LengthSpec::Px(6.0)));
    }

    #[test]
    fn margin_takes_a_negative_px_and_auto() {
        let margin = style_of(&[("margin_left", num(-8.0)), ("margin_right", text("auto"))]).margin;
        assert_eq!(margin.left, Some(LengthSpec::Px(-8.0)));
        assert_eq!(margin.right, Some(LengthSpec::Auto));
    }

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
        assert_eq!(opacity(num(f64::INFINITY)), Some(1.0));
        assert_eq!(opacity(num(f64::NEG_INFINITY)), Some(0.0));
    }

    #[test]
    fn opacity_of_nan_or_a_non_numeric_string_is_unset() {
        let opacity = |value| style_of(&[("opacity", value)]).opacity;
        assert_eq!(opacity(num(f64::NAN)), None);
        assert_eq!(opacity(text("half")), None);
    }

    #[test]
    fn auto_margin_on_an_axis_reaches_both_sides() {
        let style = style_of(&[("margin_x", text("auto")), ("margin_y", text("auto"))]);
        assert_eq!(sides(style.margin), [Some(LengthSpec::Auto); 4]);
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
    #[case::hidden(&[("overflow", "hidden")], (Some(OverflowSpec::Hidden), Some(OverflowSpec::Hidden)))]
    #[case::visible(&[("overflow", "visible")], (Some(OverflowSpec::Visible), Some(OverflowSpec::Visible)))]
    #[case::auto(&[("overflow", "auto")], (Some(OverflowSpec::Scroll), Some(OverflowSpec::Scroll)))]
    #[case::clip(&[("overflow", "clip")], (Some(OverflowSpec::Clip), Some(OverflowSpec::Clip)))]
    #[case::clip_axis_key(&[("overflow_x", "clip")], (Some(OverflowSpec::Clip), None))]
    #[case::two_values(&[("overflow", "hidden scroll")], (Some(OverflowSpec::Hidden), Some(OverflowSpec::Scroll)))]
    #[case::two_non_scrollable_values(&[("overflow", "clip visible")], (Some(OverflowSpec::Clip), Some(OverflowSpec::Visible)))]
    #[case::visible_beside_hidden(&[("overflow", "visible hidden")], (Some(OverflowSpec::Scroll), Some(OverflowSpec::Hidden)))]
    #[case::clip_beside_auto(&[("overflow", "clip auto")], (Some(OverflowSpec::Hidden), Some(OverflowSpec::Scroll)))]
    #[case::unset_beside_scroll(&[("overflow_x", "scroll")], (Some(OverflowSpec::Scroll), Some(OverflowSpec::Scroll)))]
    fn overflow_props_compute_to_these_axes(
        #[case] props: &[(&str, &str)],
        #[case] want: (Option<OverflowSpec>, Option<OverflowSpec>),
    ) {
        assert_eq!(overflow_of(props), want);
    }

    #[rstest]
    #[case::unknown("overflow", "nope")]
    #[case::unknown_axis("overflow_x", "nope")]
    #[case::empty("overflow", "")]
    #[case::unknown_second("overflow", "hidden nope")]
    #[case::three_values("overflow", "hidden scroll auto")]
    fn a_bad_overflow_value_is_ignored(#[case] key: &str, #[case] value: &str) {
        assert_eq!(overflow_of(&[(key, value)]), (None, None));
    }

    #[test]
    fn an_overflow_number_is_ignored() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "overflow", 1.0).unwrap();
        assert_eq!(build_spec(&tree, id).unwrap().style, StyleSpec::default());
    }

    #[rstest]
    #[case::x_scroll(&[("overflow_x", "scroll")], true)]
    #[case::y_auto(&[("overflow_y", "auto")], true)]
    #[case::scroll(&[("overflow", "scroll")], true)]
    #[case::hidden(&[("overflow", "hidden")], true)]
    #[case::visible_beside_hidden(&[("overflow_x", "visible"), ("overflow_y", "hidden")], true)]
    #[case::visible(&[("overflow", "visible")], false)]
    #[case::none(&[], false)]
    #[case::clip(&[("overflow", "clip")], false)]
    fn a_hidden_or_scrolling_axis_makes_a_scroll_container(
        #[case] props: &[(&str, &str)],
        #[case] want: bool,
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        for (key, value) in props {
            tree.set_style(id, *key, *value).unwrap();
        }
        assert_eq!(
            build_spec(&tree, id).unwrap().style.is_scroll_container(),
            want
        );
    }

    #[test]
    fn malformed_enum_value_is_ignored_not_a_panic() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "display", "not-a-real-display-value")
            .unwrap();

        let spec = build_spec(&tree, id).unwrap();
        assert_eq!(spec.style.display, None);
    }

    #[rstest]
    #[case::px_number(AttributeValue::Number(800.0), AttributeValue::Number(600.0), (Some(800.0), Some(600.0)))]
    #[case::px_string("800px".into(), " 600PX ".into(), (Some(800.0), Some(600.0)))]
    #[case::auto("auto".into(), "auto".into(), (None, None))]
    #[case::percent("50%".into(), "50%".into(), (None, None))]
    #[case::negative(AttributeValue::Number(-1.0), "-1px".into(), (None, None))]
    #[case::infinite("1e999px".into(), AttributeValue::Number(f64::INFINITY), (None, None))]
    fn window_size_reads_width_and_height_through_the_declaration(
        #[case] width: AttributeValue,
        #[case] height: AttributeValue,
        #[case] expected: (Option<f64>, Option<f64>),
    ) {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        tree.set_style(id, "width", width).unwrap();
        tree.set_style(id, "height", height).unwrap();

        assert_eq!(declared_window_size(&tree, id), expected);
    }

    #[test]
    fn a_rule_sizes_the_window() {
        let mut f = crate::tests::Fixture::new();
        f.sheet("#content { width: 800px; height: 600px }");
        let content = f.add(f.root, "div", "content");

        assert_eq!(
            declared_window_size(&f.tree, content),
            (Some(800.0), Some(600.0))
        );
    }

    #[test]
    fn window_size_is_none_when_unset() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();

        assert_eq!(declared_window_size(&tree, id), (None, None));
    }

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

    #[test]
    fn a_keyword_in_another_spelling_warns() {
        let (key, written) = ("justify_content", "flex_start");
        let (style, warnings) = style_and_warnings(&[(key, text(written))]);

        assert_eq!(style, StyleSpec::default(), "{key} {written}");
        assert_eq!(
            warnings,
            [format!("ignoring unsupported value for style key `{key}`")],
            "{key} {written}"
        );
    }

    #[rstest]
    #[case::hyphen_spelling("align_items", "Flex-Start", "flex-start")]
    #[case::all_capitals("flex_direction", "ROW-REVERSE", "row-reverse")]
    #[case::display("display", "Flex", "flex")]
    #[case::underscore_spelling("flex_direction", "Column_Reverse", "column_reverse")]
    fn a_keyword_in_another_letter_case_applies(
        #[case] key: &str,
        #[case] written: &str,
        #[case] lower: &str,
    ) {
        let (style, warnings) = style_and_warnings(&[(key, text(written))]);
        let (want, _) = style_and_warnings(&[(key, text(lower))]);

        assert_ne!(style, StyleSpec::default(), "{key} {written}");
        assert_eq!(style, want, "{key} {written}");
        assert!(warnings.is_empty(), "{key} {written}: {warnings:?}");
    }

    #[rstest]
    #[case::size("width", "AUTO")]
    #[case::margin("margin", "Auto")]
    fn auto_in_any_letter_case_reads_as_auto(#[case] key: &str, #[case] written: &str) {
        let (style, warnings) = style_and_warnings(&[(key, text(written))]);
        let (want, _) = style_and_warnings(&[(key, text("auto"))]);

        assert_ne!(style, StyleSpec::default(), "{key}");
        assert_eq!(style, want, "{key}");
        assert!(warnings.is_empty(), "{key}: {warnings:?}");
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

    #[test]
    fn a_keyword_list_names_every_spelling() {
        assert_eq!(
            FlexDirectionSpec::NAMES,
            [
                "row",
                "column",
                "row-reverse",
                "row_reverse",
                "column-reverse",
                "column_reverse",
            ]
        );
    }
}
