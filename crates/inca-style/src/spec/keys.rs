// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The readers of the style keys, their CSS text and the warnings for a bad value.

use super::color::{as_color, rgb_text};
use super::number::font_size_px;
use super::number::margin_length_from;
use super::number::non_negative;
use super::number::non_negative_px;
use super::number::opacity;
use super::number::opacity_value;
use super::number::size_spec_from;
use super::parse::EdgeKeys;
use super::parse::Overflows;
use super::parse::StyleFault;
use super::parse::edges_from;
use super::parse::overflow_pair_from_str;
use super::parse::store;
use crate::LengthSpec;
use crate::StyleSpec;
use inca_node::AlignSpec;
use inca_node::AttributeValue;
use inca_node::DisplaySpec;
use inca_node::FlexDirectionSpec;
use inca_node::OverflowSpec;
use inca_node::tree::{STYLE_KEYS, StyleMap, ValueKind, shortest_number_text, style_key};

/// Expands the style declaration below into `read_prop`, `read_edges` and one [`EdgeKeys`] const
/// per box-edge family. [`STYLE_KEYS`] lists the same keys.
macro_rules! style_props {
    (@read NonNegativePx, $value:ident) => { non_negative_px($value) };
    (@read FontSizePx, $value:ident) => { font_size_px($value) };
    (@read NonNegativePxOrAuto, $value:ident) => { size_spec_from($value) };
    (@read PxOrAuto, $value:ident) => { margin_length_from($value) };
    (@read NonNegativeNumber, $value:ident) => { non_negative($value) };
    (@read Opacity, $value:ident) => { opacity($value) };
    (@read Color, $value:ident) => { as_color($value) };
    (@read Keyword($ty:ident), $value:ident) => { $value.as_str().and_then($ty::from_name) };
    (@read KeywordPair(OverflowSpec), $value:ident) => {
        $value.as_str().and_then(overflow_pair_from_str)
    };
    (@fault Color) => { StyleFault::InvalidColor };
    (@fault $($kind:tt)*) => { StyleFault::UnsupportedValue };
    // `read_prop`'s parameters come in as arguments: this arm is another expansion.
    (@slot style . $field:ident, $style:ident, $overflow:ident) => { &mut $style.$field };
    (@slot overflow . $field:ident, $style:ident, $overflow:ident) => { &mut $overflow.$field };
    (
        props {
            $(
                $key:literal: $kind:ident $(($arg:ident))?
                    => $target:ident . $field:ident;
            )*
        }
        edges {
            $(
                $edges:ident: $edge_kind:ident => $edge_field:ident {
                    all: $all:literal,
                    x: $x:literal,
                    y: $y:literal,
                    top: $top:literal,
                    right: $right:literal,
                    bottom: $bottom:literal,
                    left: $left:literal $(,)?
                }
            )*
        }
    ) => {
        $(
            const $edges: EdgeKeys = EdgeKeys {
                all: $all,
                x: $x,
                y: $y,
                top: $top,
                right: $right,
                bottom: $bottom,
                left: $left,
            };
        )*

        /// Reads one style prop into `style` (and `overflow`), returning the fault
        /// when the key is unknown or its value is dropped. `key` is a style key in `snake_case`. A
        /// box key is only checked here: `edges_from` resolves the box keys
        /// afterwards.
        pub(super) fn read_prop(
            style: &mut StyleSpec,
            overflow: &mut Overflows,
            key: &str,
            value: &AttributeValue,
        ) -> Option<StyleFault> {
            match key {
                $(
                    $key => store(
                        style_props!(@slot $target . $field, style, overflow),
                        style_props!(@read $kind $(($arg))?, value),
                        style_props!(@fault $kind),
                    ),
                )*
                $(
                    $all | $x | $y | $top | $right | $bottom | $left => {
                        style_props!(@read $edge_kind, value)
                            .is_none()
                            .then_some(StyleFault::UnsupportedValue)
                    }
                )*
                _ => Some(StyleFault::UnknownKey),
            }
        }

        /// Resolves every box-edge family of `props` into `style`.
        pub(super) fn read_edges(props: &StyleMap, style: &mut StyleSpec) {
            $(
                style.$edge_field =
                    edges_from(props, &$edges, |value| style_props!(@read $edge_kind, value));
            )*
        }
    };
}

