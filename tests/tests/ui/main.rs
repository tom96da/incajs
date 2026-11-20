// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Mounts real `.vue` apps in the `Harness` and checks the tree they draw.
//! One test target, so the fixtures are bundled once per run.

mod abort_signal;
mod activation;
mod capture;
mod click;
mod common;
mod components;
mod composition;
mod context_menu;
mod disabled;
mod event_flags;
mod event_target;
mod focus;
mod focus_scroll;
mod geometry;
mod hover;
mod input;
mod interfaces;
mod key_activation;
mod keyboard;
mod layout;
mod listeners;
mod lists;
mod members;
mod node_mutation;
mod propagation;
mod reactivity;
mod scroll;
mod scroll_into_view;
mod scrollbar;
mod style;
mod tabindex;
mod template;
mod timing;
mod visual_state;
