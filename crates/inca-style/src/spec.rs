// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reads a [`VirtualTree`](inca_node::VirtualTree) node into a plain description of its element
//! and style.
//!
//! [`build_spec`] turns a node and its subtree into an [`ElementSpec`] that carries a
//! [`StyleSpec`] per node. The spec is plain data, so it is unit-testable on its own.
//!
//! Unrecognized `style_props` keys, malformed values and unrecognized `attributes` keys are
//! ignored. Reading runs on the render path, where nothing can raise a catchable exception.
//! [`style_warning`] lets the caller that stores a style report an unknown key, an invalid color
//! or a value of an unsupported shape once, when it is set.

mod build;
mod color;
mod keys;
mod number;
mod parse;
mod scroll;

pub use build::{ElementSpec, ElementTag, build_spec, hidden_by_attribute};
pub use keys::style_warning;
pub(crate) use keys::{css_declaration, unknown_key_warning};
pub use parse::{ColorSpec, EdgesSpec, LengthSpec, StyleSpec, declared_window_size};
pub use scroll::{WheelAxes, is_scroll_container, wheel_axes};

pub use inca_node::tree::{
    AlignSpec, DisplaySpec, FlexDirectionSpec, OverflowSpec, STYLE_KEYS, StyleKey, ValueKind,
};
