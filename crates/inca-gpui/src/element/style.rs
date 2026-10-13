// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

use gpui::{Display, Fill, FlexDirection, Hsla, Length, Overflow, StyleRefinement, px, rgb};

use super::spec::{AlignSpec, DisplaySpec, FlexDirectionSpec, LengthSpec, OverflowSpec, StyleSpec};

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
/// methods (`.gap_3()`, `.size_8()`, ...). Those only cover fixed steps,
/// not the arbitrary numbers this spec carries.
// f64 -> f32 for `gpui`'s `Pixels` type: real UI dimensions never carry
// enough precision or magnitude for this narrowing to matter.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn apply_style(style: &mut StyleRefinement, spec: &StyleSpec) {
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