style_props! {
    props {
        "display": Keyword(DisplaySpec) => style.display;
        "flex_direction": Keyword(FlexDirectionSpec) => style.flex_direction;
        "justify_content": Keyword(AlignSpec) => style.justify_content;
        "align_items": Keyword(AlignSpec) => style.align_items;
        "gap": NonNegativePx => style.gap;
        "width": NonNegativePxOrAuto => style.width;
        "height": NonNegativePxOrAuto => style.height;
        "border_width": NonNegativePx => style.border_width;
        "background": Color => style.background;
        "border_color": Color => style.border_color;
        "corner_radius": NonNegativePx => style.corner_radius;
        "text_color": Color => style.text_color;
        "text_size": FontSizePx => style.text_size;
        "overflow": KeywordPair(OverflowSpec) => overflow.all;
        "overflow_x": Keyword(OverflowSpec) => overflow.x;
        "overflow_y": Keyword(OverflowSpec) => overflow.y;
        "flex_grow": NonNegativeNumber => style.flex_grow;
        "flex_shrink": NonNegativeNumber => style.flex_shrink;
        "opacity": Opacity => style.opacity;
        "min_width": NonNegativePxOrAuto => style.min_width;
        "min_height": NonNegativePxOrAuto => style.min_height;
        "max_width": NonNegativePxOrAuto => style.max_width;
        "max_height": NonNegativePxOrAuto => style.max_height;
    }
    edges {
        PADDING_KEYS: NonNegativePx => padding {
            all: "padding",
            x: "padding_x",
            y: "padding_y",
            top: "padding_top",
            right: "padding_right",
            bottom: "padding_bottom",
            left: "padding_left",
        }
        MARGIN_KEYS: PxOrAuto => margin {
            all: "margin",
            x: "margin_x",
            y: "margin_y",
            top: "margin_top",
            right: "margin_right",
            bottom: "margin_bottom",
            left: "margin_left",
        }
    }
}

/// The canonical CSS text of the style prop `key` set to `value`, computed from the value the
/// spec reads. `None` when the prop has no such value: a bool, an empty string, an unknown key
/// and a value the reader of the kind rejects.
pub(crate) fn css_declaration(key: &str, value: &AttributeValue) -> Option<String> {
    let kind = STYLE_KEYS.iter().find(|entry| entry.key == key)?.kind;
    match kind {
        ValueKind::NonNegativePx => non_negative_px(value).map(px_text),
        ValueKind::FontSizePx => font_size_px(value).map(px_text),
        ValueKind::PxOrAuto => margin_length_from(value).map(length_text),
        ValueKind::NonNegativePxOrAuto => size_spec_from(value).map(length_text),
        ValueKind::NonNegativeNumber => non_negative(value).map(shortest_number_text),
        ValueKind::Opacity => opacity_value(value).map(shortest_number_text),
        ValueKind::Keyword(names) => value.as_str().and_then(|name| keyword_text(names, name)),
        ValueKind::KeywordPair(names) => {
            let text = value.as_str()?;
            overflow_pair_from_str(text)?;
            let mut words = text
                .split_ascii_whitespace()
                .map(|word| keyword_text(names, word));
            let x = words.next()??;
            let y = words.next().unwrap_or_else(|| Some(x.clone()))?;
            Some(if x == y { x } else { format!("{x} {y}") })
        }
        ValueKind::Color => match value {
            AttributeValue::String(text)
                if text.trim_ascii().eq_ignore_ascii_case("transparent") =>
            {
                Some("transparent".to_owned())
            }
            _ => as_color(value).map(rgb_text),
        },
    }
}

fn px_text(n: f64) -> String {
    format!("{}px", shortest_number_text(n))
}

fn length_text(length: LengthSpec) -> String {
    match length {
        LengthSpec::Px(n) => px_text(n),
        LengthSpec::Auto => "auto".to_owned(),
    }
}

/// The entry of `names` that `word` matches without ASCII case, written with `-`.
fn keyword_text(names: &[&str], word: &str) -> Option<String> {
    names
        .iter()
        .find(|name| name.eq_ignore_ascii_case(word))
        .map(|name| name.replace('_', "-"))
}

/// The message for a style key that no prop is named by.
pub(crate) fn unknown_key_warning(key: &str) -> String {
    format!("ignoring unknown style key `{key}`")
}

