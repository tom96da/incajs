// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What JS sees of scrolling. The scroll containers of a real `VirtualTree`, rendered through
//! `render_tree_with_events`, scroll by `ScrollEvent`s, by script offsets and by the reveal of
//! focused or scrolled-to nodes.

#![allow(clippy::unwrap_used)]

mod events;
mod reveal;
mod wheel;
