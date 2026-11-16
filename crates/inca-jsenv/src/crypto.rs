// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Crypto` class and the `crypto` global.
//!
//! The `classes` module defines the class and the global, and the `random` module the functions
//! that read the system random source.

mod classes;
mod random;

pub(crate) use classes::install;
