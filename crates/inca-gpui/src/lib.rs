// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod element;
pub mod event_sink;
pub mod input;
mod pointer;
mod scrollbar;
pub use element::{
    NodeLayout, TextInputHandler, build_element, build_element_with_events, debug_selector,
    node_gutter, rect_of, render_tree, render_tree_with_events,
};
pub use event_sink::{
    EventSink, click_payload, context_menu_payload, is_menu_key, key_down_payload, key_up_payload,
    modifiers_of, mouse_at, mouse_down_payload, mouse_move_payload, mouse_up_payload,
    wheel_payload,
};
pub use input::{InputLayout, InputRequest};
pub use pointer::{clear_presses, mouse_target, opens_context_menu, pressed_target};
pub use scrollbar::Scrollbars;

#[cfg(test)]
mod tests;
