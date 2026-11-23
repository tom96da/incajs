// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of pointer events. The window reports presses, hovers, moves and wheels over a real
//! `VirtualTree` rendered through `render_tree_with_events`.

#![allow(clippy::unwrap_used, clippy::float_cmp)]

mod click;
mod disabled;
mod fields;
mod hover;
mod ids;
mod movement;
mod press;
