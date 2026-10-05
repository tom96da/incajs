// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Converts a retained [`VirtualTree`] into real `gpui` elements.
//!
//! Split into two layers:
//! - a pure spec layer ([`ElementSpec`]/[`StyleSpec`]/[`build_spec`]) that
//!   has no `gpui` dependency and is exhaustively unit-testable on its own;
//! - a thin gpui layer ([`build_element`]/[`render_tree`]) that turns that
//!   spec into a real [`AnyElement`].
//!
//! Unrecognized `style_props` keys, malformed values and unrecognized
//! `attributes` keys are ignored. This runs on the render path, where nothing
//! can raise a catchable exception. [`style_warning`] lets the caller that
//! stores a style report an unknown key or an invalid colour once, when it is
//! set.

use std::collections::HashMap;
use std::fmt;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, DispatchPhase, Display, ElementId, Fill, FlexDirection, Global, Hsla, Length,
    MouseExitEvent, MouseMoveEvent, Overflow, Pixels, Point, ScrollHandle, ScrollWheelEvent,
    StyleRefinement, Window, canvas, div, point, px, rgb,
};

use crate::event_sink::{EventMask, EventPayload, EventSink, MousePayload};
use crate::tree::{AttributeValue, NodeId, VirtualTree};

/// What kind of element a [`VirtualNode`](crate::tree::VirtualNode) maps to.
///
/// Only two kinds exist for now — there's no per-tag dispatch table, since
/// there's exactly one container builder to pick from until a real second
/// element kind is designed.
///
/// Text is deliberately never allowed as a bare string child mixed into a
/// container's children — it's always its own dedicated node with a
/// stable id. That keeps every rendered text run addressable by [`NodeId`],
/// which later work (text selection, hit-testing, per-run event handling)
/// will need — a container that could also hold ad-hoc string children
/// would make some rendered text invisible to that addressing.
#[derive(Debug, Clone, PartialEq)]
pub enum ElementTag {
    /// Any tag name other than `"text"`: a generic styled box.
    Container,
    /// A `"text"` tag: its `"value"` attribute (missing or non-string →
    /// empty) followed by its descendants' text in child order. Descendants
    /// are not rendered as separate elements.
    Text(String),
}

/// A length in pixels, or `"auto"`. `gpui`-independent mirror of the subset
/// of [`gpui::Length`] this layer supports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthSpec {
    Px(f64),
    Auto,
}

/// `gpui`-independent mirror of [`gpui::Display`]'s supported variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplaySpec {
    Flex,
    Block,
    Grid,
    None,
}

/// `gpui`-independent mirror of [`gpui::FlexDirection`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlexDirectionSpec {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// `gpui`-independent mirror of the common subset of `gpui`'s `AlignItems`
/// and `JustifyContent` (`= AlignContent`) enums — the variants both share
/// and that this layer supports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlignSpec {
    Start,
    End,
    Center,
    Stretch,
}

/// `gpui`-independent mirror of [`gpui::Overflow`]'s supported variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverflowSpec {
    Visible,
    Hidden,
    Scroll,
}

/// One value per side of a box. `None` leaves that side at its default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgesSpec<T> {
    pub top: Option<T>,
    pub right: Option<T>,
    pub bottom: Option<T>,
    pub left: Option<T>,
}

impl<T> Default for EdgesSpec<T> {
    fn default() -> Self {
        Self {
            top: None,
            right: None,
            bottom: None,
            left: None,
        }
    }
}

/// A plain-data, `gpui`-independent style description, built from a
/// [`VirtualNode`](crate::tree::VirtualNode)'s `style_props` — see
/// `style_spec_from_props`'s match arms for the exact recognized keys.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleSpec {
    pub display: Option<DisplaySpec>,
    pub flex_direction: Option<FlexDirectionSpec>,
    pub justify_content: Option<AlignSpec>,
    pub align_items: Option<AlignSpec>,
    /// Uniform gap (px) applied to both rows and columns.
    pub gap: Option<f64>,
    pub width: Option<LengthSpec>,
    pub height: Option<LengthSpec>,
    /// Uniform border width (px) applied to all four sides.
    pub border_width: Option<f64>,
    /// `0xRRGGBB`.
    pub background: Option<u32>,
    /// `0xRRGGBB`.
    pub border_color: Option<u32>,
    /// Uniform corner radius (px) applied to all four corners.
    pub corner_radius: Option<f64>,
    /// `0xRRGGBB`. Cascades to descendant text, like `gpui`'s own
    /// `.text_color()` — text leaves carry no style of their own.
    pub text_color: Option<u32>,
    pub text_size: Option<f64>,
    pub overflow_x: Option<OverflowSpec>,
    pub overflow_y: Option<OverflowSpec>,
    /// Inner spacing (px), each side `>= 0`.
    pub padding: EdgesSpec<f64>,
    /// Outer spacing; each side a px number (negative allowed) or `"auto"`.
    pub margin: EdgesSpec<LengthSpec>,
    /// `>= 0`.
    pub flex_grow: Option<f64>,
    /// `>= 0`.
    pub flex_shrink: Option<f64>,
    /// `0.0..=1.0`; multiplies into descendants.
    pub opacity: Option<f64>,
    pub min_width: Option<LengthSpec>,
    pub min_height: Option<LengthSpec>,
    pub max_width: Option<LengthSpec>,
    pub max_height: Option<LengthSpec>,
}

impl StyleSpec {
    /// Whether either axis scrolls.
    fn scrolls(&self) -> bool {
        self.overflow_x == Some(OverflowSpec::Scroll)
            || self.overflow_y == Some(OverflowSpec::Scroll)
    }
}

/// A `gpui`-independent, recursive description of one
/// [`VirtualNode`](crate::tree::VirtualNode) and its subtree.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementSpec {
    pub id: NodeId,
    pub tag: ElementTag,
    pub style: StyleSpec,
    /// Every kind of event something is listening for on this node.
    pub listens: EventMask,
    pub children: Vec<ElementSpec>,
}

fn as_str(value: &AttributeValue) -> Option<&str> {
    match value {
        AttributeValue::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn as_number(value: &AttributeValue) -> Option<f64> {
    match value {
        AttributeValue::Number(n) => Some(*n),
        _ => None,
    }
}

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
        "scroll" | "auto" => Some(OverflowSpec::Scroll),
        _ => None,
    }
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
    as_number(value).filter(|n| n.is_finite() && *n >= 0.0)
}

/// A finite px number or `"auto"`; `NaN` and infinities are unset.
fn margin_length_from(value: &AttributeValue) -> Option<LengthSpec> {
    length_spec_from(value).filter(|l| !matches!(l, LengthSpec::Px(n) if !n.is_finite()))
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
enum StyleFault<'a> {
    UnknownKey(&'a str),
    InvalidColour(&'a str),
}

impl fmt::Display for StyleFault<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey(key) => write!(f, "ignoring unknown style key `{key}`"),
            Self::InvalidColour(key) => {
                write!(f, "ignoring invalid colour for style key `{key}`")
            }
        }
    }
}

