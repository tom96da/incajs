// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod element;
pub mod event_sink;
pub mod focus;
pub mod input;
pub mod tree;

pub use element::{
    AlignSpec, DisplaySpec, EdgesSpec, ElementSpec, ElementTag, FlexDirectionSpec, LengthSpec,
    NodeLayout, OverflowSpec, STYLE_KEYS, StyleKey, StyleSpec, TextInputHandler, ValueKind,
    build_element, build_element_with_events, build_spec, clear_presses, debug_selector,
    hidden_by_attribute, is_scroll_container, mouse_target, opens_context_menu, pressed_target,
    render_tree, render_tree_with_events, style_warning, wheel_axes,
};
pub use event_sink::{
    EventPayload, EventSink, KeyPayload, MousePayload, NO_BUTTON, PointerSource, WheelPayload,
    buttons_mask, is_menu_key,
};
pub use input::{Edit, InputLayout, InputPayload, InputRequest, InputState, InputType, Motion};
pub use tree::{
    AttributeValue, NodeId, TreeError, VirtualNode, VirtualTree, focusable_by_default,
    is_disabled_control,
};
