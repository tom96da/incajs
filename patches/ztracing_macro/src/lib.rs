// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! No-op stand-in for zed-industries/zed's GPL-3.0-or-later
//! `ztracing_macro`, a build-time dependency of `gpui`.

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn instrument(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
