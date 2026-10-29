// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::borrow::Cow;
use std::collections::HashMap;

use super::color::as_color;
use super::spec::{
    AlignSpec, DisplaySpec, EdgesSpec, FlexDirectionSpec, LengthSpec, OverflowSpec, StyleSpec,
};
use crate::tree::{AttributeValue, VirtualNode};

/// One or two overflow values, x first; a single value applies to both axes.
fn overflow_pair_from_str(s: &str) -> Option<(OverflowSpec, OverflowSpec)> {
    let mut values = s.split_ascii_whitespace().map(OverflowSpec::from_name);
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
fn number(value: &AttributeValue) -> Option<f64> {
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
fn style_px(value: &AttributeValue) -> Option<f64> {
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
fn non_negative(value: &AttributeValue) -> Option<f64> {
    number(value).filter(|n| n.is_finite() && *n >= 0.0)
}

/// A finite px value `>= 0`; negatives, `NaN` and infinities are unset.
fn non_negative_px(value: &AttributeValue) -> Option<f64> {
    style_px(value).filter(|n| n.is_finite() && *n >= 0.0)
}

/// `"auto"` or a px value.
fn length_spec_from(value: &AttributeValue) -> Option<LengthSpec> {
    match value {
        AttributeValue::String(s) if s == "auto" => Some(LengthSpec::Auto),
        _ => style_px(value).map(LengthSpec::Px),
    }
}

/// A finite px value or `"auto"`; `NaN` and infinities are unset.
fn margin_length_from(value: &AttributeValue) -> Option<LengthSpec> {
    length_spec_from(value).filter(|l| !matches!(l, LengthSpec::Px(n) if !n.is_finite()))
}

/// `"auto"` or a finite px value `>= 0`; negatives, `NaN` and infinities are
/// unset.
fn size_spec_from(value: &AttributeValue) -> Option<LengthSpec> {
    match value {
        AttributeValue::String(s) if s == "auto" => Some(LengthSpec::Auto),
        _ => non_negative_px(value).map(LengthSpec::Px),
    }
}

/// A decimal value clamped to `0..=1`; `NaN` is unset.
fn opacity(value: &AttributeValue) -> Option<f64> {
    number(value)
        .filter(|n| !n.is_nan())
        .map(|n| n.clamp(0.0, 1.0))
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

/// The value a style key takes. A value the kind does not accept is dropped
/// with a warning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueKind {
    /// A finite px value `>= 0`.
    NonNegativePx,
    /// A finite px value `>= 0`, or `"auto"`.
    NonNegativePxOrAuto,
    /// A finite px value, or `"auto"`.
    PxOrAuto,
    /// A finite decimal value `>= 0`.
    NonNegativeNumber,
    /// A decimal value, clamped to `0..=1`.
    Opacity,
    /// A number from `0` to `0xffffff`, or a string: `#rgb`, `#rgba`, `#rrggbb`,
    /// `#rrggbbaa`, `transparent`, `rgb()` or `rgba()`.
    Color,
    /// One of the names.
    Keyword(&'static [&'static str]),
    /// One or two of the names separated by whitespace: x, then y.
    KeywordPair(&'static [&'static str]),
}

/// A style key the host reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleKey {
    /// The host key, in `snake_case`.
    pub key: &'static str,
    /// The CSS names of the same prop, in `snake_case`.
    pub aliases: &'static [&'static str],
    /// The value the key takes.
    pub kind: ValueKind,
}

