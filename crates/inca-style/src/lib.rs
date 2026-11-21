// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

mod cascade;
mod inline_style;
pub mod selector;
pub mod spec;
mod stylesheet;

pub use cascade::StyleCache;
pub use inline_style::{
    css_text, priority_refuses_value, property_names, remove_style, remove_style_property,
    set_style, set_style_property, set_style_text, style_item, style_length, style_property_value,
};
pub use selector::{
    SelectorError, check_selector, closest_selector, matches_selector, query_selector,
    query_selector_all,
};
pub use spec::{
    ColorSpec, EdgesSpec, ElementSpec, ElementTag, LengthSpec, StyleSpec, WheelAxes, build_spec,
    declared_window_size, hidden_by_attribute, is_scroll_container, style_warning, wheel_axes,
};

#[cfg(test)]
mod tests;
