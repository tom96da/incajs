// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

#![allow(clippy::unwrap_used)]

use super::*;
use crate::element::{STYLE_KEYS, ValueKind, style_key};
use rstest::rstest;

#[test]
fn a_removed_style_key_renders_as_the_default() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "gap", 8.0).unwrap();
    tree.set_style(id, "background", f64::from(0x505050))
        .unwrap();
    tree.remove_style(id, "gap").unwrap();
    tree.remove_style(id, "background").unwrap();

    assert_eq!(build_spec(&tree, id).unwrap().style, StyleSpec::default());
}

#[test]
fn recognized_style_keys_are_mapped() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "display", "flex").unwrap();
    tree.set_style(id, "flex_direction", "column").unwrap();
    tree.set_style(id, "justify_content", "center").unwrap();
    tree.set_style(id, "align_items", "stretch").unwrap();
    tree.set_style(id, "gap", 8.0).unwrap();
    tree.set_style(id, "width", 120.0).unwrap();
    tree.set_style(id, "height", "auto").unwrap();
    tree.set_style(id, "border_width", 1.0).unwrap();
    tree.set_style(id, "background", f64::from(0x505050))
        .unwrap();
    tree.set_style(id, "border_color", f64::from(0x0000ff))
        .unwrap();
    tree.set_style(id, "corner_radius", 4.0).unwrap();
    tree.set_style(id, "text_color", f64::from(0xffffff))
        .unwrap();
    tree.set_style(id, "text_size", 20.0).unwrap();

    let spec = build_spec(&tree, id).unwrap();
    assert_eq!(
        spec.style,
        StyleSpec {
            display: Some(DisplaySpec::Flex),
            flex_direction: Some(FlexDirectionSpec::Column),
            justify_content: Some(AlignSpec::Center),
            align_items: Some(AlignSpec::Stretch),
            gap: Some(8.0),
            width: Some(LengthSpec::Px(120.0)),
            height: Some(LengthSpec::Auto),
            border_width: Some(1.0),
            background: Some(ColorSpec::opaque(0x505050)),
            border_color: Some(ColorSpec::opaque(0x0000ff)),
            corner_radius: Some(4.0),
            text_color: Some(ColorSpec::opaque(0xffffff)),
            text_size: Some(20.0),
            ..StyleSpec::default()
        }
    );
}

fn sides<T: Copy>(edges: EdgesSpec<T>) -> [Option<T>; 4] {
    [edges.top, edges.right, edges.bottom, edges.left]
}

#[test]
fn box_keys_are_mapped() {
    let style = style_of(&[
        ("padding_top", num(1.0)),
        ("margin_left", text("auto")),
        ("flex_grow", num(2.0)),
        ("flex_shrink", num(0.0)),
        ("opacity", num(0.5)),
        ("min_width", num(10.0)),
        ("min_height", text("auto")),
        ("max_width", num(300.0)),
        ("max_height", num(200.0)),
    ]);
    assert_eq!(
        style,
        StyleSpec {
            padding: EdgesSpec {
                top: Some(1.0),
                ..EdgesSpec::default()
            },
            margin: EdgesSpec {
                left: Some(LengthSpec::Auto),
                ..EdgesSpec::default()
            },
            flex_grow: Some(2.0),
            flex_shrink: Some(0.0),
            opacity: Some(0.5),
            min_width: Some(LengthSpec::Px(10.0)),
            min_height: Some(LengthSpec::Auto),
            max_width: Some(LengthSpec::Px(300.0)),
            max_height: Some(LengthSpec::Px(200.0)),
            ..StyleSpec::default()
        }
    );
}

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
fn each_padding_and_margin_side_key_sets_its_own_side() {
    let style = style_of(&[
        ("padding_top", num(1.0)),
        ("padding_right", num(2.0)),
        ("padding_bottom", num(3.0)),
        ("padding_left", num(4.0)),
        ("margin_top", num(5.0)),
        ("margin_right", text("auto")),
        ("margin_bottom", num(-7.0)),
        ("margin_left", num(8.0)),
    ]);
    assert_eq!(
        sides(style.padding),
        [Some(1.0), Some(2.0), Some(3.0), Some(4.0)]
    );
    assert_eq!(
        sides(style.margin),
        [
            Some(LengthSpec::Px(5.0)),
            Some(LengthSpec::Auto),
            Some(LengthSpec::Px(-7.0)),
            Some(LengthSpec::Px(8.0)),
        ]
    );
}

