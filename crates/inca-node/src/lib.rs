// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

mod dataset;
mod html;
mod inline_style;
pub mod input_text;
pub mod scroll;
pub mod selector;
pub mod spec;
mod stylesheet;
pub mod tree;

pub use dataset::{DatasetError, data_attribute, data_key};
pub use html::{HtmlScope, ParsedHtml, ParsedNode};
pub use inline_style::{css_text, style_item, style_length, style_property_value};
pub use input_text::{pasted, sanitized, typed};
pub use scroll::{Align, Rect, scroll_chain, scroll_delta};
pub use selector::{
    SelectorError, check_selector, closest_selector, matches_selector, query_selector,
    query_selector_all,
};
pub use spec::{
    AlignSpec, ColorSpec, DisplaySpec, EdgesSpec, ElementSpec, ElementTag, FlexDirectionSpec,
    LengthSpec, OverflowSpec, STYLE_KEYS, StyleKey, StyleSpec, ValueKind, WheelAxes, build_spec,
    declared_window_size, hidden_by_attribute, is_scroll_container, style_warning, wheel_axes,
};
pub use tree::{
    AttributeValue, ClassListError, NodeId, StyleMap, TreeError, VirtualNode, VirtualTree,
    focusable_by_default, is_disabled_control, is_template_tag, valid_attribute_name,
};

#[cfg(test)]
mod tests;
