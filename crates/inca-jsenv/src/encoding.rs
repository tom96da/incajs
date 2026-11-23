// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `TextEncoder` and `TextDecoder` classes.
//!
//! Each instance owns plain Rust data: a `Codec` for a decoder and nothing for an encoder. The
//! prototypes are kept in the vault, so a script that replaces a property of an instance changes
//! nothing here.
//! The `encoder` module writes UTF-8 from the WTF-8 form of a string and the `decoder` module
//! decodes bytes. This module installs both classes and converts the shared arguments.

mod construct;
mod decoder;
mod encoder;

pub(crate) use construct::install;
pub(crate) use encoder::encode;