/// The three overflow keys, resolved once every prop has been read.
#[derive(Default)]
struct Overflows {
    all: Option<OverflowSpec>,
    x: Option<OverflowSpec>,
    y: Option<OverflowSpec>,
}

/// Reads the colour `value` into `slot`.
fn read_colour<'a>(
    slot: &mut Option<u32>,
    key: &'a str,
    value: &AttributeValue,
) -> Option<StyleFault<'a>> {
    *slot = as_color(value);
    slot.is_none().then_some(StyleFault::InvalidColour(key))
}

/// Reads one style prop into `style` (and `overflow`), returning the fault
/// when the key is unknown or a colour is invalid. Box keys are resolved by
/// `edges_from` afterwards.
fn read_prop<'a>(
    style: &mut StyleSpec,
    overflow: &mut Overflows,
    key: &'a str,
    value: &AttributeValue,
) -> Option<StyleFault<'a>> {
    match key {
        "display" => style.display = as_str(value).and_then(display_spec_from_str),
        "flex_direction" => {
            style.flex_direction = as_str(value).and_then(flex_direction_spec_from_str);
        }
        "justify_content" => {
            style.justify_content = as_str(value).and_then(align_spec_from_str);
        }
        "align_items" => style.align_items = as_str(value).and_then(align_spec_from_str),
        "gap" => style.gap = as_number(value),
        "width" => style.width = length_spec_from(value),
        "height" => style.height = length_spec_from(value),
        "border_width" => style.border_width = as_number(value),
        "background" => return read_colour(&mut style.background, key, value),
        "border_color" => return read_colour(&mut style.border_color, key, value),
        "corner_radius" => style.corner_radius = as_number(value),
        "text_color" => return read_colour(&mut style.text_color, key, value),
        "text_size" => style.text_size = as_number(value),
        "overflow" => overflow.all = as_str(value).and_then(overflow_spec_from_str),
        "overflow_x" => overflow.x = as_str(value).and_then(overflow_spec_from_str),
        "overflow_y" => overflow.y = as_str(value).and_then(overflow_spec_from_str),
        "flex_grow" => style.flex_grow = non_negative(value),
        "flex_shrink" => style.flex_shrink = non_negative(value),
        "opacity" => {
            style.opacity = as_number(value)
                .filter(|n| !n.is_nan())
                .map(|n| n.clamp(0.0, 1.0));
        }
        "min_width" => style.min_width = length_spec_from(value),
        "min_height" => style.min_height = length_spec_from(value),
        "max_width" => style.max_width = length_spec_from(value),
        "max_height" => style.max_height = length_spec_from(value),
        _ if PADDING_KEYS.contains(key) || MARGIN_KEYS.contains(key) => {}
        _ => return Some(StyleFault::UnknownKey(key)),
    }
    None
}

/// Builds a [`StyleSpec`] from a node's raw `style_props`, ignoring any key
/// or value this layer doesn't (yet) recognize.
fn style_spec_from_props(props: &HashMap<String, AttributeValue>) -> StyleSpec {
    let mut style = StyleSpec::default();
    let mut overflow = Overflows::default();
    for (key, value) in props {
        read_prop(&mut style, &mut overflow, key, value);
    }
    // `props` iterates in arbitrary order, so precedence is resolved here.
    style.overflow_x = overflow.x.or(overflow.all);
    style.overflow_y = overflow.y.or(overflow.all);
    style.padding = edges_from(props, &PADDING_KEYS, non_negative);
    style.margin = edges_from(props, &MARGIN_KEYS, margin_length_from);
    style
}

/// The message for the style prop `key` set to `value` when the key is
/// unknown or a colour value is invalid. `None` when the prop applies or is
/// ignored quietly.
#[must_use]
pub fn style_warning(key: &str, value: &AttributeValue) -> Option<String> {
    // A caller checks each prop as it stores it, so a bad value that stays
    // in place is reported once and setting it again reports it again.
    read_prop(
        &mut StyleSpec::default(),
        &mut Overflows::default(),
        key,
        value,
    )
    .map(|fault| fault.to_string())
}

/// A node's `"value"` attribute followed by its descendants' text in child
/// order, whatever their tags. An unresolved child id is skipped.
fn text_content(tree: &VirtualTree, id: NodeId) -> String {
    let Some(node) = tree.get(id) else {
        return String::new();
    };
    let mut content = node
        .attributes()
        .get("value")
        .and_then(as_str)
        .unwrap_or_default()
        .to_owned();
    for &child_id in node.children() {
        content.push_str(&text_content(tree, child_id));
    }
    content
}

/// Builds an [`ElementSpec`] for `root` and its whole subtree, with nothing
/// listening for input.
#[must_use]
pub fn build_spec(tree: &VirtualTree, root: NodeId) -> Option<ElementSpec> {
    build_spec_with(tree, root, &|_| EventMask::NONE)
}

/// Builds an [`ElementSpec`] for `root` and its whole subtree, asking
/// `listens` about each node. `None` if `root` doesn't resolve, matching
/// [`VirtualTree::get`]'s convention. A child id that doesn't resolve is
/// skipped rather than panicking — this is ultimately fed by JS-supplied
/// data, so the render path stays defensive.
///
/// A predicate rather than the registry itself, so this layer stays free of
/// it.
#[must_use]
pub fn build_spec_with(
    tree: &VirtualTree,
    root: NodeId,
    listens: &dyn Fn(NodeId) -> EventMask,
) -> Option<ElementSpec> {
    let node = tree.get(root)?;

    let is_text = node.tag_name() == "text";
    let tag = if is_text {
        ElementTag::Text(text_content(tree, root))
    } else {
        ElementTag::Container
    };

    let style = style_spec_from_props(node.style_props());
    let children = if is_text {
        Vec::new()
    } else {
        node.children()
            .iter()
            .filter_map(|&child_id| build_spec_with(tree, child_id, listens))
            .collect()
    };

    Some(ElementSpec {
        id: root,
        tag,
        style,
        listens: listens(root),
        children,
    })
}

// f64 -> f32 for `gpui`'s `Pixels` type: real UI dimensions never carry
// enough precision or magnitude for this narrowing to matter.
#[allow(clippy::cast_possible_truncation)]
fn length_from_spec(spec: LengthSpec) -> Length {
    match spec {
        LengthSpec::Px(n) => px(n as f32).into(),
        LengthSpec::Auto => Length::Auto,
    }
}

