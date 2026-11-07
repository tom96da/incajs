// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Converts a retained [`VirtualTree`](inca_node::tree::VirtualTree) into real `gpui` elements
//! ([`build_element`]/[`render_tree`]), reading each node through
//! [`build_spec`](inca_node::build_spec).

mod build;
mod children;
mod input;
mod pointer;
mod style;
mod wheel;

pub use build::{
    NodeLayout, build_element, build_element_with_events, debug_selector, render_tree,
    render_tree_with_events,
};
pub use input::TextInputHandler;
pub use pointer::{clear_presses, mouse_target, opens_context_menu, pressed_target};

/// The scroll geometry and `rect_of`, which reads a laid-out box.
pub mod scroll {
    pub use super::children::rect_of;
    pub use inca_node::scroll::{Align, Rect, scroll_chain, scroll_delta};
}
