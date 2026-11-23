// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Decodes source maps and rewrites the frames of a JS stack through them.

mod load;
mod mappings;
mod remap;
mod store;

pub use remap::StackPaths;
#[cfg(test)]
pub(crate) use remap::mapped_body;
pub(crate) use remap::{register, register_compile, remap_stack, set_paths};