/// Applies a [`StyleSpec`] onto a real `gpui` [`StyleRefinement`], by direct
/// field assignment rather than `gpui`'s named Tailwind-scale builder
/// methods (`.gap_3()`, `.size_8()`, ...) — those only cover fixed steps,
/// not the arbitrary numbers this spec carries.
// f64 -> f32 for `gpui`'s `Pixels` type: real UI dimensions never carry
// enough precision or magnitude for this narrowing to matter.
#[allow(clippy::cast_possible_truncation)]
fn apply_style(style: &mut StyleRefinement, spec: &StyleSpec) {
    if let Some(display) = spec.display {
        style.display = Some(match display {
            DisplaySpec::Flex => Display::Flex,
            DisplaySpec::Block => Display::Block,
            DisplaySpec::Grid => Display::Grid,
            DisplaySpec::None => Display::None,
        });
    }
    if let Some(direction) = spec.flex_direction {
        style.flex_direction = Some(match direction {
            FlexDirectionSpec::Row => FlexDirection::Row,
            FlexDirectionSpec::Column => FlexDirection::Column,
            FlexDirectionSpec::RowReverse => FlexDirection::RowReverse,
            FlexDirectionSpec::ColumnReverse => FlexDirection::ColumnReverse,
        });
    }
    if let Some(justify) = spec.justify_content {
        style.justify_content = Some(match justify {
            AlignSpec::Start => gpui::JustifyContent::Start,
            AlignSpec::End => gpui::JustifyContent::End,
            AlignSpec::Center => gpui::JustifyContent::Center,
            AlignSpec::Stretch => gpui::JustifyContent::Stretch,
        });
    }
    if let Some(align) = spec.align_items {
        style.align_items = Some(match align {
            AlignSpec::Start => gpui::AlignItems::Start,
            AlignSpec::End => gpui::AlignItems::End,
            AlignSpec::Center => gpui::AlignItems::Center,
            AlignSpec::Stretch => gpui::AlignItems::Stretch,
        });
    }
    if let Some(gap) = spec.gap {
        let gap = px(gap as f32);
        style.gap.width = Some(gap.into());
        style.gap.height = Some(gap.into());
    }
    if let Some(width) = spec.width {
        style.size.width = Some(length_from_spec(width));
    }
    if let Some(height) = spec.height {
        style.size.height = Some(length_from_spec(height));
    }
    if let Some(border_width) = spec.border_width {
        let width = px(border_width as f32);
        style.border_widths.top = Some(width.into());
        style.border_widths.right = Some(width.into());
        style.border_widths.bottom = Some(width.into());
        style.border_widths.left = Some(width.into());
    }
    if let Some(color) = spec.background {
        style.background = Some(Fill::from(Hsla::from(rgb(color))));
    }
    if let Some(color) = spec.border_color {
        style.border_color = Some(rgb(color).into());
    }
    if let Some(radius) = spec.corner_radius {
        let radius = px(radius as f32);
        style.corner_radii.top_left = Some(radius.into());
        style.corner_radii.top_right = Some(radius.into());
        style.corner_radii.bottom_right = Some(radius.into());
        style.corner_radii.bottom_left = Some(radius.into());
    }
    if let Some(color) = spec.text_color {
        style.text.color = Some(rgb(color).into());
    }
    if let Some(size) = spec.text_size {
        style.text.font_size = Some(px(size as f32).into());
    }
    let overflow = |spec| match spec {
        OverflowSpec::Visible => Overflow::Visible,
        OverflowSpec::Hidden => Overflow::Hidden,
        OverflowSpec::Scroll => Overflow::Scroll,
    };
    style.overflow.x = spec.overflow_x.map(overflow);
    style.overflow.y = spec.overflow_y.map(overflow);
    let padding = |side: Option<f64>| side.map(|n| px(n as f32).into());
    style.padding.top = padding(spec.padding.top);
    style.padding.right = padding(spec.padding.right);
    style.padding.bottom = padding(spec.padding.bottom);
    style.padding.left = padding(spec.padding.left);
    style.margin.top = spec.margin.top.map(length_from_spec);
    style.margin.right = spec.margin.right.map(length_from_spec);
    style.margin.bottom = spec.margin.bottom.map(length_from_spec);
    style.margin.left = spec.margin.left.map(length_from_spec);
    style.flex_grow = spec.flex_grow.map(|n| n as f32);
    style.flex_shrink = spec.flex_shrink.map(|n| n as f32);
    style.opacity = spec.opacity.map(|n| n as f32);
    style.min_size.width = spec.min_width.map(length_from_spec);
    style.min_size.height = spec.min_height.map(length_from_spec);
    style.max_size.width = spec.max_width.map(length_from_spec);
    style.max_size.height = spec.max_height.map(length_from_spec);
}

/// Wires whichever of [`EventMask::MOUSE_DOWN`]/[`MOUSE_UP`]/[`MOUSE_MOVE`]/
/// [`WHEEL`](EventMask::WHEEL)/[`KEY_DOWN`](EventMask::KEY_DOWN)/
/// [`KEY_UP`](EventMask::KEY_UP) `wired` reports as present — every kind
/// that doesn't need a `gpui` `ElementId`, so `Elem` can be `Div` or
/// `Stateful<Div>` interchangeably; both are [`InteractiveElement`].
fn wire_stateless<Elem, E>(
    element: Elem,
    id: NodeId,
    wired: &impl Fn(EventMask) -> Option<E>,
) -> Elem
where
    Elem: InteractiveElement + FluentBuilder,
    E: EventSink + Clone + 'static,
{
    element
        .when_some(wired(EventMask::MOUSE_DOWN), |el, listening| {
            el.on_any_mouse_down(move |event, window, cx| {
                listening.dispatch(id, "mousedown", &event.into(), window, cx);
            })
        })
        .when_some(wired(EventMask::MOUSE_UP), |mut el, listening| {
            // No fluent `on_any_mouse_up` exists — DOM's `mouseup`
            // fires for any button, and only `Interactivity`'s
            // imperative form takes "any" rather than one button.
            el.interactivity()
                .on_any_mouse_up(move |event, window, cx| {
                    listening.dispatch(id, "mouseup", &event.into(), window, cx);
                });
            el
        })
        .when_some(wired(EventMask::MOUSE_MOVE), |el, listening| {
            el.on_mouse_move(move |event, window, cx| {
                listening.dispatch(id, "mousemove", &event.into(), window, cx);
            })
        })
        .when_some(wired(EventMask::WHEEL), |el, listening| {
            el.on_scroll_wheel(move |event, window, cx| {
                listening.dispatch(id, "wheel", &event.into(), window, cx);
            })
        })
        .when_some(wired(EventMask::KEY_DOWN), |el, listening| {
            el.on_key_down(move |event, window, cx| {
                listening.dispatch(id, "keydown", &event.into(), window, cx);
            })
        })
        .when_some(wired(EventMask::KEY_UP), |el, listening| {
            el.on_key_up(move |event, window, cx| {
                listening.dispatch(id, "keyup", &event.into(), window, cx);
            })
        })
}

/// Tracks `node_id`'s focus state on `element`, if it has a
/// [`gpui::FocusHandle`] — orthogonal to [`EventMask::needs_element_id`]:
/// `.track_focus` is an [`InteractiveElement`] method, so this applies the
/// same way whether or not the container also got a `gpui` `ElementId`.
fn wire_focus<Elem, E>(element: Elem, dispatch: Option<&E>, id: NodeId) -> Elem
where
    Elem: InteractiveElement + FluentBuilder,
    E: EventSink,
{
    element.when_some(dispatch.and_then(|d| d.focus_handle(id)), |el, handle| {
        el.track_focus(&handle)
    })
}

