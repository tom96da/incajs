// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scrollbars of a scroll container: the setting and geometry of a bar, the view that draws
//! it, and the press, drag and hover of the pointer on it.

mod bar;
mod drag;
mod hover;
mod press;
mod view;

pub use bar::Scrollbars;
pub(crate) use bar::gutter_of;
pub(crate) use hover::OverBar;
pub(crate) use view::scrollbar;
