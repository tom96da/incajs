// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;

use super::spec::{
    AlignSpec, DisplaySpec, EdgesSpec, FlexDirectionSpec, LengthSpec, OverflowSpec, StyleSpec,
};
use crate::tree::{AttributeValue, style_key};

/// Parses a `0x000000..=0xFFFFFF` number or a `"#rrggbb"`/`"#rgb"` string into
/// a `0xRRGGBB` color. Any other value (a number outside that range, `NaN`,
/// wrong hex digit count, missing `#`, non-hex characters, a bool, ...) is
/// ignored.
fn as_color(value: &AttributeValue) -> Option<u32> {
    match value {
        AttributeValue::Number(n) if (0.0..=f64::from(0x00FF_FFFF_u32)).contains(n) => {
            // The range check above keeps the cast inside `u32`.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(*n as u32)
        }
        AttributeValue::String(s) => parse_hex_color(s),
        _ => None,
    }
}

fn parse_hex_color(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok(),
        3 => {
            let mut expanded = String::with_capacity(6);
            for c in hex.chars() {
                expanded.push(c);
                expanded.push(c);
            }
            u32::from_str_radix(&expanded, 16).ok()
        }
        _ => None,
    }
}

/// `"auto"` is an alias of `"scroll"`.
fn overflow_spec_from_str(s: &str) -> Option<OverflowSpec> {
    match s {
        "visible" => Some(OverflowSpec::Visible),
        "hidden" => Some(OverflowSpec::Hidden),
        "clip" => Some(OverflowSpec::Clip),
        "scroll" | "auto" => Some(OverflowSpec::Scroll),
        _ => None,
    }
}

/// One or two overflow values, x first; a single value applies to both axes.
fn overflow_pair_from_str(s: &str) -> Option<(OverflowSpec, OverflowSpec)> {
    let mut values = s.split_ascii_whitespace().map(overflow_spec_from_str);
    let x = values.next()??;
    let y = values.next().unwrap_or(Some(x))?;
    values.next().is_none().then_some((x, y))
}