/// Every scroll container's offset before the wheel event being dispatched,
/// outermost first.
#[derive(Default)]
struct WheelSnapshots(Vec<(ScrollHandle, Point<Pixels>)>);

impl Global for WheelSnapshots {}

/// A zero-size child whose paint records its container's offset in the
/// capture phase, which runs before any bubble-phase scroll step. Children
/// paint after their parent, and a container adds this before its own
/// children, so containers record outermost first.
fn scroll_recorder(handle: ScrollHandle) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            window.on_mouse_event(move |_: &ScrollWheelEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let snapshots = &mut cx.default_global::<WheelSnapshots>().0;
                let first = snapshots.is_empty();
                snapshots.push((handle.clone(), handle.offset()));
                if first {
                    window.defer(cx, settle_wheel);
                }
            });
        },
    )
    .absolute()
    .size_0()
}

// Zero-size child; its capture-phase handlers run before any node's own.
fn pointer_tracker<E: EventSink + Clone + 'static>(dispatch: E) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let moved = dispatch.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    moved.pointer_moved(event.position, cx);
                }
            });
            let dispatch = dispatch.clone();
            window.on_mouse_event(move |_: &MouseExitEvent, phase, _, _| {
                if phase == DispatchPhase::Capture {
                    dispatch.pointer_left();
                }
            });
        },
    )
    .absolute()
    .size_0()
}

/// Undoes, once `gpui`'s scroll steps and the JS listeners have run, all
/// scrolling after `preventDefault()`, else the scrolling of every container
/// but the innermost one that can still move.
///
/// Runs before the next draw. A `gpui` that applies the wheel step later
/// than the event breaks this.
fn settle_wheel(window: &mut Window, cx: &mut App) {
    let snapshots = std::mem::take(&mut cx.default_global::<WheelSnapshots>().0);
    let mut settled = window.default_prevented();
    for (handle, before) in snapshots.into_iter().rev() {
        // `gpui` clamps the offset on the next draw; compare as it will.
        let (offset, max) = (handle.offset(), handle.max_offset());
        let moved = point(
            offset.x.clamp(-max.x, px(0.)),
            offset.y.clamp(-max.y, px(0.)),
        );
        if !settled && moved != before {
            settled = true;
        } else {
            handle.set_offset(before);
        }
    }
}

/// Tracks a scroll container's [`ScrollHandle`] and records its offset for
/// [`settle_wheel`]. Without a handle `gpui` scrolls it on its own.
fn wire_scroll<Elem, E>(element: Elem, dispatch: Option<&E>, spec: &ElementSpec) -> Elem
where
    Elem: StatefulInteractiveElement + ParentElement + FluentBuilder,
    E: EventSink,
{
    let handle = dispatch
        .filter(|_| spec.style.scrolls())
        .and_then(|d| d.scroll_handle(spec.id));
    element.when_some(handle, |el, handle| {
        el.track_scroll(&handle).child(scroll_recorder(handle))
    })
}

/// The selector `build_element` gives the container with this id. A test
/// passes it to `gpui`'s `debug_bounds` to read that container's bounds.
#[must_use]
pub fn debug_selector(id: NodeId) -> String {
    format!("node-{id}")
}

/// Recursively converts an [`ElementSpec`] into a real `gpui` [`AnyElement`].
///
/// A container gets a hitbox only when something listens on it — GPUI
/// inserts one for any element carrying a mouse listener, `click` included.
/// A container also gets a `gpui` `ElementId` (`.id()`) whenever any wired
/// kind is [`EventKind::needs_element_id`] — `click` is the only one today,
/// via `on_click` (a `StatefulInteractiveElement` method); the other kinds
/// wire through plain `InteractiveElement` methods and need no id. A container
/// that scrolls on either axis gets one too, since `gpui` keeps its scroll
/// offset in element state.
///
/// Every container carries the selector [`debug_selector`] names, a no-op
/// outside test builds, so a test can look its computed bounds up by
/// [`NodeId`], wired or not.
///
/// The root container (`root`) also carries the pointer tracker when there is
/// a dispatcher.
fn build_element_inner<E: EventSink + Clone + 'static>(
    spec: &ElementSpec,
    dispatch: Option<&E>,
    root: bool,
) -> AnyElement {
    match &spec.tag {
        ElementTag::Text(content) => content.clone().into_any_element(),
        ElementTag::Container => {
            let id = spec.id;
            // `Some(dispatch.clone())` when `mask` is both listened for and
            // there's a dispatcher to call — `None` otherwise. Threading the
            // dispatcher itself through `when_some` (rather than a `bool`
            // plus a later `dispatch.expect(..)`) makes "wired implies a
            // dispatcher" a fact the type checker holds, not one this
            // function has to keep true by hand.
            let wired = |mask: EventMask| -> Option<E> {
                dispatch.filter(|_| spec.listens.contains(mask)).cloned()
            };
            let element = div().debug_selector(move || debug_selector(id));

            if spec.style.scrolls() || (dispatch.is_some() && spec.listens.needs_element_id()) {
                let element =
                    wire_stateless(element.id(ElementId::Integer(u64::from(id))), id, &wired);
                let element = wire_focus(element, dispatch, id);
                let element = wire_scroll(element, dispatch, spec);
                // One `on_hover` covers both `mouseenter`/`mouseleave` —
                // GPUI panics if it's called twice on the same element, so
                // which name to dispatch is decided from its `bool` at
                // call time, not by registering per kind.
                let element = element.when_some(
                    dispatch
                        .filter(|_| {
                            spec.listens
                                .intersects(EventMask::MOUSE_ENTER | EventMask::MOUSE_LEAVE)
                        })
                        .cloned(),
                    |el, listening| {
                        el.on_hover(move |is_hovered, window, cx| {
                            let event = if *is_hovered {
                                "mouseenter"
                            } else {
                                "mouseleave"
                            };
                            let payload = EventPayload::Mouse(MousePayload::at(
                                window.mouse_position(),
                                window.modifiers(),
                            ));
                            listening.dispatch(id, event, &payload, window, cx);
                        })
                    },
                );
                match wired(EventMask::CLICK) {
                    Some(listening) => finish_container(
                        element.on_click(move |_, window, cx| {
                            listening.dispatch(id, "click", &EventPayload::None, window, cx);
                        }),
                        spec,
                        dispatch,
                        root,
                    ),
                    None => finish_container(element, spec, dispatch, root),
                }
            } else {
                let element = wire_stateless(element, id, &wired);
                finish_container(wire_focus(element, dispatch, id), spec, dispatch, root)
            }
        }
    }
}

