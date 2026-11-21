// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The flags of each event name, the key codes of a keyboard event and the payloads events carry.

mod flags;
mod key_code;
mod payload;

pub use flags::{EVENTS, EventSpec, Flags, flags_of, spec_of};
pub use key_code::{key_code, keypress_code};
pub use payload::{
    EventPayload, KeyPayload, Modifiers, MousePayload, NO_BUTTON, PointerSource, WheelPayload,
    buttons_mask,
};
