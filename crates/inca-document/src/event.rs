// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The flags of each event name and the key codes of a keyboard event.

mod flags;
mod key_code;

pub use flags::{EVENTS, EventSpec, Flags, flags_of, spec_of};
pub use key_code::{key_code, keypress_code};
