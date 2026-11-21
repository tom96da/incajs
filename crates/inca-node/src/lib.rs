// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

mod dataset;
mod html;
pub mod input_text;
pub mod scroll;
pub mod semantics;
pub mod tree;

pub use dataset::{DatasetError, data_attribute, data_key};
pub use html::{HtmlScope, ParsedHtml, ParsedNode};
pub use input_text::{pasted, sanitized, typed};
pub use scroll::{Align, Rect, scroll_chain, scroll_delta};
pub use semantics::{ARIA_REFLECTIONS, AriaKind, AriaReflection, aria_reflection};
pub use tree::{
    AlignSpec, AttributeValue, ClassListError, DisplaySpec, FlexDirectionSpec, InsertArg, Inserted,
    Insertion, Link, NodeId, NodeKind, OverflowSpec, Placement, STYLE_KEYS, StyleKey, StyleMap,
    TreeError, ValueKind, VirtualNode, VirtualTree, is_disabled_control, valid_attribute_name,
};

#[cfg(test)]
mod tests;
