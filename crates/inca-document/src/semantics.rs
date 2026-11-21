// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What an element means beyond its attributes. The tests of each part sit in its file.

mod aria;
mod tab_index;

pub use aria::{ARIA_REFLECTIONS, AriaKind, AriaReferences, AriaReflection, aria_reflection};
pub use tab_index::TabIndex;
