// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The inline style of a node as a block of declarations.

mod attribute;
mod declarations;
mod props;

pub use attribute::{css_text, set_style_text};
pub use declarations::{
    priority_refuses_value, property_names, remove_style_property, set_style_property, style_item,
    style_length, style_property_value,
};
pub use props::{remove_style, set_style};
