// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

pub mod geometry;
pub mod semantics;

pub use geometry::{Align, Rect, scroll_chain, scroll_delta};
pub use semantics::{
    ARIA_REFLECTIONS, AriaKind, AriaReferences, AriaReflection, TabIndex, aria_reflection,
};