/// The computed axes: beside an axis that is `hidden` or scrolls, an unset or `visible` axis
/// scrolls and a `clip` axis is `hidden`.
fn computed_axes(
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

fn length_spec_from(value: &AttributeValue) -> Option<LengthSpec> {
    match value {
        AttributeValue::Number(n) => Some(LengthSpec::Px(*n)),
        AttributeValue::String(s) if s == "auto" => Some(LengthSpec::Auto),
        _ => None,
    }
}

/// A finite px number `>= 0`; negatives, `NaN` and infinities are unset.
fn non_negative(value: &AttributeValue) -> Option<f64> {
    value.as_number().filter(|n| n.is_finite() && *n >= 0.0)
}

/// A finite px number or `"auto"`; `NaN` and infinities are unset.
fn margin_length_from(value: &AttributeValue) -> Option<LengthSpec> {
    length_spec_from(value).filter(|l| !matches!(l, LengthSpec::Px(n) if !n.is_finite()))
}

/// `"auto"` or a finite px number `>= 0`; negatives, `NaN` and infinities are unset.
fn size_spec_from(value: &AttributeValue) -> Option<LengthSpec> {
    match value {
        AttributeValue::String(s) if s == "auto" => Some(LengthSpec::Auto),
        _ => non_negative(value).map(LengthSpec::Px),
    }
}

/// Keys of one box property, most general first: the shorthand, the two
/// axes, then the four sides.
struct EdgeKeys {
    all: &'static str,
    x: &'static str,
    y: &'static str,
    top: &'static str,
    right: &'static str,
    bottom: &'static str,
    left: &'static str,
}

impl EdgeKeys {
    fn contains(&self, key: &str) -> bool {
        [
            self.all,
            self.x,
            self.y,
            self.top,
            self.right,
            self.bottom,
            self.left,
        ]
        .contains(&key)
    }
}

const PADDING_KEYS: EdgeKeys = EdgeKeys {
    all: "padding",
    x: "padding_x",
    y: "padding_y",
    top: "padding_top",
    right: "padding_right",
    bottom: "padding_bottom",
    left: "padding_left",
};

const MARGIN_KEYS: EdgeKeys = EdgeKeys {
    all: "margin",
    x: "margin_x",
    y: "margin_y",
    top: "margin_top",
    right: "margin_right",
    bottom: "margin_bottom",
    left: "margin_left",
};

/// Resolves each side from the side key, else the axis key, else the
/// shorthand. A value `parse` rejects counts as unset and falls through to
/// the next key.
fn edges_from<T>(
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

fn display_spec_from_str(s: &str) -> Option<DisplaySpec> {
    match s {
        "flex" => Some(DisplaySpec::Flex),
        "block" => Some(DisplaySpec::Block),
        "grid" => Some(DisplaySpec::Grid),
        "none" => Some(DisplaySpec::None),
        _ => None,
    }
}

fn flex_direction_spec_from_str(s: &str) -> Option<FlexDirectionSpec> {
    match s {
        "row" => Some(FlexDirectionSpec::Row),
        "column" => Some(FlexDirectionSpec::Column),
        "row_reverse" => Some(FlexDirectionSpec::RowReverse),
        "column_reverse" => Some(FlexDirectionSpec::ColumnReverse),
        _ => None,
    }
}

fn align_spec_from_str(s: &str) -> Option<AlignSpec> {
    match s {
        "start" => Some(AlignSpec::Start),
        "end" => Some(AlignSpec::End),
        "center" => Some(AlignSpec::Center),
        "stretch" => Some(AlignSpec::Stretch),
        _ => None,
    }
}

/// A style prop the spec layer drops, which [`style_warning`] reports.
enum StyleFault {
    UnknownKey,
    InvalidColor,
    /// A value of a shape the host does not support for its known key.
    UnsupportedValue,
}

/// The three overflow keys, resolved once every prop has been read.
#[derive(Default)]
struct Overflows {
    /// `overflow`, as its x and y values.
    all: Option<(OverflowSpec, OverflowSpec)>,
    x: Option<OverflowSpec>,
    y: Option<OverflowSpec>,
}

/// Stores `parsed` in `slot`, and returns `fault` when it is `None`.
fn store<T>(slot: &mut Option<T>, parsed: Option<T>, fault: StyleFault) -> Option<StyleFault> {
    *slot = parsed;
    slot.is_none().then_some(fault)
}

/// Reads one style prop into `style` (and `overflow`), returning the fault
/// when the key is unknown or its value is dropped. A box key is only checked
/// here: `edges_from` resolves the box keys afterwards.
fn read_prop(
    style: &mut StyleSpec,
    overflow: &mut Overflows,
    key: &str,
    value: &AttributeValue,
) -> Option<StyleFault> {
    use StyleFault::{InvalidColor, UnsupportedValue};
    let text = value.as_str();
    match key {
        "display" => store(
            &mut style.display,
            text.and_then(display_spec_from_str),
            UnsupportedValue,
        ),
        "flex_direction" => store(
            &mut style.flex_direction,
            text.and_then(flex_direction_spec_from_str),
            UnsupportedValue,
        ),
        "justify_content" => store(
            &mut style.justify_content,
            text.and_then(align_spec_from_str),
            UnsupportedValue,
        ),
        "align_items" => store(
            &mut style.align_items,
            text.and_then(align_spec_from_str),
            UnsupportedValue,
        ),
        "gap" => store(&mut style.gap, non_negative(value), UnsupportedValue),
        "width" => store(&mut style.width, size_spec_from(value), UnsupportedValue),
        "height" => store(&mut style.height, size_spec_from(value), UnsupportedValue),
        "border_width" => store(
            &mut style.border_width,
            non_negative(value),
            UnsupportedValue,
        ),
        "background" => store(&mut style.background, as_color(value), InvalidColor),
        "border_color" => store(&mut style.border_color, as_color(value), InvalidColor),
        "corner_radius" => store(
            &mut style.corner_radius,
            non_negative(value),
            UnsupportedValue,
        ),
        "text_color" => store(&mut style.text_color, as_color(value), InvalidColor),
        "text_size" => store(&mut style.text_size, non_negative(value), UnsupportedValue),
        "overflow" => store(
            &mut overflow.all,
            text.and_then(overflow_pair_from_str),
            UnsupportedValue,
        ),
        "overflow_x" => store(
            &mut overflow.x,
            text.and_then(overflow_spec_from_str),
            UnsupportedValue,
        ),
        "overflow_y" => store(
            &mut overflow.y,
            text.and_then(overflow_spec_from_str),
            UnsupportedValue,
        ),
        "flex_grow" => store(&mut style.flex_grow, non_negative(value), UnsupportedValue),
        "flex_shrink" => store(
            &mut style.flex_shrink,
            non_negative(value),
            UnsupportedValue,
        ),
        "opacity" => store(
            &mut style.opacity,
            value
                .as_number()
                .filter(|n| !n.is_nan())
                .map(|n| n.clamp(0.0, 1.0)),
            UnsupportedValue,
        ),
        "min_width" => store(
            &mut style.min_width,
            size_spec_from(value),
            UnsupportedValue,
        ),
        "min_height" => store(
            &mut style.min_height,
            size_spec_from(value),
            UnsupportedValue,
        ),
        "max_width" => store(
            &mut style.max_width,
            size_spec_from(value),
            UnsupportedValue,
        ),
        "max_height" => store(
            &mut style.max_height,
            size_spec_from(value),
            UnsupportedValue,
        ),
        _ if PADDING_KEYS.contains(key) => {
            non_negative(value).is_none().then_some(UnsupportedValue)
        }
        _ if MARGIN_KEYS.contains(key) => margin_length_from(value)
            .is_none()
            .then_some(UnsupportedValue),
        _ => Some(StyleFault::UnknownKey),
    }
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
    style.padding = edges_from(props, &PADDING_KEYS, non_negative);
    style.margin = edges_from(props, &MARGIN_KEYS, margin_length_from);
    style
}

/// The message for the style prop `key` set to `value` when the key is
/// unknown, a color is invalid, or the value has a shape the host does not
/// support. `key` may be `camelCase`, `kebab-case` or `snake_case`, and the
/// message names it as written. `None` when the prop applies, and for an empty
/// string on a known key.
#[must_use]
pub fn style_warning(key: &str, value: &AttributeValue) -> Option<String> {
    // A caller checks each prop as it stores it, so a bad value that stays
    // in place is reported once and setting it again reports it again.
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
        StyleFault::UnknownKey => format!("ignoring unknown style key `{key}`"),
        StyleFault::InvalidColor => format!("ignoring invalid color for style key `{key}`"),
        StyleFault::UnsupportedValue => format!("ignoring unsupported value for style key `{key}`"),
    })
}
