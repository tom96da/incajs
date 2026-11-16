// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The global function `structuredClone`.
//!
//! The `globals` module defines the function and reads its arguments, the `intrinsics` module
//! keeps what the walk reads from the realm, the `walk` module copies a value with an explicit
//! frame stack, the `leaf` module copies the values with no children, and the `buffer` module
//! copies buffers and views and moves the buffers of the transfer list.

mod buffer;
mod globals;
mod intrinsics;
mod leaf;
mod walk;

pub(crate) use globals::install;
pub use intrinsics::register_uncloneable;
