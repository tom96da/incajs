// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

mod key;
mod payload;
mod sink;

pub use key::is_menu_key;
pub(crate) use payload::button_index;
pub use payload::{
    click_payload, context_menu_payload, key_down_payload, key_up_payload, modifiers_of, mouse_at,
    mouse_down_payload, mouse_move_payload, mouse_up_payload, wheel_payload,
};
pub use sink::EventSink;
pub(crate) use sink::noop_sink_methods;
