// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod focus;
pub mod geometry;
pub mod removal;
pub mod semantics;
pub mod text;

pub use focus::{FocusModel, FocusSurface, FocusTransition};
pub use geometry::{Align, Rect, scroll_chain, scroll_delta};
pub use removal::apply_removals;
pub use semantics::{
    ARIA_REFLECTIONS, AriaKind, AriaReferences, AriaReflection, TabIndex, aria_reflection,
};
pub use text::{Edit, InputPayload, InputState, InputType, Motion};

#[cfg(test)]
mod tests;
