// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod attributes;
pub mod create;
pub mod document;
pub mod event;
pub mod focus;
pub mod geometry;
pub mod markup;
pub mod removal;
pub mod reveal;
pub mod semantics;
pub mod text;
pub mod view;
pub mod warnings;

pub use create::{NameError, is_element_local_name, validate_and_extract};
pub use document::{DocumentState, new_document_tree};
pub use event::{
    EVENTS, EventListeners, EventPayload, EventSpec, EventTargetId, Flags, Invoke, KeyPayload,
    Link, Links, Listener, Modifiers, MousePayload, NO_BUTTON, Outcome, PointerSource, Signals,
    Walk, WheelPayload, buttons_mask, deliver, flags_of, key_code, keypress_code, listened,
    next_event_id, path_from, spec_of,
};
pub use focus::{FocusModel, FocusPlan, FocusStep, FocusTransition};
pub use geometry::{
    Align, Rect, client_height, client_width, offset_of, scroll_chain, scroll_delta,
};
pub use markup::{
    Position, children_of, insert_parsed, parse_adjacent, parse_inner, parse_outer,
    replace_children, replace_with,
};
pub use reveal::{Reveal, RevealOrigin, RevealQueue, Reveals};
pub use semantics::{
    ARIA_REFLECTIONS, AriaKind, AriaReferences, AriaReflection, TabIndex, aria_reflection,
    element_aria_attribute,
};
pub use text::{Edit, InputPayload, InputState, InputType, Motion};
pub use view::{NodeBox, ScrollRange, View};
pub use warnings::StyleWarnings;

#[cfg(test)]
mod tests;
