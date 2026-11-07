// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod element;
pub mod event_sink;
pub mod focus;
pub mod input;
pub use element::{
    NodeLayout, TextInputHandler, build_element, build_element_with_events, clear_presses,
    debug_selector, mouse_target, opens_context_menu, pressed_target, rect_of, render_tree,
    render_tree_with_events,
};
pub use event_sink::{
    EventPayload, EventSink, KeyPayload, MousePayload, NO_BUTTON, PointerSource, WheelPayload,
    buttons_mask, is_menu_key,
};
pub use input::{Edit, InputLayout, InputPayload, InputRequest, InputState, InputType, Motion};

#[cfg(test)]
mod tests;