/// The message for the style prop `key` set to `value` when the key is
/// unknown, a color is invalid, or the value has a shape the key does not
/// support. `key` may be `camelCase`, `kebab-case` or `snake_case`, a style key
/// or its CSS name, and the message names it as written. `None` when the prop
/// applies, and for an empty string on a known key.
#[must_use]
pub fn style_warning(key: &str, value: &AttributeValue) -> Option<String> {
    let fault = read_prop(
        &mut StyleSpec::default(),
        &mut Overflows::default(),
        &style_key(key),
        value,
    )?;
    // A script resets a property with '' (`shown ? '' : 'none'`).
    if !matches!(fault, StyleFault::UnknownKey) && value.as_str() == Some("") {
        return None;
    }
    Some(match fault {
        StyleFault::UnknownKey => unknown_key_warning(key),
        StyleFault::InvalidColor => format!("ignoring invalid color for style key `{key}`"),
        StyleFault::UnsupportedValue => format!("ignoring unsupported value for style key `{key}`"),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::tests::*;
    use crate::{ColorSpec, EdgesSpec, LengthSpec, StyleCache, build_spec};
    use crate::{remove_style, set_style};
    use inca_node::VirtualTree;
    use rstest::rstest;

    /// Values `kind` accepts, chosen so that a reader of another kind with the
    /// same result type rejects at least one of them.
    fn sample_values(kind: ValueKind) -> Vec<AttributeValue> {
        match kind {
            ValueKind::NonNegativePx | ValueKind::FontSizePx => vec![num(1.0)],
            ValueKind::NonNegativePxOrAuto => vec![text("auto")],
            ValueKind::PxOrAuto | ValueKind::Opacity => vec![num(-1.0)],
            ValueKind::NonNegativeNumber => vec![num(2.0)],
            ValueKind::Color => vec![text("#fff")],
            ValueKind::Keyword(names) => names.iter().map(|name| text(name)).collect(),
            ValueKind::KeywordPair(names) => vec![text(&format!("{} {}", names[0], names[1]))],
        }
    }

    #[test]
    fn a_removed_style_key_renders_as_the_default() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        set_style(&mut tree, id, "gap", 8.0).unwrap();
        set_style(&mut tree, id, "background", f64::from(0x505050)).unwrap();
        remove_style(&mut tree, id, "gap").unwrap();
        remove_style(&mut tree, id, "background").unwrap();

        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id).unwrap().style,
            StyleSpec::default()
        );
    }

    #[test]
    fn recognized_style_keys_are_mapped() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        set_style(&mut tree, id, "display", "flex").unwrap();
        set_style(&mut tree, id, "flex_direction", "column").unwrap();
        set_style(&mut tree, id, "justify_content", "center").unwrap();
        set_style(&mut tree, id, "align_items", "stretch").unwrap();
        set_style(&mut tree, id, "gap", 8.0).unwrap();
        set_style(&mut tree, id, "width", 120.0).unwrap();
        set_style(&mut tree, id, "height", "auto").unwrap();
        set_style(&mut tree, id, "border_width", 1.0).unwrap();
        set_style(&mut tree, id, "background", f64::from(0x505050)).unwrap();
        set_style(&mut tree, id, "border_color", f64::from(0x0000ff)).unwrap();
        set_style(&mut tree, id, "corner_radius", 4.0).unwrap();
        set_style(&mut tree, id, "text_color", f64::from(0xffffff)).unwrap();
        set_style(&mut tree, id, "text_size", 20.0).unwrap();

        let spec = build_spec(&tree, &StyleCache::new(), id).unwrap();
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
    fn removing_a_side_key_falls_back_to_the_axis_then_the_shorthand() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        set_style(&mut tree, id, "margin", 1.0).unwrap();
        set_style(&mut tree, id, "margin_x", 2.0).unwrap();
        set_style(&mut tree, id, "margin_left", "auto").unwrap();
        let left = |tree: &VirtualTree| {
            build_spec(tree, &StyleCache::new(), id)
                .unwrap()
                .style
                .margin
                .left
        };
        assert_eq!(left(&tree), Some(LengthSpec::Auto));
        remove_style(&mut tree, id, "margin_left").unwrap();
        assert_eq!(left(&tree), Some(LengthSpec::Px(2.0)));
        remove_style(&mut tree, id, "margin_x").unwrap();
        assert_eq!(left(&tree), Some(LengthSpec::Px(1.0)));
        remove_style(&mut tree, id, "margin").unwrap();
        assert_eq!(left(&tree), None);
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

    #[test]
    fn a_removed_overflow_key_renders_as_the_default() {
        let mut tree = VirtualTree::new();
        let id = tree.create_node("div").unwrap();
        set_style(&mut tree, id, "overflow", "scroll").unwrap();
        remove_style(&mut tree, id, "overflow").unwrap();
        assert_eq!(
            build_spec(&tree, &StyleCache::new(), id).unwrap().style,
            StyleSpec::default()
        );
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
    fn a_known_key_with_an_unsupported_value_warns(
        #[case] key: &str,
        #[case] value: AttributeValue,
    ) {
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

    #[rstest]
    #[case::px("width", num(5.0), Some("5px"))]
    #[case::negative_px("margin", num(-2.5), Some("-2.5px"))]
    #[case::px_text("width", text("10PX"), Some("10px"))]
    #[case::unitless_zero("width", text("0"), Some("0px"))]
    #[case::negative_zero("width", text("-0px"), Some("0px"))]
    #[case::auto("width", text("AUTO"), Some("auto"))]
    #[case::margin_auto("margin_left", text("auto"), Some("auto"))]
    #[case::decimal("flex_grow", num(2.0), Some("2"))]
    #[case::opacity("opacity", num(0.5), Some("0.5"))]
    #[case::opacity_beyond_one("opacity", num(2.0), Some("2"))]
    #[case::opacity_negative("opacity", text("-1"), Some("-1"))]
    #[case::opacity_dot_before_exponent("opacity", text("1.e2"), None)]
    #[case::opacity_overflowing_text("opacity", text("1e999"), Some("3.4028234663852886e+38"))]
    #[case::opacity_negative_overflowing_text(
        "opacity",
        text("-1e999"),
        Some("-3.4028234663852886e+38")
    )]
    #[case::opacity_number_above_f32_max("opacity", num(1e300), Some("1e+300"))]
    #[case::opacity_percent("opacity", text("50%"), Some("0.5"))]
    #[case::opacity_exponent_percent("opacity", text("5e1%"), Some("0.5"))]
    #[case::opacity_percent_beyond_one("opacity", text("150%"), Some("1.5"))]
    #[case::opacity_negative_percent("opacity", text("-50%"), Some("-0.5"))]
    #[case::opacity_small_percent("opacity", text("0.7%"), Some("0.007"))]
    #[case::opacity_rounded_percent("opacity", text("12.3%"), Some("0.123"))]
    #[case::opacity_infinity("opacity", num(f64::INFINITY), None)]
    #[case::opacity_negative_infinity("opacity", num(f64::NEG_INFINITY), None)]
    #[case::keyword_case("display", text("FLEX"), Some("flex"))]
    #[case::keyword_underscore("flex_direction", text("ROW_REVERSE"), Some("row-reverse"))]
    #[case::keyword_hyphen("flex_direction", text("row-reverse"), Some("row-reverse"))]
    #[case::keyword_auto("overflow_x", text("auto"), Some("auto"))]
    #[case::pair_equal("overflow", text("hidden  Hidden"), Some("hidden"))]
    #[case::pair_single("overflow", text("scroll"), Some("scroll"))]
    #[case::pair_unequal("overflow", text("HIDDEN scroll"), Some("hidden scroll"))]
    #[case::color_hex("text_color", text("#FFF"), Some("rgb(255, 255, 255)"))]
    #[case::color_hex_alpha("text_color", text("#ff000080"), Some("rgba(255, 0, 0, 0.5)"))]
    #[case::color_function("background", text("rgb(1 2 3)"), Some("rgb(1, 2, 3)"))]
    #[case::color_function_alpha(
        "background",
        text("rgb(0 0 0 / 50%)"),
        Some("rgba(0, 0, 0, 0.5)")
    )]
    #[case::color_number(
        "text_color",
        num(f64::from(0x00ab_cdef_u32)),
        Some("rgb(171, 205, 239)")
    )]
    #[case::color_transparent("border_color", text("TRANSPARENT"), Some("transparent"))]
    #[case::color_transparent_padded("border_color", text(" transparent "), Some("transparent"))]
    #[case::color_transparent_hex("border_color", text("#0000"), Some("rgba(0, 0, 0, 0)"))]
    #[case::bool("width", AttributeValue::Bool(true), None)]
    #[case::empty_string("width", text(""), None)]
    #[case::flagged_string("width", text("bogus"), None)]
    #[case::overflowing_length("width", text("1e999px"), Some("33554428px"))]
    #[case::length_above_the_limit("width", text("99999999px"), Some("33554428px"))]
    #[case::negative_overflowing_margin("margin_top", text("-1e999px"), Some("-33554428px"))]
    #[case::overflowing_corner_radius("corner_radius", text("1e999px"), Some("33554428px"))]
    #[case::overflowing_flex_factor("flex_grow", text("1e999"), Some("1000000000000000000"))]
    #[case::overflowing_font_size("text_size", text("1e999px"), Some("2048px"))]
    #[case::font_size_above_the_limit("text_size", text("4096px"), Some("2048px"))]
    #[case::flagged_number("display", num(1.0), None)]
    #[case::unknown_key("not_a_key", num(1.0), None)]
    fn css_declaration_writes_the_value_by_its_kind(
        #[case] key: &str,
        #[case] value: AttributeValue,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(css_declaration(key, &value).as_deref(), expected);
    }

    #[test]
    fn the_key_list_names_the_keys_the_reader_knows() {
        let source = include_str!("keys.rs");
        let rows = source
            .split_once("\nstyle_props! {\n")
            .and_then(|(_, rest)| rest.split_once("\n}\n"))
            .unwrap()
            .0;
        let keys: Vec<&str> = rows.split('"').skip(1).step_by(2).collect();
        assert!(!keys.is_empty());
        for key in keys {
            assert!(STYLE_KEYS.iter().any(|entry| entry.key == key), "{key}");
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
}
