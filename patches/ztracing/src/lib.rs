// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! No-op stand-in for zed-industries/zed's `ztracing`, which `gpui`
//! depends on — avoids pulling in its GPL-3.0-or-later `zlog` dependency.

pub use ztracing_macro::instrument;
