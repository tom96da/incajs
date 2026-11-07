// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::{Display, Fill, FlexDirection, Hsla, Length, Overflow, StyleRefinement, px, rgba};

use inca_node::spec::{
    AlignSpec, ColorSpec, DisplaySpec, FlexDirectionSpec, LengthSpec, OverflowSpec, StyleSpec,
};

// f64 -> f32 for `gpui`'s `Pixels` type: real UI dimensions never carry
// enough precision or magnitude for this narrowing to matter.
#[allow(clippy::cast_possible_truncation)]
fn length_from_spec(spec: LengthSpec) -> Length {
    match spec {
        LengthSpec::Px(n) => px(n as f32).into(),
        LengthSpec::Auto => Length::Auto,
    }
}

fn display_from_spec(spec: DisplaySpec) -> Display {
    match spec {
        DisplaySpec::Flex => Display::Flex,
        DisplaySpec::Block => Display::Block,
        DisplaySpec::Grid => Display::Grid,
        DisplaySpec::None => Display::None,
    }
}

fn flex_direction_from_spec(spec: FlexDirectionSpec) -> FlexDirection {
    match spec {
        FlexDirectionSpec::Row => FlexDirection::Row,
        FlexDirectionSpec::Column => FlexDirection::Column,
        FlexDirectionSpec::RowReverse => FlexDirection::RowReverse,
        FlexDirectionSpec::ColumnReverse => FlexDirection::ColumnReverse,
    }
}

fn justify_from_spec(spec: AlignSpec) -> gpui::JustifyContent {
    match spec {
        AlignSpec::Start => gpui::JustifyContent::Start,
        AlignSpec::End => gpui::JustifyContent::End,
        AlignSpec::Center => gpui::JustifyContent::Center,
        AlignSpec::Stretch => gpui::JustifyContent::Stretch,
    }
}

fn align_from_spec(spec: AlignSpec) -> gpui::AlignItems {
    match spec {
        AlignSpec::Start => gpui::AlignItems::Start,
        AlignSpec::End => gpui::AlignItems::End,
        AlignSpec::Center => gpui::AlignItems::Center,
        AlignSpec::Stretch => gpui::AlignItems::Stretch,
    }
}

/// Maps a `Scroll` axis to `Hidden`. `gpui` then leaves the wheel step to the
/// caller and still keeps the tracked offset, its clamp and its maximum.
pub(super) fn clip_scroll_axes(style: &mut StyleRefinement) {
    for axis in [&mut style.overflow.x, &mut style.overflow.y] {
        if *axis == Some(Overflow::Scroll) {
            *axis = Some(Overflow::Hidden);
        }
    }
}

fn overflow_from_spec(spec: OverflowSpec) -> Overflow {
    match spec {
        OverflowSpec::Visible => Overflow::Visible,
        OverflowSpec::Clip => Overflow::Clip,
        OverflowSpec::Hidden => Overflow::Hidden,
        OverflowSpec::Scroll => Overflow::Scroll,
    }
}

fn color_from_spec(color: ColorSpec) -> Hsla {
    Hsla::from(rgba((color.rgb << 8) | u32::from(color.alpha)))
}

/// Sets `slot` from `value` when the spec carries one. An absent value leaves `slot` as it is.
fn overlay<S, T>(slot: &mut Option<T>, value: Option<S>, make: impl FnOnce(S) -> T) {
    if let Some(value) = value {
        *slot = Some(make(value));
    }
}

/// Applies a [`StyleSpec`] onto a real `gpui` [`StyleRefinement`], by direct
/// field assignment, which carries the arbitrary numbers a spec holds.
/// `gpui`'s named Tailwind-scale builder methods (`.gap_3()`, `.size_8()`)
/// cover fixed steps only. A field the spec leaves unset
/// keeps the value `style` already has.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn apply_style(style: &mut StyleRefinement, spec: &StyleSpec) {
    overlay(&mut style.display, spec.display, display_from_spec);
    overlay(
        &mut style.flex_direction,
        spec.flex_direction,
        flex_direction_from_spec,
    );
    overlay(
        &mut style.justify_content,
        spec.justify_content,
        justify_from_spec,
    );
    overlay(&mut style.align_items, spec.align_items, align_from_spec);
    let gap = spec.gap.map(|n| px(n as f32));
    overlay(&mut style.gap.width, gap, Into::into);
    overlay(&mut style.gap.height, gap, Into::into);
    overlay(&mut style.size.width, spec.width, length_from_spec);
    overlay(&mut style.size.height, spec.height, length_from_spec);
    let border_width = spec.border_width.map(|n| px(n as f32));
    overlay(&mut style.border_widths.top, border_width, Into::into);
    overlay(&mut style.border_widths.right, border_width, Into::into);
    overlay(&mut style.border_widths.bottom, border_width, Into::into);
    overlay(&mut style.border_widths.left, border_width, Into::into);
    overlay(&mut style.background, spec.background, |color| {
        Fill::from(color_from_spec(color))
    });
    overlay(&mut style.border_color, spec.border_color, color_from_spec);
    let radius = spec.corner_radius.map(|n| px(n as f32));
    overlay(&mut style.corner_radii.top_left, radius, Into::into);
    overlay(&mut style.corner_radii.top_right, radius, Into::into);
    overlay(&mut style.corner_radii.bottom_right, radius, Into::into);
    overlay(&mut style.corner_radii.bottom_left, radius, Into::into);
    overlay(&mut style.text.color, spec.text_color, color_from_spec);
    overlay(&mut style.text.font_size, spec.text_size, |size| {
        px(size as f32).into()
    });
    overlay(&mut style.overflow.x, spec.overflow_x, overflow_from_spec);
    overlay(&mut style.overflow.y, spec.overflow_y, overflow_from_spec);
    let padding = |n: f64| px(n as f32).into();
    overlay(&mut style.padding.top, spec.padding.top, padding);
    overlay(&mut style.padding.right, spec.padding.right, padding);
    overlay(&mut style.padding.bottom, spec.padding.bottom, padding);
    overlay(&mut style.padding.left, spec.padding.left, padding);
    overlay(&mut style.margin.top, spec.margin.top, length_from_spec);
    overlay(&mut style.margin.right, spec.margin.right, length_from_spec);
    overlay(
        &mut style.margin.bottom,
        spec.margin.bottom,
        length_from_spec,
    );
    overlay(&mut style.margin.left, spec.margin.left, length_from_spec);
    overlay(&mut style.flex_grow, spec.flex_grow, |n| n as f32);
    overlay(&mut style.flex_shrink, spec.flex_shrink, |n| n as f32);
    overlay(&mut style.opacity, spec.opacity, |n| n as f32);
    overlay(&mut style.min_size.width, spec.min_width, length_from_spec);
    overlay(
        &mut style.min_size.height,
        spec.min_height,
        length_from_spec,
    );
    overlay(&mut style.max_size.width, spec.max_width, length_from_spec);
    overlay(
        &mut style.max_size.height,
        spec.max_height,
        length_from_spec,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::*;

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

    #[test]
    fn clip_overflow_reaches_the_gpui_refinement_as_clip() {
        let mut refinement = StyleRefinement::default();
        apply_style(&mut refinement, &style_of(&[("overflow", text("clip"))]));
        assert_eq!(refinement.overflow.x, Some(gpui::Overflow::Clip));
        assert_eq!(refinement.overflow.y, Some(gpui::Overflow::Clip));
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
}
