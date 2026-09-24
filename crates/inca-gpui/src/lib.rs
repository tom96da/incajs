// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod element;
pub mod event_sink;
pub mod tree;

pub use element::{
    AlignSpec, DisplaySpec, ElementSpec, ElementTag, FlexDirectionSpec, LengthSpec, StyleSpec,
    build_element, build_element_with_events, build_spec, build_spec_with, render_tree,
    render_tree_with_events,
};
pub use event_sink::{EventKind, EventMask, EventPayload, EventSink, MousePayload};
pub use tree::{AttributeValue, NodeId, TreeError, VirtualNode, VirtualTree};