/// Expands the style declaration below into [`STYLE_KEYS`], `read_prop`,
/// `alias_target`, `read_edges` and one [`EdgeKeys`] const per box-edge family.
macro_rules! style_props {
    (@kind Keyword($ty:ident)) => { ValueKind::Keyword($ty::NAMES) };
    (@kind KeywordPair($ty:ident)) => { ValueKind::KeywordPair($ty::NAMES) };
    (@kind $kind:ident) => { ValueKind::$kind };
    (@read NonNegativePx, $value:ident) => { non_negative_px($value) };
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
                $key:literal [$($alias:literal),*]: $kind:ident $(($arg:ident))?
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
        /// Every style key the host reads, in `snake_case`, with its CSS names and
        /// the value it takes.
        pub const STYLE_KEYS: &[StyleKey] = &[
            $(
                StyleKey {
                    key: $key,
                    aliases: &[$($alias),*],
                    kind: style_props!(@kind $kind $(($arg))?),
                },
            )*
            $(
                StyleKey { key: $all, aliases: &[], kind: ValueKind::$edge_kind },
                StyleKey { key: $x, aliases: &[], kind: ValueKind::$edge_kind },
                StyleKey { key: $y, aliases: &[], kind: ValueKind::$edge_kind },
                StyleKey { key: $top, aliases: &[], kind: ValueKind::$edge_kind },
                StyleKey { key: $right, aliases: &[], kind: ValueKind::$edge_kind },
                StyleKey { key: $bottom, aliases: &[], kind: ValueKind::$edge_kind },
                StyleKey { key: $left, aliases: &[], kind: ValueKind::$edge_kind },
            )*
        ];

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
        /// when the key is unknown or its value is dropped. `key` is a host key. A
        /// box key is only checked here: `edges_from` resolves the box keys
        /// afterwards.
        fn read_prop(
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
        fn read_edges(props: &HashMap<String, AttributeValue>, style: &mut StyleSpec) {
            $(
                style.$edge_field =
                    edges_from(props, &$edges, |value| style_props!(@read $edge_kind, value));
            )*
        }

        /// The host key that `key`, a CSS name in `snake_case`, stands for.
        fn alias_target(key: &str) -> Option<&'static str> {
            match key {
                $($($alias => Some($key),)*)*
                _ => None,
            }
        }
    };
}

style_props! {
    props {
        "display" []: Keyword(DisplaySpec) => style.display;
        "flex_direction" []: Keyword(FlexDirectionSpec) => style.flex_direction;
        "justify_content" []: Keyword(AlignSpec) => style.justify_content;
        "align_items" []: Keyword(AlignSpec) => style.align_items;
        "gap" []: NonNegativePx => style.gap;
        "width" []: NonNegativePxOrAuto => style.width;
        "height" []: NonNegativePxOrAuto => style.height;
        "border_width" []: NonNegativePx => style.border_width;
        "background" ["background_color"]: Color => style.background;
        "border_color" []: Color => style.border_color;
        "corner_radius" ["border_radius"]: NonNegativePx => style.corner_radius;
        "text_color" ["color"]: Color => style.text_color;
        "text_size" ["font_size"]: NonNegativePx => style.text_size;
        "overflow" []: KeywordPair(OverflowSpec) => overflow.all;
        "overflow_x" []: Keyword(OverflowSpec) => overflow.x;
        "overflow_y" []: Keyword(OverflowSpec) => overflow.y;
        "flex_grow" []: NonNegativeNumber => style.flex_grow;
        "flex_shrink" []: NonNegativeNumber => style.flex_shrink;
        "opacity" []: Opacity => style.opacity;
        "min_width" []: NonNegativePxOrAuto => style.min_width;
        "min_height" []: NonNegativePxOrAuto => style.min_height;
        "max_width" []: NonNegativePxOrAuto => style.max_width;
        "max_height" []: NonNegativePxOrAuto => style.max_height;
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

/// The host key of a style key. `key` may be `camelCase`, `kebab-case` or
/// `snake_case`, and a CSS name listed in [`STYLE_KEYS`] names its host key.
/// A key that is already a host key in `snake_case` comes back borrowed.
pub(crate) fn style_key(key: &str) -> Cow<'_, str> {
    let snake = if key.bytes().any(|b| b.is_ascii_uppercase() || b == b'-') {
        let mut out = String::with_capacity(key.len() + 2);
        for c in key.chars() {
            match c {
                '-' => out.push('_'),
                c if c.is_ascii_uppercase() => {
                    out.push('_');
                    out.push(c.to_ascii_lowercase());
                }
                c => out.push(c),
            }
        }
        Cow::Owned(out)
    } else {
        Cow::Borrowed(key)
    };
    match alias_target(&snake) {
        Some(key) => Cow::Borrowed(key),
        None => snake,
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
    read_edges(props, &mut style);
    style
}

/// The window size an element's style declares, in px: `(width, height)`.
/// A dimension that is unset, `"auto"`, a percentage or not a finite size
/// `>= 0` is `None`.
#[must_use]
pub fn declared_window_size(node: &VirtualNode) -> (Option<f64>, Option<f64>) {
    let style = style_spec_from_props(node.style_props());
    let px = |length| match length {
        Some(LengthSpec::Px(n)) => Some(n),
        _ => None,
    };
    (px(style.width), px(style.height))
}

/// The message for the style prop `key` set to `value` when the key is
/// unknown, a color is invalid, or the value has a shape the host does not
/// support. `key` may be `camelCase`, `kebab-case` or `snake_case`, a host name
/// or its CSS name, and the message names it as written. `None` when the prop
/// applies, and for an empty string on a known key.
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
