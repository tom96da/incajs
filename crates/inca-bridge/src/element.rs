// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Element` interface: the natives that create elements, read and write their attributes and
//! styles, parse markup and hold the ARIA element references, and the members of `Element` that
//! read the name, the attributes, the markup, the pointer capture and the meaning of an element. It
//! holds the natives for focus, pointer capture, scrolling, selectors and input text, and the
//! members that read the box of an element, scroll it and act on it.

mod attributes;
mod capture;
mod focus;
mod html;
mod input;
mod layout_boxes;
mod members;
mod names;
mod scroll;
mod selector;
mod semantics;
mod style_props;

pub(crate) use capture::install_capture;
pub(crate) use focus::install_focus;
pub(crate) use html::install_html;
pub(crate) use input::install_input;
pub(crate) use layout_boxes::install as install_layout_members;
pub(crate) use members::install as install_members;
pub(crate) use members::install_markup as install_markup_members;
pub(crate) use names::install_tree;
pub(crate) use scroll::install_scroll;
pub(crate) use selector::install_selector;
pub(crate) use semantics::install as install_semantics_members;
pub(crate) use semantics::install_aria;
