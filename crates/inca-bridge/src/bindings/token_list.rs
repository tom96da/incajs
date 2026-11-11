// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `DOMTokenList` class of `classList` and its iterator.

mod iterator;
mod list;

pub(in crate::bindings) use list::{install_token_list, method};