#[test]
fn auto_margin_on_an_axis_reaches_both_sides() {
    let style = style_of(&[("margin_x", text("auto")), ("margin_y", text("auto"))]);
    assert_eq!(sides(style.margin), [Some(LengthSpec::Auto); 4]);
}

#[test]
fn box_keys_reach_the_gpui_refinement() {
    let style = style_of(&[
        ("padding_right", num(1.0)),
        ("padding_bottom", num(2.0)),
        ("padding_left", num(3.0)),
        ("margin_top", num(-2.0)),
        ("margin_right", text("auto")),
        ("margin_bottom", num(4.0)),
        ("margin_left", num(-5.0)),
        ("flex_grow", num(2.0)),
        ("flex_shrink", num(0.5)),
        ("opacity", num(0.5)),
        ("min_width", num(10.0)),
        ("max_height", num(20.0)),
    ]);
    let mut refinement = StyleRefinement::default();
    apply_style(&mut refinement, &style);
    assert_eq!(refinement.padding.top, None);
    assert_eq!(refinement.padding.right, Some(px(1.).into()));
    assert_eq!(refinement.padding.bottom, Some(px(2.).into()));
    assert_eq!(refinement.padding.left, Some(px(3.).into()));
    assert_eq!(refinement.margin.top, Some(px(-2.).into()));
    assert_eq!(refinement.margin.right, Some(Length::Auto));
    assert_eq!(refinement.margin.bottom, Some(px(4.).into()));
    assert_eq!(refinement.margin.left, Some(px(-5.).into()));
    assert_eq!(refinement.flex_grow, Some(2.0));
    assert_eq!(refinement.flex_shrink, Some(0.5));
    assert_eq!(refinement.opacity, Some(0.5));
    assert_eq!(refinement.min_size.width, Some(px(10.).into()));
    assert_eq!(refinement.max_size.height, Some(px(20.).into()));
    assert_eq!(refinement.min_size.height, None);
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

#[test]
fn removing_a_side_key_falls_back_to_the_axis_then_the_shorthand() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "margin", 1.0).unwrap();
    tree.set_style(id, "margin_x", 2.0).unwrap();
    tree.set_style(id, "margin_left", "auto").unwrap();
    let left = |tree: &VirtualTree| build_spec(tree, id).unwrap().style.margin.left;
    assert_eq!(left(&tree), Some(LengthSpec::Auto));
    tree.remove_style(id, "margin_left").unwrap();
    assert_eq!(left(&tree), Some(LengthSpec::Px(2.0)));
    tree.remove_style(id, "margin_x").unwrap();
    assert_eq!(left(&tree), Some(LengthSpec::Px(1.0)));
    tree.remove_style(id, "margin").unwrap();
    assert_eq!(left(&tree), None);
}

