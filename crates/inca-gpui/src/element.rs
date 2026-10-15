// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Converts a retained [`VirtualTree`](crate::tree::VirtualTree) into real `gpui` elements.
//!
//! Split into two layers:
//! - a pure spec layer ([`ElementSpec`]/[`StyleSpec`]/[`build_spec`]) that
//!   has no `gpui` dependency and is exhaustively unit-testable on its own;
//! - a thin gpui layer ([`build_element`]/[`render_tree`]) that turns that
//!   spec into a real [`AnyElement`](gpui::AnyElement).
//!
//! Unrecognized `style_props` keys, malformed values and unrecognized
//! `attributes` keys are ignored. This runs on the render path, where nothing
//! can raise a catchable exception. [`style_warning`] lets the caller that
//! stores a style report an unknown key or an invalid color once, when it is
//! set.

mod build;
mod parse;
mod pointer;
pub mod scroll;
mod spec;
mod style;

pub use build::{
    NodeLayout, build_element, build_element_with_events, debug_selector, render_tree,
    render_tree_with_events,
};
pub use parse::style_warning;
pub use pointer::{clear_presses, mouse_target, pressed_target};
pub use spec::{
    AlignSpec, DisplaySpec, EdgesSpec, ElementSpec, ElementTag, FlexDirectionSpec, LengthSpec,
    OverflowSpec, StyleSpec, build_spec, hidden_by_attribute, is_scroll_container,
};

#[cfg(test)]
mod tests;