/// Applies `spec`'s style and children to a container, whichever kind of
/// element it turned out to be. The root container (`root`) also carries the
/// pointer tracker, as its first child, when there is a dispatcher.
fn finish_container<E>(
    mut element: E,
    spec: &ElementSpec,
    dispatch: Option<&(impl EventSink + Clone + 'static)>,
    root: bool,
) -> AnyElement
where
    E: Styled + ParentElement + IntoElement + 'static,
{
    apply_style(element.style(), &spec.style);
    if let Some(dispatch) = dispatch.filter(|_| root) {
        element = element.child(pointer_tracker(dispatch.clone()));
    }
    for child in &spec.children {
        if child.tag == ElementTag::Text(String::new()) {
            continue;
        }
        element = element.child(build_element_inner(child, dispatch, false));
    }
    element.into_any_element()
}

/// Type-only stand-in for [`build_element_inner`]'s `dispatch` parameter
/// when there's no dispatcher at all — [`build_element`] always passes
/// `None`, so [`EventSink::dispatch`] is unreachable here, but a concrete
/// type is still needed to name in that `None`.
#[derive(Clone)]
struct NeverListens;

impl EventSink for NeverListens {
    fn listens(&self, _node_id: NodeId) -> EventMask {
        EventMask::NONE
    }

    fn dispatch(
        &self,
        _node_id: NodeId,
        _event: &str,
        _payload: &EventPayload,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn focus_handle(&self, _node_id: NodeId) -> Option<gpui::FocusHandle> {
        None
    }

    fn scroll_handle(&self, _node_id: NodeId) -> Option<ScrollHandle> {
        None
    }

    fn pointer_moved(&self, _position: Point<Pixels>, _cx: &mut App) {}

    fn pointer_left(&self) {}
}

/// Recursively converts an [`ElementSpec`] into a real `gpui` [`AnyElement`],
/// with no event wiring — see [`build_element_with_events`] for a version
/// whose containers dispatch `"click"` into JS.
#[must_use]
pub fn build_element(spec: &ElementSpec) -> AnyElement {
    build_element_inner::<NeverListens>(spec, None, true)
}

/// Like [`build_element`], but every container's click dispatches into JS
/// via `dispatch` (an `inca-bridge::EventDispatcher`, behind this crate's
/// [`EventSink`] trait). `dispatch` also receives
/// [`EventSink::pointer_moved`] and [`EventSink::pointer_left`] for the
/// window's pointer moves and exits.
#[must_use]
pub fn build_element_with_events<E: EventSink + Clone + 'static>(
    spec: &ElementSpec,
    dispatch: &E,
) -> AnyElement {
    build_element_inner(spec, Some(dispatch), true)
}

/// Composes [`build_spec`] and [`build_element`]: builds `root` and its
/// whole subtree from `tree` into a real `gpui` element. `None` if `root`
/// doesn't resolve.
#[must_use]
pub fn render_tree(tree: &VirtualTree, root: NodeId) -> Option<AnyElement> {
    build_spec(tree, root).map(|spec| build_element(&spec))
}

/// Like [`render_tree`], but every container's click dispatches into JS via
/// `dispatch` (see [`build_element_with_events`]).
#[must_use]
pub fn render_tree_with_events<E: EventSink + Clone + 'static>(
    tree: &VirtualTree,
    root: NodeId,
    dispatch: &E,
) -> Option<AnyElement> {
    build_spec_with(tree, root, &|id| dispatch.listens(id))
        .map(|spec| build_element_with_events(&spec, dispatch))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn num(n: f64) -> AttributeValue {
        AttributeValue::Number(n)
    }

    fn text(s: &str) -> AttributeValue {
        AttributeValue::String(s.to_owned())
    }

    #[test]
    fn debug_selector_names_the_node_id() {
        assert_eq!(debug_selector(7), "node-7");
    }

    mod spec_layer {
        use super::*;

        #[test]
        fn unknown_root_returns_none() {
            let tree = VirtualTree::new();
            assert!(build_spec(&tree, 42).is_none());
        }

        #[test]
        fn non_text_tag_is_a_container() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.tag, ElementTag::Container);
        }

        #[test]
        fn text_tag_uses_its_value_attribute() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("text");
            tree.set_attribute(id, "value", "hello").unwrap();

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.tag, ElementTag::Text("hello".into()));
        }

        #[test]
        fn text_tag_uses_its_child_text() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("text");
            let child = tree.create_node("text");
            tree.set_attribute(child, "value", "Hello").unwrap();
            tree.append_child(id, child).unwrap();

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.tag, ElementTag::Text("Hello".into()));
            assert!(spec.children.is_empty());
        }

        #[test]
        fn text_tag_puts_its_value_before_its_children_in_order() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("text");
            tree.set_attribute(id, "value", "a").unwrap();
            for part in ["b", "c"] {
                let child = tree.create_node("text");
                tree.set_attribute(child, "value", part).unwrap();
                tree.append_child(id, child).unwrap();
            }

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.tag, ElementTag::Text("abc".into()));
        }

        #[test]
        fn text_tag_includes_text_nested_in_any_tag() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("text");
            let wrapper = tree.create_node("div");
            let leaf = tree.create_node("text");
            tree.set_attribute(leaf, "value", "nested").unwrap();
            tree.append_child(wrapper, leaf).unwrap();
            tree.append_child(id, wrapper).unwrap();

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.tag, ElementTag::Text("nested".into()));
        }

        #[test]
        fn text_tag_counts_a_non_string_value_as_empty() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("text");
            tree.set_attribute(id, "value", 1.0).unwrap();
            let child = tree.create_node("text");
            tree.set_attribute(child, "value", "kept").unwrap();
            tree.append_child(id, child).unwrap();

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.tag, ElementTag::Text("kept".into()));
        }

        #[test]
        fn text_tag_missing_value_is_empty_content() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("text");

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.tag, ElementTag::Text(String::new()));
        }

        #[test]
        fn children_are_built_in_append_order() {
            let mut tree = VirtualTree::new();
            let parent = tree.create_node("div");
            let a = tree.create_node("div");
            let b = tree.create_node("div");
            tree.append_child(parent, a).unwrap();
            tree.append_child(parent, b).unwrap();

            let spec = build_spec(&tree, parent).unwrap();
            let child_ids: Vec<NodeId> = spec.children.iter().map(|c| c.id).collect();
            assert_eq!(child_ids, &[a, b]);
        }

        #[test]
        fn a_removed_style_key_renders_as_the_default() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");
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
            let id = tree.create_node("div");
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
                    background: Some(0x505050),
                    border_color: Some(0x0000ff),
                    corner_radius: Some(4.0),
                    text_color: Some(0xffffff),
                    text_size: Some(20.0),
                    ..StyleSpec::default()
                }
            );
        }

        fn style_of(props: &[(&str, AttributeValue)]) -> StyleSpec {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");
            for (key, value) in props {
                tree.set_style(id, *key, value.clone()).unwrap();
            }
            build_spec(&tree, id).unwrap().style
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
        fn box_keys_resolve_the_same_in_any_insertion_order() {
            let keys = [
                ("padding", num(1.0)),
                ("padding_x", num(2.0)),
                ("padding_left", num(3.0)),
                ("margin", num(4.0)),
                ("margin_y", text("auto")),
                ("margin_top", num(-5.0)),
            ];
            let expected = style_of(&keys);
            for shift in 1..keys.len() {
                let mut rotated = keys.to_vec();
                rotated.rotate_left(shift);
                assert_eq!(style_of(&rotated), expected);
                rotated.reverse();
                assert_eq!(style_of(&rotated), expected);
            }
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
        fn margin_takes_negatives_and_auto_but_padding_does_not() {
            let margin =
                style_of(&[("margin_left", num(-8.0)), ("margin_right", text("auto"))]).margin;
            assert_eq!(margin.left, Some(LengthSpec::Px(-8.0)));
            assert_eq!(margin.right, Some(LengthSpec::Auto));
            for bad in [
                num(-1.0),
                num(f64::NAN),
                num(f64::INFINITY),
                text("auto"),
                text("1 2"),
            ] {
                let padding = style_of(&[("padding", bad)]).padding;
                assert_eq!(padding, EdgesSpec::default());
            }
            for bad in [
                num(f64::NAN),
                num(f64::INFINITY),
                num(f64::NEG_INFINITY),
                text("1 2"),
            ] {
                let margin = style_of(&[("margin", bad)]).margin;
                assert_eq!(margin, EdgesSpec::default());
            }
        }

        #[test]
        fn flex_factors_reject_negatives_and_nan() {
            for bad in [num(-1.0), num(f64::NAN), num(f64::INFINITY), text("1")] {
                let style = style_of(&[("flex_grow", bad.clone()), ("flex_shrink", bad)]);
                assert_eq!((style.flex_grow, style.flex_shrink), (None, None));
            }
            let style = style_of(&[("flex_grow", num(0.0)), ("flex_shrink", num(1.5))]);
            assert_eq!((style.flex_grow, style.flex_shrink), (Some(0.0), Some(1.5)));
        }

        #[test]
        fn opacity_is_clamped_and_nan_is_unset() {
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
            assert_eq!(opacity(num(f64::NAN)), None);
            assert_eq!(opacity(text("0.5")), None);
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
            let id = tree.create_node("div");
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
            let id = tree.create_node("div");
            for (key, value) in props {
                tree.set_style(id, *key, *value).unwrap();
            }
            let style = build_spec(&tree, id).unwrap().style;
            (style.overflow_x, style.overflow_y)
        }

        #[test]
        fn overflow_keys_are_mapped_and_auto_scrolls() {
            use OverflowSpec::{Hidden, Scroll, Visible};
            assert_eq!(
                overflow_of(&[("overflow", "hidden")]),
                (Some(Hidden), Some(Hidden))
            );
            assert_eq!(
                overflow_of(&[("overflow", "visible")]),
                (Some(Visible), Some(Visible))
            );
            assert_eq!(
                overflow_of(&[("overflow", "auto")]),
                (Some(Scroll), Some(Scroll))
            );
            assert_eq!(
                overflow_of(&[("overflow_x", "scroll")]),
                (Some(Scroll), None)
            );
            assert_eq!(overflow_of(&[("overflow_y", "auto")]), (None, Some(Scroll)));
        }

        #[test]
        fn an_overflow_axis_key_beats_the_shorthand_in_any_order() {
            use OverflowSpec::{Hidden, Scroll};
            let keys = [
                ("overflow", "hidden"),
                ("overflow_x", "scroll"),
                ("overflow_y", "scroll"),
            ];
            for order in [[0, 1, 2], [2, 1, 0], [1, 0, 2], [1, 2, 0]] {
                let props: Vec<_> = order.iter().map(|&i| keys[i]).collect();
                assert_eq!(overflow_of(&props), (Some(Scroll), Some(Scroll)));
            }
            assert_eq!(
                overflow_of(&[("overflow_y", "scroll"), ("overflow", "hidden")]),
                (Some(Hidden), Some(Scroll))
            );
        }

        #[test]
        fn a_bad_overflow_value_is_ignored() {
            assert_eq!(overflow_of(&[("overflow", "clip")]), (None, None));
            assert_eq!(overflow_of(&[("overflow_x", "nope")]), (None, None));
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");
            tree.set_style(id, "overflow", 1.0).unwrap();
            assert_eq!(build_spec(&tree, id).unwrap().style, StyleSpec::default());
        }

        #[test]
        fn a_removed_overflow_key_renders_as_the_default() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");
            tree.set_style(id, "overflow", "scroll").unwrap();
            tree.remove_style(id, "overflow").unwrap();
            assert_eq!(build_spec(&tree, id).unwrap().style, StyleSpec::default());
        }

        #[test]
        fn either_scrolling_axis_makes_a_container_scroll() {
            let scrolls = |props: &[(&str, &str)]| {
                let mut tree = VirtualTree::new();
                let id = tree.create_node("div");
                for (key, value) in props {
                    tree.set_style(id, *key, *value).unwrap();
                }
                build_spec(&tree, id).unwrap().style.scrolls()
            };
            assert!(scrolls(&[("overflow_x", "scroll")]));
            assert!(scrolls(&[("overflow_y", "auto")]));
            assert!(scrolls(&[("overflow", "scroll")]));
            assert!(!scrolls(&[("overflow", "hidden")]));
            assert!(!scrolls(&[
                ("overflow_x", "visible"),
                ("overflow_y", "hidden")
            ]));
            assert!(!scrolls(&[]));
        }

        #[test]
        fn nothing_listens_unless_asked() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.listens, EventMask::NONE);
        }

        #[test]
        fn the_predicate_is_asked_about_every_node() {
            let mut tree = VirtualTree::new();
            let parent = tree.create_node("div");
            let listening = tree.create_node("div");
            let quiet = tree.create_node("div");
            tree.append_child(parent, listening).unwrap();
            tree.append_child(parent, quiet).unwrap();

            let spec = build_spec_with(&tree, parent, &|id| {
                if id == listening {
                    EventMask::CLICK
                } else {
                    EventMask::NONE
                }
            })
            .unwrap();

            assert_eq!(spec.listens, EventMask::NONE);
            assert_eq!(spec.children[0].listens, EventMask::CLICK);
            assert_eq!(spec.children[1].listens, EventMask::NONE);
        }

        /// The style a node gets from `props`, and what [`style_warning`] says
        /// about each prop, in order.
        fn style_and_warnings(props: &[(&str, AttributeValue)]) -> (StyleSpec, Vec<String>) {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");
            for (key, value) in props {
                tree.set_style(id, *key, value.clone()).unwrap();
            }
            let warnings = props
                .iter()
                .filter_map(|(key, value)| style_warning(key, value))
                .collect();
            (build_spec(&tree, id).unwrap().style, warnings)
        }

        #[test]
        fn unrecognized_style_key_is_ignored_with_a_warning() {
            let (style, warnings) = style_and_warnings(&[("not_a_real_prop", num(1.0))]);
            assert_eq!(style, StyleSpec::default());
            assert_eq!(warnings, ["ignoring unknown style key `not_a_real_prop`"]);
        }

        #[test]
        fn a_camel_case_key_is_ignored_with_a_warning_naming_it() {
            let (style, warnings) =
                style_and_warnings(&[("flexDirection", text("column")), ("paddingTop", num(4.0))]);
            assert_eq!(style, StyleSpec::default());
            assert_eq!(warnings.len(), 2);
            for key in ["flexDirection", "paddingTop"] {
                assert!(
                    warnings.iter().any(|w| w.contains(&format!("`{key}`"))),
                    "{warnings:?}"
                );
            }
        }

        #[test]
        fn recognized_style_keys_apply_quietly() {
            let (_, warnings) = style_and_warnings(&[
                ("flex_direction", text("column")),
                ("padding_top", num(4.0)),
                ("background", num(1.0)),
                ("overflow", text("scroll")),
                ("opacity", num(2.0)),
            ]);
            assert!(warnings.is_empty(), "{warnings:?}");
        }

        #[test]
        fn every_padding_and_margin_key_is_recognized() {
            for base in ["padding", "margin"] {
                for suffix in ["", "_x", "_y", "_top", "_right", "_bottom", "_left"] {
                    let key = format!("{base}{suffix}");
                    let (_, warnings) = style_and_warnings(&[(key.as_str(), num(1.0))]);
                    assert!(warnings.is_empty(), "{key}: {warnings:?}");
                }
            }
        }

        #[test]
        fn a_wrong_shaped_value_of_a_known_key_is_dropped_quietly() {
            let (_, warnings) =
                style_and_warnings(&[("display", text("nope")), ("gap", text("1"))]);
            assert!(warnings.is_empty(), "{warnings:?}");
        }

        #[test]
        fn malformed_enum_value_is_ignored_not_a_panic() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");
            tree.set_style(id, "display", "not-a-real-display-value")
                .unwrap();

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.style.display, None);
        }

        #[test]
        fn color_accepts_hex_strings_as_well_as_numbers() {
            let mut tree = VirtualTree::new();
            let id = tree.create_node("div");
            tree.set_style(id, "background", "#505050").unwrap();
            tree.set_style(id, "border_color", "#00f").unwrap();

            let spec = build_spec(&tree, id).unwrap();
            assert_eq!(spec.style.background, Some(0x505050));
            assert_eq!(
                spec.style.border_color,
                Some(0x0000ff),
                "a 3-digit hex string must expand each digit, not zero-pad it"
            );
        }

        #[test]
        fn malformed_color_string_is_ignored_with_a_warning() {
            let (style, warnings) = style_and_warnings(&[("background", text("not-a-color"))]);
            assert_eq!(style.background, None);
            assert_eq!(
                warnings,
                ["ignoring invalid colour for style key `background`"]
            );
        }

        #[test]
        fn a_color_string_with_a_sign_or_non_hex_digit_is_ignored() {
            for bad in [
                "#+abcde", "#-abcde", "#+ab", "#+abcd", "#12345g", "#gg0", "#12", "#1234",
                "#1234567", "ffffff", "#", "#ééé",
            ] {
                let (style, warnings) = style_and_warnings(&[("text_color", text(bad))]);
                assert_eq!(style.text_color, None, "{bad}");
                assert_eq!(warnings.len(), 1, "{bad}");
            }
        }

        #[test]
        fn color_strings_accept_either_hex_case() {
            let (style, warnings) = style_and_warnings(&[
                ("background", text("#AbCdEf")),
                ("border_color", text("#FfF")),
            ]);
            assert_eq!(style.background, Some(0x00ab_cdef));
            assert_eq!(style.border_color, Some(0x00ff_ffff));
            assert!(warnings.is_empty());
        }

        #[test]
        fn numeric_colors_cover_exactly_zero_to_ffffff() {
            for (n, expected) in [(0.0, 0), (f64::from(0xFF_FFFF), 0xFF_FFFF), (255.0, 255)] {
                let (style, warnings) = style_and_warnings(&[("background", num(n))]);
                assert_eq!(style.background, Some(expected), "{n}");
                assert!(warnings.is_empty(), "{n}");
            }
        }

        #[test]
        fn a_fractional_numeric_color_rounds_toward_zero() {
            for (n, expected) in [(0.5, 0), (1.5, 1), (16_777_214.5, 0xFF_FFFE)] {
                let (style, warnings) = style_and_warnings(&[("background", num(n))]);
                assert_eq!(style.background, Some(expected), "{n}");
                assert!(warnings.is_empty(), "{n}");
            }
        }

        #[test]
        fn a_negative_fractional_numeric_color_is_ignored_with_a_warning() {
            let (style, warnings) = style_and_warnings(&[("background", num(-0.5))]);
            assert_eq!(style.background, None);
            assert_eq!(warnings.len(), 1);
        }

        #[test]
        fn an_out_of_range_numeric_color_is_ignored_with_a_warning() {
            for bad in [
                -1.0,
                f64::from(0x0100_0000),
                f64::from(u32::MAX),
                1e20,
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ] {
                let (style, warnings) = style_and_warnings(&[
                    ("background", num(bad)),
                    ("border_color", num(bad)),
                    ("text_color", num(bad)),
                ]);
                assert_eq!(
                    (style.background, style.border_color, style.text_color),
                    (None, None, None),
                    "{bad}"
                );
                assert_eq!(warnings.len(), 3, "{bad}");
            }
        }

        #[test]
        fn a_bool_color_is_ignored_with_a_warning() {
            let (style, warnings) =
                style_and_warnings(&[("background", AttributeValue::Bool(true))]);
            assert_eq!(style.background, None);
            assert_eq!(warnings.len(), 1);
        }
    }

    mod gpui_layer {
        use super::{num as n, text as s, *};
        use gpui::{
            Context, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, TestAppContext,
            VisualTestContext, point, size,
        };

        #[gpui::test]
        fn container_with_fixed_size_lays_out_at_that_size(cx: &mut TestAppContext) {
            let mut tree = VirtualTree::new();
            let root = tree.create_node("div");
            tree.set_style(root, "width", 120.0).unwrap();
            tree.set_style(root, "height", 80.0).unwrap();

            let cx = cx.add_empty_window();
            cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
                render_tree(&tree, root).unwrap()
            });

            let bounds = cx
                .debug_bounds("node-0")
                .expect("root container should be tagged with a debug selector");
            assert_eq!(bounds.size.width, px(120.0));
            assert_eq!(bounds.size.height, px(80.0));
        }

        #[gpui::test]
        fn text_leaf_renders_without_panicking(cx: &mut TestAppContext) {
            let mut tree = VirtualTree::new();
            let root = tree.create_node("text");
            tree.set_attribute(root, "value", "hello").unwrap();

            let cx = cx.add_empty_window();
            cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
                render_tree(&tree, root).unwrap()
            });
        }

        /// Bounds of a container styled `root_props` (first) and its `children`
        /// (at most two), in node order.
        fn layout(
            cx: &mut TestAppContext,
            root_props: &[(&str, AttributeValue)],
            children: &[&[(&str, AttributeValue)]],
        ) -> Vec<gpui::Bounds<Pixels>> {
            let mut tree = VirtualTree::new();
            let root = tree.create_node("div");
            for (key, value) in root_props {
                tree.set_style(root, *key, value.clone()).unwrap();
            }
            for props in children {
                let child = tree.create_node("div");
                for (key, value) in *props {
                    tree.set_style(child, *key, value.clone()).unwrap();
                }
                tree.append_child(root, child).unwrap();
            }
            let cx = cx.add_empty_window();
            cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
                render_tree(&tree, root).unwrap()
            });
            ["node-0", "node-1", "node-2"][..=children.len()]
                .iter()
                .map(|name| cx.debug_bounds(name).unwrap())
                .collect()
        }

        #[gpui::test]
        fn padding_insets_the_child(cx: &mut TestAppContext) {
            let bounds = layout(
                cx,
                &[("width", n(200.)), ("height", n(100.)), ("padding", n(10.))],
                &[&[("width", n(50.)), ("height", n(30.))]],
            );
            assert_eq!(bounds[1].origin - bounds[0].origin, point(px(10.), px(10.)));
            assert_eq!(bounds[0].size, size(px(200.), px(100.)));
        }

        #[gpui::test]
        fn padding_grows_a_container_sized_by_its_content(cx: &mut TestAppContext) {
            let bounds = layout(
                cx,
                &[("padding_top", n(4.)), ("padding_bottom", n(6.))],
                &[&[("height", n(30.))]],
            );
            assert_eq!(bounds[0].size.height, px(40.));
        }

        #[gpui::test]
        fn a_negative_margin_pulls_the_child_out(cx: &mut TestAppContext) {
            let bounds = layout(
                cx,
                &[("width", n(200.)), ("height", n(100.))],
                &[&[
                    ("width", n(50.)),
                    ("height", n(30.)),
                    ("margin_left", n(-5.)),
                ]],
            );
            assert_eq!(bounds[1].origin.x - bounds[0].origin.x, px(-5.));
        }

        #[gpui::test]
        fn margin_offsets_the_child(cx: &mut TestAppContext) {
            let bounds = layout(
                cx,
                &[("width", n(200.)), ("height", n(100.))],
                &[&[
                    ("width", n(50.)),
                    ("height", n(30.)),
                    ("margin", n(2.)),
                    ("margin_left", n(7.)),
                    ("margin_top", n(3.)),
                ]],
            );
            assert_eq!(bounds[1].origin - bounds[0].origin, point(px(7.), px(3.)));
        }

        #[gpui::test]
        fn an_auto_margin_takes_the_free_space(cx: &mut TestAppContext) {
            let row = [
                ("display", s("flex")),
                ("flex_direction", s("row")),
                ("width", n(200.)),
                ("height", n(100.)),
            ];
            let pushed = layout(
                cx,
                &row,
                &[&[
                    ("width", n(50.)),
                    ("height", n(30.)),
                    ("margin_left", s("auto")),
                ]],
            );
            assert_eq!(pushed[1].origin.x - pushed[0].origin.x, px(150.));
            let centered = layout(
                cx,
                &row,
                &[&[
                    ("width", n(50.)),
                    ("height", n(30.)),
                    ("margin_x", s("auto")),
                ]],
            );
            assert_eq!(centered[1].origin.x - centered[0].origin.x, px(75.));
        }

        #[gpui::test]
        fn min_width_widens_and_min_height_heightens(cx: &mut TestAppContext) {
            let bounds = layout(
                cx,
                &[
                    ("width", n(50.)),
                    ("height", n(20.)),
                    ("min_width", n(80.)),
                    ("min_height", n(35.)),
                ],
                &[],
            );
            assert_eq!(bounds[0].size, size(px(80.), px(35.)));
        }

        #[gpui::test]
        fn max_width_narrows_and_max_height_shortens(cx: &mut TestAppContext) {
            let bounds = layout(
                cx,
                &[
                    ("width", n(200.)),
                    ("height", n(100.)),
                    ("max_width", n(120.)),
                    ("max_height", n(60.)),
                ],
                &[],
            );
            assert_eq!(bounds[0].size, size(px(120.), px(60.)));
        }

        fn row() -> [(&'static str, AttributeValue); 4] {
            [
                ("display", s("flex")),
                ("flex_direction", s("row")),
                ("width", n(200.)),
                ("height", n(50.)),
            ]
        }

        #[gpui::test]
        fn flex_grow_fills_the_remaining_space(cx: &mut TestAppContext) {
            let bounds = layout(
                cx,
                &row(),
                &[
                    &[("width", n(50.)), ("flex_grow", n(1.))],
                    &[("width", n(50.))],
                ],
            );
            assert_eq!(bounds[1].size.width, px(150.));
            assert_eq!(bounds[2].size.width, px(50.));
            assert_eq!(bounds[2].origin.x - bounds[0].origin.x, px(150.));
        }

        struct TreeView(VirtualTree);

        impl Render for TreeView {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                render_tree(&self.0, 0).unwrap()
            }
        }

        /// How far a wheel `delta` over a 100x100 container with `props`
        /// moves its 300x300 child, without a dispatcher.
        fn content_shift(
            cx: &mut TestAppContext,
            props: &[(&str, &str)],
            delta: Point<Pixels>,
        ) -> Point<Pixels> {
            let mut tree = VirtualTree::new();
            let root = tree.create_node("div");
            let child = tree.create_node("div");
            tree.set_style(root, "width", 100.0).unwrap();
            tree.set_style(root, "height", 100.0).unwrap();
            for (key, value) in props {
                tree.set_style(root, *key, *value).unwrap();
            }
            tree.set_style(child, "width", 300.0).unwrap();
            tree.set_style(child, "height", 300.0).unwrap();
            tree.append_child(root, child).unwrap();

            let window = cx.add_window(|_, _| TreeView(tree));
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
            let mut cx = VisualTestContext::from_window(window.into(), cx);
            cx.simulate_event(ScrollWheelEvent {
                position: point(px(10.), px(10.)),
                delta: ScrollDelta::Pixels(delta),
                ..Default::default()
            });
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .unwrap();
            let container = cx.debug_bounds("node-0").unwrap();
            let content = cx.debug_bounds("node-1").unwrap();
            content.origin - container.origin
        }

        #[gpui::test]
        fn an_x_only_container_scrolls_along_x_without_a_dispatcher(cx: &mut TestAppContext) {
            let shift = content_shift(cx, &[("overflow_x", "scroll")], point(px(-50.), px(0.)));
            assert_eq!(shift, point(px(-50.), px(0.)));
        }

        #[gpui::test]
        fn a_scroll_container_scrolls_without_a_dispatcher(cx: &mut TestAppContext) {
            let shift = content_shift(cx, &[("overflow_y", "scroll")], point(px(0.), px(-50.)));
            assert_eq!(shift, point(px(0.), px(-50.)));
        }
    }
}
