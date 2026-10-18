// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod element;
pub mod event_sink;
pub mod focus;
pub mod tree;

pub use element::{
    AlignSpec, DisplaySpec, EdgesSpec, ElementSpec, ElementTag, FlexDirectionSpec, LengthSpec,
    NodeLayout, OverflowSpec, StyleSpec, build_element, build_element_with_events, build_spec,
    clear_presses, debug_selector, hidden_by_attribute, is_scroll_container, mouse_target,
    pressed_target, render_tree, render_tree_with_events, style_warning, wheel_axes,
};
pub use event_sink::{
    EventPayload, EventSink, KeyPayload, MousePayload, PointerSource, WheelPayload, buttons_mask,
};
pub use tree::{AttributeValue, NodeId, TreeError, VirtualNode, VirtualTree, is_disabled_button};
