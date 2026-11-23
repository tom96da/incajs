// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The pointer state of the app and the listeners each container wires, and the tracker that
//! reports each pointer input once.

mod target;
mod tracker;

pub use target::{clear_presses, mouse_target, opens_context_menu, pressed_target};
pub(crate) use target::{
    control_click_is_right, mark_mouse_target, mark_target, move_over, report_hover, restored_down,
    restored_up, root_keys, wire_focus, wire_hover, wire_modifier_keys,
};
pub(crate) use tracker::pointer_tracker;