fn overflow_of(props: &[(&str, &str)]) -> (Option<OverflowSpec>, Option<OverflowSpec>) {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    for (key, value) in props {
        tree.set_style(id, *key, *value).unwrap();
    }
    let style = build_spec(&tree, id).unwrap().style;
    (style.overflow_x, style.overflow_y)
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
#[case::shorthand_first([0, 1, 2])]
#[case::axes_then_shorthand([2, 1, 0])]
#[case::x_shorthand_y([1, 0, 2])]
#[case::x_y_shorthand([1, 2, 0])]
fn an_overflow_axis_key_beats_the_shorthand_in_any_order(#[case] order: [usize; 3]) {
    use OverflowSpec::{Hidden, Scroll};
    let keys = [
        ("overflow", "hidden"),
        ("overflow_x", "scroll"),
        ("overflow_y", "scroll"),
    ];
    let props: Vec<_> = order.iter().map(|&i| keys[i]).collect();
    assert_eq!(overflow_of(&props), (Some(Scroll), Some(Scroll)));
    assert_eq!(
        overflow_of(&[("overflow_y", "scroll"), ("overflow", "hidden")]),
        (Some(Hidden), Some(Scroll))
    );
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

#[test]
fn a_removed_overflow_key_renders_as_the_default() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();
    tree.set_style(id, "overflow", "scroll").unwrap();
    tree.remove_style(id, "overflow").unwrap();
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
fn clip_overflow_reaches_the_gpui_refinement_as_clip() {
    let mut refinement = StyleRefinement::default();
    apply_style(&mut refinement, &style_of(&[("overflow", text("clip"))]));
    assert_eq!(refinement.overflow.x, Some(gpui::Overflow::Clip));
    assert_eq!(refinement.overflow.y, Some(gpui::Overflow::Clip));
}

#[test]
fn unrecognized_style_key_is_ignored_with_a_warning() {
    let (style, warnings) = style_and_warnings(&[("not_a_real_prop", num(1.0))]);
    assert_eq!(style, StyleSpec::default());
    assert_eq!(warnings, ["ignoring unknown style key `not_a_real_prop`"]);
}

#[test]
fn every_spelling_of_a_known_key_applies_quietly() {
    for key in ["flexDirection", "flex-direction", "flex_direction"] {
        let (style, warnings) = style_and_warnings(&[(key, text("column"))]);
        assert_eq!(
            style.flex_direction,
            Some(FlexDirectionSpec::Column),
            "{key}"
        );
        assert!(warnings.is_empty(), "{key}: {warnings:?}");
    }
    for key in ["paddingTop", "padding-top"] {
        let (style, warnings) = style_and_warnings(&[(key, num(4.0))]);
        assert_eq!(style.padding.top, Some(4.0), "{key}");
        assert!(warnings.is_empty(), "{key}: {warnings:?}");
    }
}

#[test]
fn a_warning_names_the_key_as_written() {
    for key in ["fooBar", "foo-bar", "FlexDirection", "flex-Direction"] {
        let (_, warnings) = style_and_warnings(&[(key, num(1.0))]);
        assert_eq!(warnings, [format!("ignoring unknown style key `{key}`")]);
    }
    for key in ["textColor", "text-color", "text_color"] {
        let (style, warnings) = style_and_warnings(&[(key, num(-1.0))]);
        assert_eq!(style.text_color, None, "{key}");
        assert_eq!(
            warnings,
            [format!("ignoring invalid color for style key `{key}`")]
        );
    }
    for key in ["flexGrow", "flex-grow", "flex_grow"] {
        let (_, warnings) = style_and_warnings(&[(key, num(-1.0))]);
        assert_eq!(
            warnings,
            [format!("ignoring unsupported value for style key `{key}`")]
        );
    }
}

#[rstest]
#[case::color("color", text("nope"), "invalid color")]
#[case::font_size("fontSize", text("big"), "unsupported value")]
#[case::border_radius("border-radius", num(-1.0), "unsupported value")]
fn a_warning_names_an_alias_as_written(
    #[case] key: &str,
    #[case] value: AttributeValue,
    #[case] what: &str,
) {
    let (_, warnings) = style_and_warnings(&[(key, value)]);

    assert_eq!(warnings, [format!("ignoring {what} for style key `{key}`")]);
}

#[test]
fn recognized_style_keys_apply_quietly() {
    let (_, warnings) = style_and_warnings(&[
        ("flex_direction", text("column")),
        ("padding_top", num(4.0)),
        ("background", num(1.0)),
        ("overflow", text("scroll")),
        ("opacity", num(2.0)),
        ("margin_left", num(-8.0)),
        ("width", text("auto")),
    ]);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[rstest]
#[case::display("display", text("nope"))]
#[case::flex_direction("flex_direction", text("nope"))]
#[case::justify_content("justify_content", text("nope"))]
#[case::align_items("align_items", num(1.0))]
#[case::gap_negative("gap", num(-1.0))]
#[case::width_negative("width", num(-1.0))]
#[case::height_negative("height", num(-1.0))]
#[case::border_width_negative("border_width", num(-1.0))]
#[case::corner_radius_negative("corner_radius", num(-1.0))]
#[case::text_size_negative("text_size", num(-1.0))]
#[case::overflow("overflow", text("nope"))]
#[case::overflow_x("overflow_x", num(1.0))]
#[case::overflow_y("overflow_y", text("hidden scroll"))]
#[case::flex_grow("flex_grow", num(-1.0))]
#[case::flex_shrink("flex_shrink", text("nope"))]
#[case::opacity("opacity", text("half"))]
#[case::min_width_negative("min_width", num(-1.0))]
#[case::min_height_negative("min_height", num(-1.0))]
#[case::max_width_negative("max_width", num(-1.0))]
#[case::max_height_negative("max_height", num(-1.0))]
#[case::padding("padding", num(-1.0))]
#[case::margin("margin_top", num(f64::NAN))]
fn a_known_key_with_an_unsupported_value_warns(#[case] key: &str, #[case] value: AttributeValue) {
    let (_, warnings) = style_and_warnings(&[(key, value)]);

    assert_eq!(
        warnings,
        [format!("ignoring unsupported value for style key `{key}`")]
    );
}

#[rstest]
#[case::display("display", text("flex"))]
#[case::background("background", text("#fff"))]
fn an_empty_string_on_a_known_key_is_quiet(#[case] key: &str, #[case] set: AttributeValue) {
    let (style, warnings) = style_and_warnings(&[(key, set), (key, text(""))]);

    assert!(warnings.is_empty(), "{key}: {warnings:?}");
    assert_eq!((style.display, style.background), (None, None), "{key}");
}

#[test]
fn an_empty_string_on_an_unknown_key_warns() {
    let (_, warnings) = style_and_warnings(&[("not_a_real_prop", text(""))]);

    assert_eq!(warnings, ["ignoring unknown style key `not_a_real_prop`"]);
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

#[test]
fn applying_a_style_keeps_the_values_the_spec_leaves_unset() {
    let mut style = StyleRefinement::default();
    apply_style(
        &mut style,
        &style_of(&[("display", "flex".into()), ("width", num(120.0))]),
    );
    apply_style(&mut style, &style_of(&[("height", num(40.0))]));

    assert_eq!(style.display, Some(gpui::Display::Flex));
    assert_eq!(style.size.width, Some(Length::from(px(120.0))));
    assert_eq!(style.size.height, Some(Length::from(px(40.0))));
}

/// Values `kind` accepts, chosen so that a reader of another kind with the
/// same result type rejects at least one of them.
fn sample_values(kind: ValueKind) -> Vec<AttributeValue> {
    match kind {
        ValueKind::NonNegativePx => vec![num(1.0)],
        ValueKind::NonNegativePxOrAuto => vec![text("auto")],
        ValueKind::PxOrAuto | ValueKind::Opacity => vec![num(-1.0)],
        ValueKind::NonNegativeNumber => vec![num(2.0)],
        ValueKind::Color => vec![text("#fff")],
        ValueKind::Keyword(names) => names.iter().map(|name| text(name)).collect(),
        ValueKind::KeywordPair(names) => vec![text(&format!("{} {}", names[0], names[1]))],
    }
}

#[test]
fn every_listed_alias_maps_to_its_key() {
    for entry in STYLE_KEYS {
        for alias in entry.aliases {
            assert!(
                STYLE_KEYS.iter().all(|k| k.key != *alias),
                "{alias} is also a key"
            );
            assert_eq!(style_key(alias), entry.key, "{alias}");
        }
    }
}

#[test]
fn every_listed_name_reads_a_value_of_its_kind() {
    for entry in STYLE_KEYS {
        let samples = sample_values(entry.kind);
        for name in std::iter::once(entry.key).chain(entry.aliases.iter().copied()) {
            for value in &samples {
                assert_eq!(style_warning(name, value), None, "{name} {value:?}");
            }
        }
    }
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

    assert_eq!(declared_window_size(tree.get(id).unwrap()), expected);
}

#[test]
fn window_size_is_none_when_unset() {
    let mut tree = VirtualTree::new();
    let id = tree.create_node("div").unwrap();

    assert_eq!(declared_window_size(tree.get(id).unwrap()), (None, None));
}

#[test]
fn color_keys_reach_the_gpui_refinement() {
    let mut style = StyleRefinement::default();
    apply_style(
        &mut style,
        &style_of(&[
            ("background", text("#33669980")),
            ("border_color", num(f64::from(0x0033_6699))),
            ("text_color", text("rgb(51 102 153 / 50%)")),
        ]),
    );

    assert_eq!(
        style.background,
        Some(gpui::Fill::from(gpui::Hsla::from(gpui::rgba(0x3366_9980))))
    );
    assert_eq!(style.border_color, Some(gpui::rgba(0x3366_99ff).into()));
    assert_eq!(style.text.color, Some(gpui::rgba(0x3366_9980).into()));
}
