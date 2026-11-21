// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

mod dataset;
mod html;
mod inline_style;
pub mod input_text;
pub mod scroll;
pub mod selector;
pub mod semantics;
pub mod spec;
mod stylesheet;
pub mod tree;

pub use dataset::{DatasetError, data_attribute, data_key};
pub use html::{HtmlScope, ParsedHtml, ParsedNode};
pub use inline_style::{
    css_text, remove_style, remove_style_property, set_style, set_style_property, set_style_text,
    style_item, style_length, style_property_value,
};
pub use input_text::{pasted, sanitized, typed};
pub use scroll::{Align, Rect, scroll_chain, scroll_delta};
pub use selector::{
    SelectorError, check_selector, closest_selector, matches_selector, query_selector,
    query_selector_all,
};
pub use semantics::{ARIA_REFLECTIONS, AriaKind, AriaReflection, aria_reflection};
pub use spec::{
    ColorSpec, EdgesSpec, ElementSpec, ElementTag, LengthSpec, StyleSpec, WheelAxes, build_spec,
    declared_window_size, hidden_by_attribute, is_scroll_container, style_warning, wheel_axes,
};
pub use tree::{
    AlignSpec, AttributeValue, ClassListError, DisplaySpec, FlexDirectionSpec, InsertArg, Inserted,
    Insertion, Link, NodeId, NodeKind, OverflowSpec, Placement, STYLE_KEYS, StyleCache, StyleKey,
    StyleMap, TreeError, ValueKind, VirtualNode, VirtualTree, is_disabled_control,
    valid_attribute_name,
};

#[cfg(test)]
mod tests;
