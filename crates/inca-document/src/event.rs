// Copyright (c) 2026 tom96da
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The flags of each event name, the path and the delivery order of an event, the key codes of a
//! keyboard event, the payloads events carry, the targets and the listener registry.

mod dispatch;
mod flags;
mod key_code;
mod listeners;
mod payload;
mod signals;
mod target;

pub use dispatch::{Invoke, Outcome, Walk, deliver, listened, next_event_id, path_from};
pub use flags::{EVENTS, EventSpec, Flags, flags_of, spec_of};
pub use key_code::{key_code, keypress_code};
pub use listeners::{EventListeners, Listener};
pub use payload::{
    EventPayload, KeyPayload, Modifiers, MousePayload, NO_BUTTON, PointerSource, WheelPayload,
    buttons_mask,
};
pub use signals::{Link, Links, Signals};
pub use target::EventTargetId;
