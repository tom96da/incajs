// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Crypto` and `SubtleCrypto` classes and the `crypto` global.
//!
//! The `classes` module defines the classes and the global, the `random` module the functions that
//! read the system random source and the `digest` module `crypto.subtle.digest()`.

mod classes;
mod digest;
mod random;

pub(crate) use classes::install;
