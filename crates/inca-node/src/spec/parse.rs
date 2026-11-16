// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::keys::{read_edges, read_prop};
use super::number::MAX_LENGTH_PX;
use super::{EdgesSpec, LengthSpec, OverflowSpec, StyleSpec};
use crate::tree::{AttributeValue, NodeId, StyleMap, VirtualTree};

/// One or two overflow values, x first; a single value applies to both axes.
pub(super) fn overflow_pair_from_str(s: &str) -> Option<(OverflowSpec, OverflowSpec)> {
    let mut values = s.split_ascii_whitespace().map(OverflowSpec::from_name);
    let x = values.next()??;
    let y = values.next().unwrap_or(Some(x))?;
    values.next().is_none().then_some((x, y))
}

/// The computed axes: beside an axis that is `hidden` or scrolls, an unset or `visible` axis
/// computes to `auto` and a `clip` axis to `hidden`.
pub(super) fn computed_axes(
    x: Option<OverflowSpec>,
    y: Option<OverflowSpec>,
) -> (Option<OverflowSpec>, Option<OverflowSpec>) {
    let scrollable = |axis| {
        matches!(
            axis,
            Some(OverflowSpec::Hidden | OverflowSpec::Scroll | OverflowSpec::Auto)
        )
    };
    let computed = |axis, other| match axis {
        None | Some(OverflowSpec::Visible) if scrollable(other) => Some(OverflowSpec::Auto),
        Some(OverflowSpec::Clip) if scrollable(other) => Some(OverflowSpec::Hidden),
        axis => axis,
    };
    (computed(x, y), computed(y, x))
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
    props: &StyleMap,
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
pub(super) fn style_spec_from_props(props: &StyleMap) -> StyleSpec {
    let mut style = StyleSpec::default();
    let mut overflow = Overflows::default();
    for (key, value) in props {
        read_prop(&mut style, &mut overflow, key, value);
    }
    // Precedence does not depend on the order of `props`, so it is resolved here.
    (style.overflow_x, style.overflow_y) = computed_axes(
        overflow.x.or(overflow.all.map(|(x, _)| x)),
        overflow.y.or(overflow.all.map(|(_, y)| y)),
    );
    read_edges(props, &mut style);
    style
}

/// The window size an element's style declares, in px: `(width, height)`.
/// A dimension that is unset, `"auto"`, a percentage, not a size `>= 0` or `33554428` and above is
/// `None`.
#[must_use]
pub fn declared_window_size(tree: &VirtualTree, id: NodeId) -> (Option<f64>, Option<f64>) {
    let style = style_spec_from_props(&tree.computed_style(id));
    let px = |length| match length {
        Some(LengthSpec::Px(n)) if n < MAX_LENGTH_PX => Some(n),
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

    #[test]
    fn auto_margin_on_an_axis_reaches_both_sides() {
        let style = style_of(&[("margin_x", text("auto")), ("margin_y", text("auto"))]);
        assert_eq!(sides(style.margin), [Some(LengthSpec::Auto); 4]);
    }

    #[rstest]
    #[case::hidden(&[("overflow", "hidden")], (Some(OverflowSpec::Hidden), Some(OverflowSpec::Hidden)))]
    #[case::visible(&[("overflow", "visible")], (Some(OverflowSpec::Visible), Some(OverflowSpec::Visible)))]
    #[case::auto(&[("overflow", "auto")], (Some(OverflowSpec::Auto), Some(OverflowSpec::Auto)))]
    #[case::clip(&[("overflow", "clip")], (Some(OverflowSpec::Clip), Some(OverflowSpec::Clip)))]
    #[case::clip_axis_key(&[("overflow_x", "clip")], (Some(OverflowSpec::Clip), None))]
    #[case::two_values(&[("overflow", "hidden scroll")], (Some(OverflowSpec::Hidden), Some(OverflowSpec::Scroll)))]
    #[case::two_non_scrollable_values(&[("overflow", "clip visible")], (Some(OverflowSpec::Clip), Some(OverflowSpec::Visible)))]
    #[case::visible_beside_hidden(&[("overflow", "visible hidden")], (Some(OverflowSpec::Auto), Some(OverflowSpec::Hidden)))]
    #[case::clip_beside_auto(&[("overflow", "clip auto")], (Some(OverflowSpec::Hidden), Some(OverflowSpec::Auto)))]
    #[case::unset_beside_scroll(&[("overflow_x", "scroll")], (Some(OverflowSpec::Scroll), Some(OverflowSpec::Auto)))]
    #[case::unset_beside_auto(&[("overflow_x", "auto")], (Some(OverflowSpec::Auto), Some(OverflowSpec::Auto)))]
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
    #[case::beyond_the_limit("99999999px".into(), "10px".into(), (None, Some(10.0)))]
    #[case::at_the_limit("33554428px".into(), "10px".into(), (None, Some(10.0)))]
    #[case::below_the_limit(
        "33554427px".into(),
        "10000px".into(),
        (Some(33_554_427.0), Some(10000.0))
    )]
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
