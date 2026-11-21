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
pub mod semantics;
pub mod text;
pub mod warnings;

pub use attributes::store_attribute;
pub use create::{NameError, is_element_local_name, validate_and_extract};
pub use document::new_document_tree;
pub use event::{
    EVENTS, EventPayload, EventSpec, Flags, KeyPayload, Modifiers, MousePayload, NO_BUTTON,
    PointerSource, WheelPayload, buttons_mask, flags_of, key_code, keypress_code, spec_of,
};
pub use focus::{FocusModel, FocusSurface, FocusTransition};
pub use geometry::{Align, Rect, scroll_chain, scroll_delta};
pub use markup::{
    MarkupContext, Position, children_of, insert_parsed, parse_adjacent, parse_inner, parse_outer,
    replace_children, replace_with,
};
pub use removal::apply_removals;
pub use semantics::{
    ARIA_REFLECTIONS, AriaKind, AriaReferences, AriaReflection, TabIndex, aria_reflection,
    element_aria_attribute,
};
pub use text::{Edit, InputPayload, InputState, InputType, Motion};
pub use warnings::StyleWarnings;

#[cfg(test)]
mod tests;
